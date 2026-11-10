use super::*;
use axionomy_problems::mission::expedition::{
    self, AGENTS, AccountId, Agent, Asset, LOCATIONS, Location, Policy, ROADS, RateId, World,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
) -> Result<Study, ServiceError> {
    let initial = expedition::initial(matches!(profile, InstanceProfile::Stress), study.seed);
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
        let (trace, final_world) =
            simulate("mission", &initial, |world| expedition::run(world, policy))?;
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
    assemble(study, descriptor, documents)
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
        let (supply, supply_label) = if agent == Agent::Scout {
            (Asset::Radio, "Radio charges")
        } else {
            (Asset::MedicalKit, "Medical kits")
        };
        entity.metrics = vec![
            asset_metric(
                slug("energy"),
                "Energy",
                &Asset::Energy,
                val(own, Asset::Energy),
            ),
            // Each point of health lost adds a tick to every journey.
            visual_metric(
                slug("injury"),
                "Injury delay",
                val(own, Asset::Harm),
                Some("ticks"),
            ),
            asset_metric(slug("capacity"), supply_label, &supply, val(own, supply)),
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
            asset_metric(
                slug("deadline"),
                "Deadline",
                &Asset::Deadline,
                val(account, Asset::Deadline),
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
        asset_metric(
            slug("time"),
            "Mission time",
            &Asset::Time,
            expedition::now(world),
        ),
        asset_metric(
            slug("complete"),
            "Objectives complete",
            &Asset::Completed,
            val(AccountId::World, Asset::Completed),
        ),
        asset_metric(
            slug("failed"),
            "Objectives lost",
            &Asset::FailedCount,
            val(AccountId::World, Asset::FailedCount),
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
        asset_metric(
            slug("radio"),
            "Radio charges spent",
            &Asset::UsedRadio,
            val(AccountId::Agent(Agent::Scout), Asset::UsedRadio),
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

// How Studio names and writes this problem's values.

/// An objective as it reads inside a sentence.
fn objective(id: u8) -> String {
    if expedition::patient(id) {
        format!("Casualty {}", id / 2 + 1)
    } else {
        "the research archive".into()
    }
}

/// An objective as it reads at the start of a sentence.
fn objective_subject(id: u8) -> String {
    if expedition::patient(id) {
        objective(id)
    } else {
        "Research archive".into()
    }
}

impl StudioFormat for expedition::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Time
            | Self::TimeRemaining
            | Self::Horizon
            | Self::ReadyAt
            | Self::Deadline
            | Self::NextClinical => QuantityFormat::COUNT.unit("ticks"),
            Self::Energy | Self::SpentEnergy => QuantityFormat::COUNT.unit("units"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for expedition::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Expedition identity".into(),
            Self::NatureIdentity => "Nature identity".into(),
            Self::AgentIdentity(agent) => format!("{agent:?} identity"),
            Self::ObjectiveIdentity(id) => format!("{} identity", objective_subject(*id)),
            Self::Active => "Mission in progress".into(),
            Self::Closed => "Mission closed".into(),
            Self::Time => "Mission time".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::Horizon => "Mission horizon".into(),
            Self::Count => "Objectives in the mission".into(),
            Self::Resolved => "Objectives resolved".into(),
            Self::Completed => "Objectives completed".into(),
            Self::FailedCount => "Objectives lost".into(),
            Self::SavedVitality => "Evacuated vitality".into(),
            Self::Permit(agent) => format!("{agent:?} may act this tick"),
            Self::UsedPermit(agent) => format!("{agent:?} has acted this tick"),
            Self::EnvironmentReady => "No weather change pending".into(),
            Self::WeatherDue(opening) => {
                if *opening {
                    "Hazard due to clear".into()
                } else {
                    "Hazard due to arrive".into()
                }
            }
            Self::HazardSite(location) => format!("Hazard site: {location:?}"),
            Self::WeatherOpen(location) => format!("{location:?} open"),
            Self::WeatherBlocked(location) => format!("{location:?} blocked by the hazard"),
            Self::At(location) => format!("At {location:?}"),
            Self::Travelling(location) => format!("Travelling to {location:?}"),
            Self::TeamAt(agent, location) => format!("{agent:?} at {location:?}"),
            Self::TeamTravelling(agent) => format!("{agent:?} travelling"),
            Self::Ready => "Ready".into(),
            Self::ReadyAt => "Busy until".into(),
            Self::Collecting(id) => format!("Recovering {}", objective(*id)),
            Self::Extracting(id) => format!("Extracting {}", objective(*id)),
            Self::AwaitingReport => "Awaiting a sensor report".into(),
            Self::Energy => "Energy".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::MedicalKit => "Medical kits".into(),
            Self::UsedMedicalKit => "Medical kits used".into(),
            Self::Health => "Health".into(),
            Self::Harm => "Health lost".into(),
            Self::Radio => "Radio charges".into(),
            Self::UsedRadio => "Radio charges spent".into(),
            Self::Sensor => "Sensor charges".into(),
            Self::UsedSensor => "Sensor charges used".into(),
            Self::ScanRound(round) => format!("Sensor reports so far: {round}"),
            Self::Signal(round, location) => {
                format!("Sensor report {} will suggest {location:?}", round + 1)
            }
            Self::Hint(round, location) => {
                format!("Sensor report {} suggests {location:?}", round + 1)
            }
            Self::Unsearched(location) => format!("{location:?} not yet surveyed"),
            Self::Surveyed(location) => format!("{location:?} surveyed"),
            Self::Known(id, location) => format!("Knows {} is at {location:?}", objective(*id)),
            Self::Shared(id, location) => {
                format!("Briefed Medic: {} at {location:?}", objective(*id))
            }
            Self::Truth(id, location) => {
                format!("{} is at {location:?}", objective_subject(*id))
            }
            Self::Deadline => "Deadline".into(),
            Self::Hidden => "Not yet found".into(),
            Self::Located => "Located".into(),
            Self::Working => "Recovery under way".into(),
            Self::Carried => "Being carried".into(),
            Self::Complete => "Delivered to base".into(),
            Self::Failed => "Lost".into(),
            Self::Stable => "Stabilized".into(),
            Self::Unstable => "Needs stabilization".into(),
            Self::ClinicalDue(day) => format!("Assessment due at tick {day}"),
            Self::ClinicalSettled(day) => format!("Assessed at tick {day}"),
            Self::NextClinical => "Next casualty assessment".into(),
            Self::ClinicalCount => "Casualties assessed this round".into(),
            Self::Cargo(id) => format!("Carrying {}", objective(*id)),
            Self::Seat => "Free evacuation seat".into(),
            Self::ReservedSeat(id) => format!("Seat reserved for {}", objective(*id)),
            Self::Supply => "Resupplies left".into(),
            Self::UsedSupply => "Resupplies used".into(),
        }
    }
}

impl StudioLabel for expedition::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Expedition",
            Self::Scout => "Scout",
            Self::Medic => "Medic",
            Self::Nature => "Nature",
            Self::Objective => "Objective",
        }
        .into()
    }
}

