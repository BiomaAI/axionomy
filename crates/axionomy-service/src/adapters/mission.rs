use super::*;
use axionomy_search::rollout::Unrecorded;
mod expedition;
use axionomy::Exchange;
use axionomy_problems::mission::{self, AccountId, AgentId, Asset, Location, Policy, World};
use axionomy_search::mcts::MctsConfig;
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ObservationView, ParetoFrontView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return expedition::build(study, descriptor, profile, progress);
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
    progress.ensure()?;
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
    progress.ensure()?;
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
    progress.ensure()?;
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
    assemble(study, descriptor, documents)
}

fn observation(world: &World, agent: AgentId) -> ObservationView {
    let key = mission::agent_view(world, agent).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(
                account_key::<mission::Ontology>(account),
                account.studio_label(),
            ),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(asset_key::<mission::Ontology>(asset), asset.studio_label()),
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
                asset: ViewId::new(asset_key::<mission::Ontology>(asset), asset.studio_label()),
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
                id: ViewId::new(path!("location").then(&location), format!("{location:?}")),
                classes: if occupied {
                    vec![path!("current")]
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
            id: path!("mission").then(&location),
            source: path!("location").then(&Location::Base),
            target: path!("location").then(&location),
            label: Some("hidden truth + hazard".into()),
            classes: if [AgentId::Scout, AgentId::Medic].into_iter().any(|agent| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(location))
                    .is_zero()
            }) {
                vec![path!("current"), path!("uncertain")]
            } else {
                vec![path!("uncertain")]
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
        let known = [Location::North, Location::South]
            .into_iter()
            .find_map(|location| {
                if !world
                    .balance(&AccountId::Agent(agent), &Asset::SharedIntel(location))
                    .is_zero()
                {
                    Some(format!("shared {location:?}"))
                } else if !world
                    .balance(&AccountId::Agent(agent), &Asset::Intel(location))
                    .is_zero()
                {
                    Some(format!(
                        "{} {location:?}",
                        if agent == AgentId::Scout {
                            "saw"
                        } else {
                            "briefed"
                        }
                    ))
                } else {
                    None
                }
            });
        let mut entity = link_balance::<mission::Ontology>(
            visual_entity(
                path!("agent").then(&agent),
                format!("{agent:?}"),
                if agent == AgentId::Scout {
                    SceneGlyphView::Sensor
                } else {
                    SceneGlyphView::Agent
                },
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&location),
                },
                SceneEntityRoleView::Occupant,
                SceneToneView::Active,
                Some(
                    if world
                        .balance(&AccountId::Agent(agent), &Asset::Injured)
                        .is_zero()
                    {
                        known.unwrap_or_else(|| "ready".into())
                    } else {
                        "injured".into()
                    },
                ),
            ),
            &AccountId::Agent(agent),
            &Asset::At(location),
        );
        entity.metrics = vec![asset_metric(
            path!("energy").then(&agent),
            "Energy",
            &Asset::Energy,
            world.balance(&AccountId::Agent(agent), &Asset::Energy),
        )];
        entity
    });
    let system_entities = [
        link_account::<mission::Ontology>(
            visual_entity(
                path!("system/mission"),
                "Mission control",
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&Location::Base),
                },
                SceneEntityRoleView::State,
                SceneToneView::Neutral,
                Some("shared state".into()),
            ),
            &AccountId::Mission,
        ),
        link_account::<mission::Ontology>(
            visual_entity(
                path!("system/nature"),
                "Hidden scenario",
                SceneGlyphView::Weather,
                SceneAnchorView::Unanchored,
                SceneEntityRoleView::Context,
                SceneToneView::Uncertain,
                Some("private truth".into()),
            ),
            &AccountId::Nature,
        ),
        link_account::<mission::Ontology>(
            visual_entity(
                path!("system/success"),
                "Mission outcome",
                SceneGlyphView::Goal,
                SceneAnchorView::Unanchored,
                SceneEntityRoleView::Context,
                SceneToneView::Goal,
                Some(
                    if world.matches(&mission::goal()) {
                        "complete"
                    } else {
                        "rescue objective"
                    }
                    .into(),
                ),
            ),
            &AccountId::Success,
        ),
    ];
    Some(
        Scene::graph("Where each agent is, and what it knows", nodes, edges, None)
            .with_entities(agent_entities.chain(system_entities))
            .with_metrics([
                asset_metric(
                    path!("time"),
                    "Mission time remaining",
                    &Asset::TimeRemaining,
                    world.balance(&AccountId::Mission, &Asset::TimeRemaining),
                ),
                visual_metric(
                    path!("shared-intel"),
                    "Shared intel",
                    [Location::North, Location::South]
                        .into_iter()
                        .filter(|location| {
                            !world
                                .balance(
                                    &AccountId::Agent(AgentId::Scout),
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

// How Studio names and writes this problem's values.

impl StudioLabel for mission::Hazard {
    fn studio_label(&self) -> String {
        match self {
            Self::Safe => "no hazard",
            Self::Injury => "injury hazard",
        }
        .into()
    }
}

impl StudioFormat for mission::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy => QuantityFormat::COUNT.unit("units"),
            Self::TimeRemaining | Self::ElapsedTime => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for mission::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("{agent:?}"),
            Self::Nature => "Hidden mission scenario".into(),
            Self::Mission => "Mission clock and state".into(),
            Self::Success => "Mission objective".into(),
        }
    }
}

impl StudioLabel for mission::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::AgentIdentity(agent) => format!("Identity: {agent:?}"),
            Self::NatureIdentity => "Nature identity".into(),
            Self::MissionIdentity => "Mission identity".into(),
            Self::SuccessIdentity => "Objective identity".into(),
            Self::At(location) => format!("At {location:?}"),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Sensor => "Sensor available".into(),
            Self::UsedSensor => "Sensor used".into(),
            Self::MedicalKit => "Medical kit available".into(),
            Self::UsedMedicalKit => "Medical kit used".into(),
            Self::Intel(location) => format!("Private sighting: {location:?}"),
            Self::SharedIntel(location) => format!("Shared sighting: {location:?}"),
            Self::Injured => "Agent injured".into(),
            Self::Unresolved => "Scenario not yet drawn".into(),
            Self::ScenarioWeight(location, seed, danger) => format!(
                "Odds: objective at {location:?}, sensor state {seed}, {}",
                danger.studio_label()
            ),
            Self::Truth(location) => format!("Objective at {location:?}"),
            Self::Seed(seed) => format!("Sensor state {seed}"),
            Self::Hazard(danger) => format!("Drawn: {}", danger.studio_label()),
            Self::HazardResolved => "Hazard resolved".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::Planning => "Choosing an action".into(),
            Self::AwaitingScan => "Waiting for a sighting".into(),
            Self::AwaitingEncounter => "Waiting for a hazard outcome".into(),
            Self::NeedsTreatment => "Treatment required".into(),
            Self::AreaSafe => "Area made safe".into(),
            Self::Rescued => "Objective rescued".into(),
            Self::Solved => "Mission complete".into(),
        }
    }
}

impl StudioLabel for mission::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Instantiate {
                truth,
                seed,
                hazard: danger,
            } => format!(
                "Draw scenario: objective at {truth:?}, sensor state {seed}, {}",
                danger.studio_label()
            ),
            Self::BeginScan => "Scout scans the area".into(),
            Self::ResolveScan {
                truth,
                seed,
                report,
                ..
            } => format!("Scout sees {report:?} (objective at {truth:?}, sensor state {seed})"),
            Self::Share(location) => format!("Scout shares sighting: {location:?}"),
            Self::MoveTogether(location) => format!("Scout and Medic move to {location:?}"),
            Self::MoveDirect(location) => format!("Move directly to {location:?}"),
            Self::Encounter { location, hazard } => match hazard {
                mission::Hazard::Safe => format!("{location:?} turns out safe"),
                mission::Hazard::Injury => format!("Hazard at {location:?} injures the Scout"),
            },
            Self::Treat => "Medic treats the injury".into(),
            Self::Rescue(location) => format!("Rescue objective at {location:?}"),
            Self::Finish => "Complete the mission".into(),
        }
    }
}

impl StudioLabel for mission::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::Scout => "Scout",
            Self::Medic => "Medic",
            Self::Nature => "Nature",
            Self::Mission => "Mission",
            Self::Goal => "Goal",
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::{InstanceProfile, ReferenceService, StudyRequest};
    use axionomy::{Quantity, path};

    #[test]
    fn mission_replays_actual_search_and_private_information_transfer() {
        let mut request = StudyRequest::new(path!("mission")).with_instance(InstanceProfile::Micro);
        request.seed = 11;
        let study = ReferenceService.study(request).unwrap();
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
