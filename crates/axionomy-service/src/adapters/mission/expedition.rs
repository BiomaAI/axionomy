use super::*;
use axionomy_problems::mission::expedition::{
    self as mission, AGENTS, AccountId, Agent, Asset, LOCATIONS, Location, Policy, ROADS, RateId,
    World,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
) -> Result<RunArtifact, ServiceError> {
    let initial = mission::initial(matches!(profile, InstanceProfile::Stress), request.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            "coordinated",
            "Mission · independent agents, shared plan",
            "Scout discovers sites and spends scarce radio charges to brief Medic. Independent travel overlaps; extraction requires both teammates, current stabilization and a free evacuation seat.",
            Policy::RadioCoordination,
        ),
        (
            "direct_north",
            "Mission · north-first sector search",
            "Scout starts north without sensing. Intelligence travels through in-person briefings, and each teammate responds to the same changing hazards and clinical deadlines.",
            Policy::NorthFirst,
        ),
        (
            "rendezvous",
            "Mission · rendezvous before deployment",
            "Save radio charges by meeting at base to share confirmed discoveries. The extra journeys compete with archive recovery and timely casualty evacuation.",
            Policy::Rendezvous,
        ),
    ] {
        let trace = mission::run(&initial, policy);
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("mission", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "mission",
                strategy,
                title,
                description,
                source_label: "Cooperative expedition",
            },
            &initial,
            &mission::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("mission", error))?;
        view.observations = AGENTS
            .into_iter()
            .map(|agent| observation(&initial, agent))
            .collect();
        let mut observed = initial.clone();
        for (frame, exchange) in view.frames.iter_mut().zip(trace.exchanges()) {
            observed
                .apply(exchange.clone())
                .map_err(|error| problem_error("mission", error))?;
            frame.observations = AGENTS
                .into_iter()
                .map(|agent| observation(&observed, agent))
                .collect();
        }
        view.telemetry.push(telemetry(
            "separate observation-responsive teammate policies",
            true,
            [(
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "verified timed work, communication and environmental events".into(),
            )],
        ));
        view.proposals.push(proposal("mission", ProposalSpec { id: "extract-without-intelligence", label: "Extract a casualty before discovery or briefing", description: "Both agents need the confirmed site, physical co-location, free work capacity, energy, and a stabilized patient. An unverified sensor hint cannot satisfy these conditions." }, &initial, &mission::action(RateId::Extract { objective: 0, location: Location::North })));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = evaluated_integer_front(
            &documents,
            &documents[index],
            "Team outcomes on the same hidden sites and hazards",
        );
        documents[index].pareto_fronts.push(front);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    let val = |account, asset| mission::value(world, account, asset);
    [
        (
            "completed",
            "Objectives completed",
            val(AccountId::World, Asset::Completed),
            ObjectiveDirectionView::Maximize,
        ),
        (
            "failed",
            "Objectives lost",
            val(AccountId::World, Asset::FailedCount),
            ObjectiveDirectionView::Minimize,
        ),
        (
            "vitality",
            "Evacuated vitality",
            val(AccountId::World, Asset::SavedVitality),
            ObjectiveDirectionView::Maximize,
        ),
        (
            "radio",
            "Radio charges spent",
            val(AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            ObjectiveDirectionView::Minimize,
        ),
        (
            "energy",
            "Team energy spent",
            AGENTS
                .into_iter()
                .map(|agent| val(AccountId::Agent(agent), Asset::SpentEnergy))
                .sum(),
            ObjectiveDirectionView::Minimize,
        ),
        (
            "time",
            "Elapsed mission time",
            mission::now(world),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView {
        key: key.into(),
        label: label.into(),
        value: value.to_string(),
        direction,
    })
    .collect()
}
fn observation(world: &World, agent: Agent) -> ObservationView {
    let key = mission::agent_view(world, agent).observation_key();
    ObservationView {
        actor: ViewId::new(format!("mission:actor:{agent:?}"), format!("{agent:?}")),
        label: format!(
            "{agent:?}'s private observations at tick {}",
            mission::now(world)
        ),
        visible_accounts: key
            .visible_accounts()
            .iter()
            .map(|account| AccountView {
                account: ViewId::new(
                    format!("mission:observed:{account:?}"),
                    account.studio_label(),
                ),
                balances: key
                    .balances()
                    .iter()
                    .filter(|(owner, _, _)| owner == account)
                    .map(|(_, asset, quantity)| AssetQuantityView {
                        asset: ViewId::new(
                            format!("mission:observed:{asset:?}"),
                            asset.studio_label(),
                        ),
                        quantity: ExactQuantity(quantity.to_string()),
                    })
                    .collect(),
            })
            .collect(),
        facts: key
            .balances()
            .iter()
            .filter(|(owner, asset, _)| {
                *owner == AccountId::Agent(agent)
                    && matches!(
                        asset,
                        Asset::Known(..)
                            | Asset::Shared(..)
                            | Asset::Hint(..)
                            | Asset::Harm
                            | Asset::UsedRadio
                    )
            })
            .map(|(_, asset, quantity)| AssetQuantityView {
                asset: ViewId::new(format!("mission:fact:{asset:?}"), asset.studio_label()),
                quantity: ExactQuantity(quantity.to_string()),
            })
            .collect(),
    }
}
fn scene(_: u64, world: &World) -> Option<Scene> {
    let val = |account, asset| mission::value(world, account, asset);
    let nodes = [
        (Location::Base, 400.0, 300.0),
        (Location::North, 140.0, 100.0),
        (Location::East, 730.0, 100.0),
        (Location::South, 730.0, 520.0),
        (Location::West, 140.0, 520.0),
    ]
    .into_iter()
    .map(|(location, x, y)| GraphNodeView {
        id: ViewId::new(format!("{location:?}"), format!("{location:?}")),
        classes: vec!["location".into()],
        x: Some(x),
        y: Some(y),
    })
    .collect();
    let edges = ROADS
        .into_iter()
        .map(|(from, to)| GraphEdgeView {
            id: format!("road:{from:?}:{to:?}"),
            source: format!("{from:?}"),
            target: format!("{to:?}"),
            label: Some("1 tick + injury delay".into()),
            classes: vec![],
        })
        .collect();
    let mut entities = Vec::new();
    for agent in AGENTS {
        let own = AccountId::Agent(agent);
        let moving = LOCATIONS
            .into_iter()
            .find(|location| val(own, Asset::Travelling(*location)) > 0);
        let at = LOCATIONS
            .into_iter()
            .find(|location| val(own, Asset::At(*location)) > 0);
        let status = if let Some(to) = moving {
            format!(
                "Travelling to {to:?} · arrives {}",
                val(own, Asset::ReadyAt)
            )
        } else if val(own, Asset::AwaitingReport) > 0 {
            "Awaiting private sensor report".into()
        } else if (0..3).any(|id| val(own, Asset::Extracting(id)) > 0) {
            format!("Joint extraction until {}", val(own, Asset::ReadyAt))
        } else if (0..3).any(|id| val(own, Asset::Collecting(id)) > 0) {
            format!("Archive recovery until {}", val(own, Asset::ReadyAt))
        } else if (0..3).any(|id| val(own, Asset::Cargo(id)) > 0) {
            "Carrying objective to base".into()
        } else if val(own, Asset::Harm) > 0 {
            "Ready · injury slows travel".into()
        } else {
            "Ready".into()
        };
        let anchor = if let Some(at) = at {
            SceneAnchorView::GraphNode {
                node: format!("{at:?}"),
            }
        } else {
            SceneAnchorView::Unanchored
        };
        let mut entity = link_account(
            visual_entity(
                format!("agent:{agent:?}"),
                format!("{agent:?}"),
                if agent == Agent::Scout {
                    SceneGlyphView::Sensor
                } else {
                    SceneGlyphView::Person
                },
                anchor,
                if at.is_some() {
                    SceneEntityRoleView::Occupant
                } else {
                    SceneEntityRoleView::Context
                },
                if val(own, Asset::Harm) > 0 {
                    SceneToneView::Warning
                } else {
                    SceneToneView::Active
                },
                Some(status),
            ),
            format!(
                "mission:account:agent-{}",
                if agent == Agent::Scout {
                    "scout"
                } else {
                    "medic"
                }
            ),
        );
        entity.metrics = vec![
            visual_metric("energy", "Energy", val(own, Asset::Energy), None),
            visual_metric(
                "injury",
                "Injury delay",
                val(own, Asset::Harm),
                Some("ticks"),
            ),
            visual_metric(
                "capacity",
                if agent == Agent::Scout {
                    "Radio charges"
                } else {
                    "Medical kits"
                },
                val(
                    own,
                    if agent == Agent::Scout {
                        Asset::Radio
                    } else {
                        Asset::MedicalKit
                    },
                ),
                None,
            ),
        ];
        entities.push(entity);
    }
    for id in 0..val(AccountId::World, Asset::Count) as u8 {
        let Some(site) = LOCATIONS
            .into_iter()
            .find(|location| val(AccountId::Agent(Agent::Scout), Asset::Known(id, *location)) > 0)
        else {
            continue;
        };
        let account = AccountId::Objective(id);
        let complete = val(account, Asset::Complete) > 0;
        let failed = val(account, Asset::Failed) > 0;
        let carrying = val(account, Asset::Carried) > 0;
        let carrier = if mission::patient(id) {
            Agent::Medic
        } else {
            Agent::Scout
        };
        let node = if complete {
            Some(Location::Base)
        } else if carrying {
            LOCATIONS
                .into_iter()
                .find(|location| val(AccountId::Agent(carrier), Asset::At(*location)) > 0)
        } else {
            Some(site)
        };
        let anchor = node.map_or(SceneAnchorView::Unanchored, |location| {
            SceneAnchorView::GraphNode {
                node: format!("{location:?}"),
            }
        });
        let status = if complete {
            "Delivered at base"
        } else if failed {
            "Objective lost"
        } else if carrying {
            "Carried by teammate"
        } else if val(account, Asset::Working) > 0 {
            "Recovery in progress"
        } else if mission::patient(id) && val(account, Asset::Stable) == 0 {
            "Needs stabilization"
        } else {
            "Ready for recovery"
        };
        let mut entity = link_account(
            visual_entity(
                format!("objective:{id}"),
                account.studio_label(),
                if mission::patient(id) {
                    SceneGlyphView::Person
                } else {
                    SceneGlyphView::Information
                },
                anchor,
                if node.is_some() {
                    SceneEntityRoleView::Attachment
                } else {
                    SceneEntityRoleView::Context
                },
                if complete {
                    SceneToneView::Success
                } else if failed {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Warning
                },
                Some(status.into()),
            ),
            format!("mission:account:objective-{id}"),
        );
        entity.metrics = vec![
            visual_metric(
                "knowledge",
                "Medic briefed",
                if val(AccountId::Agent(Agent::Medic), Asset::Known(id, site)) > 0 {
                    "yes"
                } else {
                    "no"
                },
                None,
            ),
            visual_metric("deadline", "Deadline", val(account, Asset::Deadline), None),
        ];
        if mission::patient(id) {
            entity.metrics.insert(
                0,
                visual_metric(
                    "health",
                    "Vitality",
                    val(account, Asset::Health),
                    Some("/ 3"),
                ),
            );
        }
        entities.push(entity);
    }
    let mut scene = Scene::graph(
        "Independent agents, scarce communication and evolving hazards",
        nodes,
        edges,
        None,
    )
    .with_entities(entities)
    .with_metrics([
        visual_metric("time", "Mission time", mission::now(world), None),
        visual_metric(
            "complete",
            "Objectives complete",
            val(AccountId::World, Asset::Completed),
            None,
        ),
        visual_metric(
            "failed",
            "Objectives lost",
            val(AccountId::World, Asset::FailedCount),
            None,
        ),
        visual_metric(
            "busy",
            "Teammates occupied",
            AGENTS
                .into_iter()
                .filter(|agent| val(AccountId::Agent(*agent), Asset::Ready) == 0)
                .count(),
            Some("/ 2"),
        ),
        visual_metric(
            "radio",
            "Radio charges spent",
            val(AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            None,
        ),
    ]);
    for location in LOCATIONS {
        if let Some(entity) = scene
            .entities
            .iter_mut()
            .find(|entity| entity.id.key == format!("{location:?}"))
        {
            entity.status = Some(
                if val(AccountId::World, Asset::WeatherBlocked(location)) > 0 {
                    "Hazard · departures toward this sector blocked"
                } else if location == Location::Base {
                    "Hospital, archive delivery and finite resupply"
                } else if val(AccountId::Agent(Agent::Scout), Asset::Surveyed(location)) > 0 {
                    "Surveyed"
                } else {
                    "Unsearched"
                }
                .into(),
            );
            if val(AccountId::World, Asset::WeatherBlocked(location)) > 0 {
                entity.tone = SceneToneView::Danger;
            }
        }
    }
    Some(scene)
}