impl StudioLabel for expedition::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Team clock, weather and supplies".into(),
            Self::Nature => "Hidden mission sites and reports".into(),
            Self::Agent(agent) => format!("{agent:?}"),
            Self::Objective(id) => objective_subject(*id),
        }
    }
}

impl StudioLabel for expedition::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Depart { agent, from, to } => {
                format!("{agent:?} starts travel {from:?} → {to:?}")
            }
            Self::Arrive {
                agent,
                to,
                hazardous,
            } => format!(
                "{agent:?} arrives at {to:?}{}",
                if *hazardous {
                    " and is injured by the hazard"
                } else {
                    ""
                }
            ),
            Self::Survey(location) => format!("Scout surveys {location:?}"),
            Self::Discover {
                objective: id,
                location,
            } => format!("Scout discovers {} at {location:?}", objective(*id)),
            Self::Scan => "Scout requests a fallible sensor report".into(),
            Self::Report { round, location } => {
                format!("Private sensor report {} suggests {location:?}", round + 1)
            }
            Self::Share {
                objective: id,
                location,
                remote,
            } => format!(
                "Brief Medic on {} at {location:?} {}",
                objective(*id),
                if *remote {
                    "by radio"
                } else {
                    "at a rendezvous"
                }
            ),
            Self::Collect {
                objective: id,
                location,
            } => format!(
                "Scout starts a three-tick recovery of {} at {location:?}",
                objective(*id)
            ),
            Self::FinishCollect(id) => {
                format!("Scout finishes recovery and carries {}", objective(*id))
            }
            Self::TreatPatient {
                objective: id,
                location,
                restore,
            } => format!(
                "Medic stabilizes {} at {location:?}{}",
                objective(*id),
                if *restore {
                    " and restores 1 health"
                } else {
                    ""
                }
            ),
            Self::Extract {
                objective: id,
                location,
            } => format!(
                "Scout and Medic begin a two-tick extraction of {} at {location:?}",
                objective(*id)
            ),
            Self::FinishExtract(id) => {
                format!("Joint extraction ends; Medic carries {}", objective(*id))
            }
            Self::Deliver(id) => format!("Deliver {} safely to base", objective(*id)),
            Self::TreatAgent { target, location } => match target {
                expedition::Agent::Medic => {
                    format!("Medic treats their own injury at {location:?}")
                }
                expedition::Agent::Scout => {
                    format!("Medic treats Scout's injury at {location:?}")
                }
            },
            Self::Resupply(agent) => {
                format!(
                    "{agent:?} takes finite base energy{}",
                    if *agent == expedition::Agent::Medic {
                        " and a medical kit"
                    } else {
                        ""
                    }
                )
            }
            Self::Weather { location, opening } => format!(
                "{location:?} {}",
                if *opening {
                    "reopens as the hazard clears"
                } else {
                    "closes as the hazard arrives"
                }
            ),
            Self::Clinical {
                objective: id,
                day,
                active,
            } => {
                if *active {
                    format!("{} deteriorates at tick {day}", objective_subject(*id))
                } else {
                    format!(
                        "{} at tick {day}: outcome already settled",
                        objective_subject(*id)
                    )
                }
            }
            Self::FinishClinical(day) => format!("Finish the tick {day} casualty assessment"),
            Self::Fail {
                objective: id,
                phase,
                dead,
            } => format!(
                "{} {} {}",
                objective_subject(*id),
                if *dead {
                    "dies"
                } else {
                    "misses the mission deadline"
                },
                match phase {
                    expedition::Phase::Hidden => "before discovery",
                    expedition::Phase::Located => "at its site",
                    expedition::Phase::Carried => "while carried",
                }
            ),
            Self::Tick(day) => format!("Advance mission time to tick {}", day + 1),
            Self::Close => "Close the mission with every objective accounted for".into(),
        }
    }
}
