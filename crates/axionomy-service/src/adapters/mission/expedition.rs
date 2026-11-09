use super::*;
use axionomy_problems::mission::expedition::{
    self, AGENTS, AccountId, Agent, Asset, LOCATIONS, Location, Policy, ROADS, RateId, World,
};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
) -> Result<RunArtifact, ServiceError> {
    let initial = expedition::initial(matches!(profile, InstanceProfile::Stress), run.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            "coordinated",
            "Mission · independent agents, shared plan",
            "Scout discovers sites and spends scarce radio charges to brief Medic. Independent travel overlaps; extraction requires both teammates, current stabilization and a free evacuation seat.",
            Policy::RadioCoordination,
        ),
        (
            "direct-north",
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
        let trace = expedition::run(&initial, policy);
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
            &expedition::goal(),
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
        view.proposals.push(proposal(ProposalSpec { id: "extract-without-intelligence", label: "Extract a casualty before discovery or briefing", description: "Both agents need the confirmed site, physical co-location, free work capacity, energy, and a stabilized patient. An unverified sensor hint cannot satisfy these conditions." }, &initial, &expedition::action(RateId::Extract { objective: 0, location: Location::North })));
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
    artifact(run, descriptor, documents)
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    let val = |account, asset| expedition::value(world, account, asset);
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
            expedition::now(world),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView {
        key: slug(key),
        label: label.into(),
        value: value.to_string(),
        direction,
    })
    .collect()
}
fn observation(world: &World, agent: Agent) -> ObservationView {
    let key = expedition::agent_view(world, agent).observation_key();
    ObservationView {
        actor: ViewId::new(
            slug(format!("mission/actor/{agent:?}")),
            format!("{agent:?}"),
        ),
        label: format!(
            "{agent:?}'s private observations at tick {}",
            expedition::now(world)
        ),
        visible_accounts: key
            .visible_accounts()
            .iter()
            .map(|account| AccountView {
                account: ViewId::new(
                    account_key::<expedition::Ontology>(account),
                    account.studio_label(),
                ),
                balances: key
                    .balances()
                    .iter()
                    .filter(|(owner, _, _)| owner == account)
                    .map(|(_, asset, quantity)| AssetQuantityView {
                        asset: ViewId::new(
                            asset_key::<expedition::Ontology>(asset),
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
                asset: ViewId::new(
                    asset_key::<expedition::Ontology>(asset),
                    asset.studio_label(),
                ),
                quantity: ExactQuantity(quantity.to_string()),
            })
            .collect(),
    }
}
fn scene(_: u64, world: &World) -> Option<Scene> {
    let val = |account, asset| expedition::value(world, account, asset);
    let nodes = [
        (Location::Base, 400.0, 300.0),
        (Location::North, 140.0, 100.0),
        (Location::East, 730.0, 100.0),
        (Location::South, 730.0, 520.0),
        (Location::West, 140.0, 520.0),
    ]
    .into_iter()
    .map(|(location, x, y)| GraphNodeView {
        id: ViewId::new(slug(format!("{location:?}")), format!("{location:?}")),
        classes: vec![slug("location")],
        x: Some(x),
        y: Some(y),
    })
    .collect();
    let edges = ROADS
        .into_iter()
        .map(|(from, to)| GraphEdgeView {
            id: slug(format!("road/{from:?}/{to:?}")),
            source: slug(format!("{from:?}")),
            target: slug(format!("{to:?}")),
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
                node: slug(format!("{at:?}")),
            }
        } else {
            SceneAnchorView::Unanchored
        };
        let mut entity = link_account::<expedition::Ontology>(
            visual_entity(
                slug(format!("agent/{agent:?}")),
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
            &own,
        );
        entity.metrics = vec![
            visual_metric(slug("energy"), "Energy", val(own, Asset::Energy), None),
            visual_metric(
                slug("injury"),
                "Injury delay",
                val(own, Asset::Harm),
                Some("ticks"),
            ),
            visual_metric(
                slug("capacity"),
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
        let carrier = if expedition::patient(id) {
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
                node: slug(format!("{location:?}")),
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
        } else if expedition::patient(id) && val(account, Asset::Stable) == 0 {
            "Needs stabilization"
        } else {
            "Ready for recovery"
        };
        let mut entity = link_account::<expedition::Ontology>(
            visual_entity(
                slug(format!("objective/{id}")),
                account.studio_label(),
                if expedition::patient(id) {
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
            &account,
        );
        entity.metrics = vec![
            visual_metric(
                slug("knowledge"),
                "Medic briefed",
                if val(AccountId::Agent(Agent::Medic), Asset::Known(id, site)) > 0 {
                    "yes"
                } else {
                    "no"
                },
                None,
            ),
            visual_metric(
                slug("deadline"),
                "Deadline",
                val(account, Asset::Deadline),
                None,
            ),
        ];
        if expedition::patient(id) {
            entity.metrics.insert(
                0,
                visual_metric(
                    slug("health"),
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
        visual_metric(slug("time"), "Mission time", expedition::now(world), None),
        visual_metric(
            slug("complete"),
            "Objectives complete",
            val(AccountId::World, Asset::Completed),
            None,
        ),
        visual_metric(
            slug("failed"),
            "Objectives lost",
            val(AccountId::World, Asset::FailedCount),
            None,
        ),
        visual_metric(
            slug("busy"),
            "Teammates occupied",
            AGENTS
                .into_iter()
                .filter(|agent| val(AccountId::Agent(*agent), Asset::Ready) == 0)
                .count(),
            Some("/ 2"),
        ),
        visual_metric(
            slug("radio"),
            "Radio charges spent",
            val(AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            None,
        ),
    ]);
    for location in LOCATIONS {
        if let Some(entity) = scene
            .entities
            .iter_mut()
            .find(|entity| entity.id.key == slug(format!("{location:?}")))
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
