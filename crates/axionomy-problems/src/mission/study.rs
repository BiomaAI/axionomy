//! The hidden-information mission as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    mission::{self, AccountId, AgentId, Asset, Location, Policy, World},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::{Exchange, path};
use axionomy_search::{mcts::MctsConfig, rollout::Unrecorded};
use axionomy_study::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, StudyError, StudyKey, StudyProgress, Work, document, problem_error, proposal,
    proposal_at, telemetry,
};
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, Keyed, Label, ObjectiveAxisView,
    ObjectiveDirectionView, ObjectiveView, ObservationView, ParetoFrontView,
    SearchObservationKindView, TelemetryKindView, ViewDocument, ViewId, account_key, asset_key,
    sampled_policy_front,
};

/// Hidden-information mission: its catalog entry and how its studies are computed.
pub struct Mission {
    descriptor: ProblemDescriptor,
}

impl Default for Mission {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("mission"),
                    title: "Hidden-information mission",
                    summary: "Coordinate independent Scout and Medic timelines. Share private discoveries, recover archives, stabilize casualties and rendezvous for extraction as hazards close sectors and resources run short.",
                    instances: [
                        "Sixteen hidden scenarios with a small per-decision search budget",
                        "A hidden archive and casualty, independent travel, timed recovery, radio or local briefing, sector hazards and clinical decline",
                        "A second casualty, one radio charge, lower energy, finite resupply and one evacuation seat",
                    ],
                },
                ProblemFamily::PartialObservation,
                path!("coordinated"),
                &[
                    StrategyCopy {
                        key: path!("coordinated"),
                        label: "Coordinate independent agents",
                        description: "Larger missions use separate observation-responsive policies with radio briefing; Micro retains belief-conditioned ISMCTS.",
                        algorithm: "Private-view policies; Micro information-set MCTS",
                    },
                    StrategyCopy {
                        key: path!("direct-north"),
                        label: "Search north first",
                        description: "Larger missions begin north, then coordinate through local briefings. Micro retains the blind joint-movement baseline.",
                        algorithm: "North-first local coordination; Micro Monte Carlo",
                    },
                ],
                &[
                    Capability::InformationSetSearch,
                    Capability::PartialObservation,
                    Capability::Chance,
                    Capability::MonteCarlo,
                    Capability::ApproximatePareto,
                    Capability::RlProjection,
                ],
            ),
        }
    }
}

