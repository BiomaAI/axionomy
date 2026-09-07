use super::*;
use axionomy::Exchange;
use axionomy_problems::mission::{self, AccountId, AgentId, Asset, Location, Policy, World};
use axionomy_search::mcts::MctsConfig;
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, FrontierCompletenessView, GraphEdgeView,
    GraphNodeView, ObjectiveAxisView, ObjectiveDirectionView, ObservationView, ParetoFrontView,
    ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = instance_profile(request, descriptor);
    let model = mission::initial();
    let sample_index = (request.seed as usize) % 16;
    let samples = match profile {
        InstanceProfile::Micro => request.budget.clamp(2, 16),
        InstanceProfile::Showcase => request.budget.max(2),
        InstanceProfile::Stress => request.budget.max(256),
    } as usize;
    let comparison = mission::monte_carlo_with_progress(
        &model,
        samples,
        request.seed,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                "monte_carlo",
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
    progress.ensure()?;
    let comparison = comparison.ok_or_else(|| problem_error("mission", "Monte Carlo failed"))?;
    let front = mission::policy_front_with_progress(
        &model,
        samples,
        request.seed,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                "pareto_sampling",
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
    progress.ensure()?;
    let front = front.ok_or_else(|| problem_error("mission", "policy front failed"))?;
    let baseline_observations = progress
        .observations()
        .iter()
        .filter(|item| item.phase == "monte_carlo")
        .cloned()
        .collect::<Vec<_>>();
    let planned = mission::run_planned_with_progress(
        &model,
        sample_index,
        MctsConfig::new(samples, 12).with_seed(request.seed),
        samples.clamp(1, 8),
        |decision, state| {
            progress.emit(
                "ismcts",
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
    .map_err(|error| problem_error("mission", error))?;
    progress.ensure()?;
    let planned = planned.ok_or_else(|| problem_error("mission", "planning was interrupted"))?;
    let planning_observations = progress
        .observations()
        .iter()
        .filter(|item| item.phase == "ismcts")
        .cloned()
        .collect::<Vec<_>>();
    let policies = [
        ("coordinated", Policy::ShareAndCoordinate),
        ("direct_north", Policy::NorthTogether),
    ];
    let mut documents = Vec::new();
    for (strategy, policy) in policies {
        let rollout = if policy == Policy::ShareAndCoordinate {
            planned.rollout.clone()
        } else {
            mission::run_policy(&model, policy, sample_index)
        };
        let mut view = document(DocumentSpec { problem: "mission", strategy, title: if policy == Policy::ShareAndCoordinate { "Mission · observe, decide, replan" } else { "Mission · both go north" }, description: if policy == Policy::ShareAndCoordinate { "ISMCTS chooses every public action from the Scout’s observation. After each action and Nature response, compatible belief worlds are retained and the next decision is searched again. Compare the resulting costs with committing north without information." } else { "Both agents commit without looking or sharing. The replay keeps every failure and what it cost." }, source_label: "Hidden-information mission" }, &model, &mission::goal(), rollout.trace(), vec![
            ObjectiveView { key: "success".into(), label: "Succeeded".into(), direction: ObjectiveDirectionView::Maximize, value: u8::from(rollout.succeeded()).to_string() },
            ObjectiveView { key: "time".into(), label: "Elapsed time".into(), direction: ObjectiveDirectionView::Minimize, value: rollout.elapsed_time().to_string() },
            ObjectiveView { key: "medical".into(), label: "Medical kit used".into(), direction: ObjectiveDirectionView::Minimize, value: u8::from(rollout.used_medical_kit()).to_string() },
        ], scene).map_err(|error| problem_error("mission", error))?;
        view.pareto_fronts.push(policy_front(
            &front,
            (policy == Policy::NorthTogether).then_some(policy),
        ));
        view.solve_observations = if policy == Policy::ShareAndCoordinate {
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
        let mut replay = model.fork();
        for (index, frame) in view.frames.iter_mut().enumerate() {
            replay
                .apply(rollout.trace().exchanges()[index].clone())
                .map_err(|error| problem_error("mission", format!("{error:?}")))?;
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
                view.proposals.push(proposal("mission", ProposalSpec {
                    id: "move-before-sharing",
                    label: "Move together before sharing the sighting",
                    description: "The Scout has seen a location, but the Medic has no intelligence yet. Assessing this move at the post-scan snapshot exposes the missing shared information; sharing is a causal prerequisite for coordinated movement.",
                }, &replay, &premature));
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
            let malformed = Exchange::new(*candidate.rate(), *candidate.units());
            view.proposals.push(proposal("mission", ProposalSpec { id: "action-without-actors", label: "Mission action without roles", description: "Every mission action names an actor, Nature, the mission, and the goal. Leaving them out is rejected with the specific role missing." }, &model, &malformed));
        }
        documents.push(view);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn observation(world: &World, agent: AgentId) -> ObservationView {
    let key = mission::agent_view(world, agent).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(
                format!("mission:account:{account:?}"),
                account.studio_label(),
            ),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(format!("mission:asset:{asset:?}"), asset.studio_label()),
                    quantity: ExactQuantity(quantity.to_string()),
                })
                .collect(),
        })
        .collect();
    ObservationView {
        actor: ViewId::new(format!("mission:actor:{agent:?}"), format!("{agent:?}")),
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
                asset: ViewId::new(format!("mission:asset:{asset:?}"), asset.studio_label()),
                quantity: ExactQuantity(quantity.to_string()),
            })
            .collect(),
    }
}

fn policy_front(front: &mission::PolicyFront, selected: Option<Policy>) -> ParetoFrontView {
    ParetoFrontView {
        title:
            "Fixed-policy baselines · reliability, time, medical kit (planner not evaluated here)"
                .into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: vec![
            ObjectiveAxisView {
                key: "success".into(),
                label: "Success probability".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: "time".into(),
                label: "Mean time".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "medical".into(),
                label: "Medical-kit use".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points: front
            .entries()
            .iter()
            .map(|entry| {
                let policy = *entry.payload().policy();
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|o| format!("{:.3}", o.value()))
                    .collect();
                ParetoPointView {
                    label: format!("{policy:?}"),
                    values,
                    selected: Some(policy) == selected,
                }
            })
            .collect(),
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        (Location::Base, 310.0, 250.0),
        (Location::North, 110.0, 60.0),
        (Location::South, 510.0, 60.0),
    ];
    let nodes = locations
        .into_iter()
        .map(|(location, x, y)| {
            let occupied = [AgentId::Scout, AgentId::Medic].into_iter().any(|agent| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(location))
                    .is_zero()
            });
            GraphNodeView {
                id: ViewId::new(format!("location:{location:?}"), format!("{location:?}")),
                classes: if occupied {
                    vec!["current".into()]
                } else {
                    Vec::new()
                },
                x: Some(x),
                y: Some(y),
            }
        })
        .collect();
    let edges = [Location::North, Location::South]
        .into_iter()
        .map(|location| GraphEdgeView {
            id: format!("mission:{location:?}"),
            source: "location:Base".into(),
            target: format!("location:{location:?}"),
            label: Some("hidden truth + hazard".into()),
            classes: if [AgentId::Scout, AgentId::Medic].into_iter().any(|agent| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(location))
                    .is_zero()
            }) {
                vec!["current".into(), "uncertain".into()]
            } else {
                vec!["uncertain".into()]
            },
        })
        .collect();
    let agent_entities = [AgentId::Scout, AgentId::Medic].into_iter().map(|agent| {
        let location = [Location::Base, Location::North, Location::South]
            .into_iter()
            .find(|location| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(*location))
                    .is_zero()
            })
            .expect("mission agents remain at an encoded location");
        let mut entity = link_balance(
            visual_entity(
                format!("agent:{agent:?}"),
                format!("{agent:?}"),
                if agent == AgentId::Scout {
                    SceneGlyphView::Sensor
                } else {
                    SceneGlyphView::Agent
                },
                SceneAnchorView::GraphNode {
                    node: format!("location:{location:?}"),
                },
                SceneEntityRoleView::Occupant,
                SceneToneView::Active,
                Some(
                    if world
                        .balance(&AccountId::Agent(agent), &Asset::Injured)
                        .is_zero()
                    {
                        "ready".into()
                    } else {
                        "injured".into()
                    },
                ),
            ),
            format!("mission:account:agent-{agent:?}").to_ascii_lowercase(),
            format!("mission:asset:at-{location:?}").to_ascii_lowercase(),
        );
        entity.metrics = vec![visual_metric(
            format!("energy-{agent:?}"),
            "Energy",
            world.balance(&AccountId::Agent(agent), &Asset::Energy),
            Some("units"),
        )];
        entity
    });
    let system_entities = [
        link_account(
            visual_entity(
                "system:mission",
                "Mission control",
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: "location:Base".into(),
                },
                SceneEntityRoleView::State,
                SceneToneView::Neutral,
                Some("shared state".into()),
            ),
            "mission:account:mission",
        ),
        link_account(
            visual_entity(
                "system:nature",
                "Hidden scenario",
                SceneGlyphView::Weather,
                SceneAnchorView::GraphNode {
                    node: "location:North".into(),
                },
                SceneEntityRoleView::Context,
                SceneToneView::Uncertain,
                Some("private truth".into()),
            ),
            "mission:account:nature",
        ),
        link_account(
            visual_entity(
                "system:success",
                "Mission outcome",
                SceneGlyphView::Goal,
                SceneAnchorView::GraphNode {
                    node: "location:South".into(),
                },
                SceneEntityRoleView::State,
                SceneToneView::Goal,
                Some("outcome account".into()),
            ),
            "mission:account:success",
        ),
    ];
    Some(
        Scene::graph("Where each agent is, and what it knows", nodes, edges, None)
            .with_entities(agent_entities.chain(system_entities))
            .with_metrics([
                visual_metric(
                    "time",
                    "Mission time remaining",
                    world.balance(&AccountId::Mission, &Asset::TimeRemaining),
                    Some("ticks"),
                ),
                visual_metric(
                    "shared-intel",
                    "Shared intel",
                    [Location::North, Location::South]
                        .into_iter()
                        .filter(|location| {
                            !world
                                .balance(
                                    &AccountId::Agent(AgentId::Medic),
                                    &Asset::SharedIntel(*location),
                                )
                                .is_zero()
                        })
                        .count(),
                    Some("reports"),
                ),
            ]),
    )
}