impl Problem for Mission {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, StudyError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let profile = study.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return super::expedition::study::build(study, profile, progress);
    }
    let model = mission::initial();
    let sample_index = (study.seed as usize) % 16;
    let samples = match profile {
        InstanceProfile::Micro => study.budget.clamp(2, 16),
        InstanceProfile::Showcase => study.budget.max(2),
        InstanceProfile::Stress => study.budget.max(256),
    } as usize;
    let (monte_carlo, pareto_sampling) = (Work::monte_carlo(), Work::pareto_sampling());
    let ismcts = Work::new(
        path!("ismcts"),
        path!("ismcts"),
        SearchObservationKindView::Tree,
    );
    let comparison = mission::monte_carlo_with_progress(
        &model,
        samples,
        study.seed,
        &mut Unrecorded,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                &monte_carlo,
                state.completed_samples() as u64,
                state.total_samples() as u64,
                format!(
                    "{}/{} hidden-scenario policy rollouts",
                    state.completed_samples(),
                    state.total_samples()
                ),
            )
        },
    );
    progress.checkpoint()?;
    let comparison =
        comparison.ok_or_else(|| problem_error(&study.problem, "Monte Carlo failed"))?;
    let front = mission::policy_front_with_progress(
        &model,
        samples,
        study.seed,
        &mut Unrecorded,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                &pareto_sampling,
                state.completed_samples() as u64,
                state.total_samples() as u64,
                format!(
                    "{}/{} multi-objective mission rollouts",
                    state.completed_samples(),
                    state.total_samples()
                ),
            )
        },
    );
    progress.checkpoint()?;
    let front = front.ok_or_else(|| problem_error(&study.problem, "policy front failed"))?;
    let baseline_observations = progress
        .observations()
        .filter(|item| item.phase == monte_carlo.phase)
        .cloned()
        .collect::<Vec<_>>();
    let planned = mission::run_planned_with_progress(
        &model,
        sample_index,
        MctsConfig::new(samples, 12).with_seed(study.seed),
        samples.clamp(1, 8),
        |decision, state| {
            progress.emit(
                &ismcts,
                state.iterations() as u64,
                state.target_iterations() as u64,
                format!(
                    "Decision {} · {}/{} ISMCTS iterations · {} information sets",
                    decision + 1,
                    state.iterations(),
                    state.target_iterations(),
                    state.information_sets()
                ),
            )
        },
    )
    .map_err(|error| problem_error(&study.problem, error))?;
    progress.checkpoint()?;
    let planned =
        planned.ok_or_else(|| problem_error(&study.problem, "planning was interrupted"))?;
    let planning_observations = progress
        .observations()
        .filter(|item| item.phase == ismcts.phase)
        .cloned()
        .collect::<Vec<_>>();
    let policies = [
        (path!("coordinated"), Policy::ShareAndCoordinate),
        (path!("direct-north"), Policy::NorthTogether),
    ];
    let mut documents = Vec::new();
    for (strategy, policy) in policies {
        let rollout = if policy == Policy::ShareAndCoordinate {
            planned.rollout.clone()
        } else {
            mission::run_policy(&mut model.clone(), policy, sample_index)
        };
        let mut view = document(progress, DocumentSpec { problem: &study.problem, document: strategy, title: if policy == Policy::ShareAndCoordinate { "Mission · observe, decide, replan" } else { "Mission · both go north" }, description: if policy == Policy::ShareAndCoordinate { "ISMCTS chooses every public action from the Scout’s observation. After each action and Nature response, compatible belief worlds are retained and the next decision is searched again. Compare the resulting costs with committing north without information." } else { "Both agents commit without looking or sharing. The replay keeps every failure and what it cost." }, source_label: "Hidden-information mission" }, &model, &mission::goal(), rollout.trace(), vec![
            ObjectiveView::new(path!("success"), "Succeeded", ObjectiveDirectionView::Maximize, u64::from(rollout.succeeded())),
            ObjectiveView::new(path!("time"), "Elapsed time", ObjectiveDirectionView::Minimize, rollout.elapsed_time()),
            ObjectiveView::new(path!("medical"), "Medical kit used", ObjectiveDirectionView::Minimize, u64::from(rollout.used_medical_kit())),
        ], scene).map_err(|error| problem_error(&study.problem, error))?;
        view.pareto_fronts.push(policy_front(
            &front,
            (policy == Policy::NorthTogether).then_some(policy),
        ));
        view.thinking = if policy == Policy::ShareAndCoordinate {
            planning_observations.clone()
        } else {
            baseline_observations.clone()
        };
        view.telemetry.push(telemetry(
            if policy == Policy::ShareAndCoordinate {
                "Receding-horizon ISMCTS"
            } else {
                "Fixed-policy Monte Carlo"
            },
            false,
            [
                (
                    TelemetryKindView::Iteration,
                    if policy == Policy::ShareAndCoordinate {
                        planned
                            .decisions
                            .iter()
                            .map(|step| step.decision.iterations() as u64)
                            .sum()
                    } else {
                        0
                    },
                    "ISMCTS iterations".into(),
                ),
                (
                    TelemetryKindView::InformationSet,
                    if policy == Policy::ShareAndCoordinate {
                        planned
                            .decisions
                            .iter()
                            .map(|step| step.decision.information_sets() as u64)
                            .sum()
                    } else {
                        0
                    },
                    "information sets".into(),
                ),
                (
                    TelemetryKindView::Sample,
                    comparison.samples() as u64,
                    "policy rollouts".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    rollout.trace().exchanges().len() as u64,
                    "selected rollout exchanges".into(),
                ),
            ],
        ));
        view.observations = vec![
            observation(&model, AgentId::Scout),
            observation(&model, AgentId::Medic),
        ];
        let mut replay = model.clone();
        for (index, frame) in view.frames.iter_mut().enumerate() {
            replay
                .apply(rollout.trace().exchanges()[index].clone())
                .map_err(|error| problem_error(&study.problem, format!("{error:?}")))?;
            frame.observations = vec![
                observation(&replay, AgentId::Scout),
                observation(&replay, AgentId::Medic),
            ];
            if matches!(
                rollout.trace().exchanges()[index].rate(),
                mission::RateId::ResolveScan { .. }
            ) {
                let location = [Location::North, Location::South]
                    .into_iter()
                    .find(|location| {
                        !replay
                            .balance(&AccountId::Agent(AgentId::Scout), &Asset::Intel(*location))
                            .is_zero()
                    })
                    .expect("scan produced intelligence");
                let premature = Exchange::new(
                    mission::RateId::MoveTogether(location),
                    axionomy::Quantity::new(1),
                )
                .bind(mission::Role::Scout, AccountId::Agent(AgentId::Scout))
                .bind(mission::Role::Medic, AccountId::Agent(AgentId::Medic))
                .bind(mission::Role::Mission, AccountId::Mission);
                view.proposals.push(proposal_at(ProposalSpec {
                    id: path!("move-before-sharing"),
                    label: "Move together before sharing the sighting",
                    description: "The Scout has seen a location, but the Medic has no intelligence yet. Assessing this move at the post-scan snapshot exposes the missing shared information; sharing is a causal prerequisite for coordinated movement.",
                }, index as u64 + 1, &replay, &premature));
            }
            if policy == Policy::ShareAndCoordinate {
                if let Some(step) = planned
                    .decisions
                    .iter()
                    .find(|step| step.trace_index == index)
                {
                    frame.cues.push(axionomy_view::FrameCueView {
                        kind: axionomy_view::FrameCueKindView::Information,
                        label: "Chosen by ISMCTS from the Scout’s observation".into(),
                        details: vec![format!(
                            "{} compatible belief worlds · {} iterations · {} public alternatives",
                            step.prior_worlds,
                            step.decision.iterations(),
                            step.decision.children().len()
                        )],
                        subjects: Vec::new(),
                    });
                }
                if let Some(step) = planned.decisions.iter().find(|step| {
                    let next = planned
                        .decisions
                        .iter()
                        .find(|next| next.trace_index > step.trace_index)
                        .map_or(rollout.trace().exchanges().len(), |next| next.trace_index);
                    index + 1 == next
                }) {
                    frame.cues.push(axionomy_view::FrameCueView {
                        kind: axionomy_view::FrameCueKindView::Information,
                        label: "Beliefs conditioned on the new observation".into(),
                        details: vec![format!(
                            "{} → {} compatible worlds. The next decision uses this posterior.",
                            step.prior_worlds, step.posterior_worlds
                        )],
                        subjects: Vec::new(),
                    });
                }
            }
        }
        if !rollout.succeeded()
            && let Some(last) = view.frames.last_mut()
        {
            last.cues.push(axionomy_view::FrameCueView {
                    kind: axionomy_view::FrameCueKindView::Information,
                    label: "The mission did not reach its goal".into(),
                    details: vec!["A private sighting can be wrong. Replay validity proves that the actions obeyed the model; it does not promise that a decision under uncertainty succeeds.".into()],
                    subjects: Vec::new(),
                });
        }
        for frame in &mut view.frames {
            frame.cues.sort_by_key(|cue| match cue.kind {
                axionomy_view::FrameCueKindView::AtomicExchange => 0,
                axionomy_view::FrameCueKindView::Information => 1,
                _ => 2,
            });
        }
        if let Some(candidate) = mission::candidates(&model).first() {
            let malformed = Exchange::new(*candidate.rate(), candidate.units());
            view.proposals.push(proposal(ProposalSpec { id: path!("action-without-actors"), label: "Mission action without roles", description: "Every mission action names an actor, Nature, the mission, and the goal. Leaving them out is rejected with the specific role missing." }, &model, &malformed));
        }
        documents.push(view);
    }
    Ok(documents)
}