#[cfg(test)]
mod tests {
    use crate::{ReferenceService, RunRequest};

    #[test]
    fn mission_replays_actual_search_and_private_information_transfer() {
        let mut request = RunRequest::new("mission");
        request.seed = 11;
        let artifact = ReferenceService.run(request).unwrap();
        let planned = &artifact.documents[0];
        let direct = &artifact.documents[1];
        assert_eq!(planned.objectives[0].value, "1");
        assert_eq!(direct.objectives[0].value, "0");
        assert_eq!(planned.frames[0].after, direct.frames[0].after);
        assert!(
            planned
                .solve_observations
                .iter()
                .all(|item| item.phase == "ismcts")
        );
        assert!(
            direct
                .solve_observations
                .iter()
                .all(|item| item.phase == "monte_carlo")
        );
        let scan = planned
            .frames
            .iter()
            .find(|frame| frame.exchange.rate.label == "Scout sees South")
            .unwrap();
        assert!(!scan.observations[0].facts.is_empty());
        assert!(scan.observations[1].facts.is_empty());
        let share = planned
            .frames
            .iter()
            .find(|frame| frame.exchange.rate.label == "Scout shares sighting: South")
            .unwrap();
        assert!(!share.observations[1].facts.is_empty());
        for frame in &planned.frames {
            for observation in &frame.observations {
                assert!(
                    observation
                        .visible_accounts
                        .iter()
                        .all(|account| !account.account.key.contains("Nature"))
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