fn observation(world: &World, agent: AgentId) -> ObservationView {
    let key = mission::agent_view(world, agent).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(account_key::<mission::Ontology>(account), account.label()),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(asset_key::<mission::Ontology>(asset), asset.label()),
                    quantity: ExactQuantity(quantity.to_string()),
                })
                .collect(),
        })
        .collect();
    ObservationView {
        actor: ViewId::new(path!("mission/actor").then(&agent), format!("{agent:?}")),
        label: format!("{agent:?}-visible economic state; Nature remains hidden"),
        visible_accounts: accounts,
        facts: key
            .balances()
            .iter()
            .filter(|(owner, asset, _)| {
                *owner == AccountId::Agent(agent)
                    && matches!(
                        asset,
                        Asset::Intel(_)
                            | Asset::SharedIntel(_)
                            | Asset::Injured
                            | Asset::UsedMedicalKit
                    )
            })
            .map(|(_, asset, quantity)| AssetQuantityView {
                asset: ViewId::new(asset_key::<mission::Ontology>(asset), asset.label()),
                quantity: ExactQuantity(quantity.to_string()),
            })
            .collect(),
    }
}

fn policy_front(front: &mission::PolicyFront, selected: Option<Policy>) -> ParetoFrontView {
    sampled_policy_front(
        "Fixed-policy baselines · reliability, time, medical kit (planner not evaluated here)",
        vec![
            ObjectiveAxisView {
                key: path!("success"),
                label: "Success probability".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: path!("time"),
                label: "Mean time".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: path!("medical"),
                label: "Medical-kit use".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        front,
        |policy| {
            match policy {
                Policy::ShareAndCoordinate => "Share and coordinate",
                Policy::NorthTogether => "Both go north",
            }
            .into()
        },
        |policy| Some(policy) == selected,
    )
}

#[cfg(test)]
mod tests {
    use crate::registry;
    use axionomy::{Quantity, path};
    use axionomy_study::{InstanceProfile, StudyRequest};

    #[test]
    fn mission_replays_actual_search_and_private_information_transfer() {
        let mut request = StudyRequest::new(path!("mission")).with_instance(InstanceProfile::Micro);
        request.seed = 11;
        let study = registry().study(request).unwrap();
        let planned = &study.documents[0];
        let direct = &study.documents[1];
        assert_eq!(planned.objectives[0].value, Quantity::new(1));
        assert_eq!(direct.objectives[0].value, Quantity::ZERO);
        assert_eq!(planned.frames[0].after, direct.frames[0].after);
        assert!(
            planned
                .thinking
                .iter()
                .all(|item| item.phase == path!("ismcts"))
        );
        assert!(
            direct
                .thinking
                .iter()
                .all(|item| item.phase == path!("monte-carlo"))
        );
        let scan = planned
            .frames
            .iter()
            .find(|frame| frame.exchange.rate.label.starts_with("Scout sees South"))
            .unwrap();
        assert!(!scan.observations[0].facts.is_empty());
        assert!(scan.observations[1].facts.is_empty());
        let share = planned
            .frames
            .iter()
            .find(|frame| frame.exchange.rate.label == "Scout shares sighting: South")
            .unwrap();
        assert!(!share.observations[1].facts.is_empty());
        let scene = share.after.scene.as_ref().unwrap();
        assert_eq!(
            scene
                .metrics
                .iter()
                .find(|metric| metric.key == path!("shared-intel"))
                .unwrap()
                .value,
            "1"
        );
        assert!(
            scene
                .entities
                .iter()
                .filter(|entity| entity.id.key == path!("system/nature")
                    || entity.id.key == path!("system/success"))
                .all(|entity| matches!(entity.anchor, axionomy_view::SceneAnchorView::Unanchored))
        );
        for frame in &planned.frames {
            for observation in &frame.observations {
                assert!(
                    observation.visible_accounts.iter().all(|account| !account
                        .account
                        .key
                        .as_str()
                        .contains("nature"))
                );
            }
        }
        assert!(
            planned
                .proposals
                .iter()
                .any(|proposal| proposal.label == "Move together before sharing the sighting")
        );
        assert!(planned.frames.iter().any(|frame| {
            frame
                .cues
                .iter()
                .any(|cue| cue.label == "Chosen by ISMCTS from the Scout’s observation")
        }));
    }
}
