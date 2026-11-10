use super::*;
use axionomy_problems::rescue::incident::{
    self, AccountId, Asset, LOCATIONS, Location, Policy, RateId, World,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let initial = incident::initial(matches!(profile, InstanceProfile::Stress), study.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            "observe-then-follow",
            "Rescue · responsive triage",
            "Spend limited sensor charges on fallible hints, search sectors, prioritize observed casualties, and choose treatment or rapid evacuation against continuing deterioration.",
            Policy::Triage,
        ),
        (
            "direct-north",
            "Rescue · rapid sector sweep",
            "Begin north without a scan, search successive sectors, and use fast transport. Each rescued patient occupies the only evacuation seat until hospital arrival.",
            Policy::RapidNorth,
        ),
        (
            "stabilize-first",
            "Rescue · stabilize before transport",
            "Treat discovered patients before loading when a kit remains. Restored vitality and delayed deterioration cost time that other casualties may not have.",
            Policy::StabilizeFirst,
        ),
    ] {
        let (trace, final_world) =
            simulate("rescue", &initial, |world| incident::run(world, policy))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: "rescue",
                strategy,
                title,
                description,
                source_label: "Multi-patient incident",
            },
            &initial,
            &incident::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("rescue", error))?;
        view.observations.push(observation(&initial));
        let mut observed = initial.clone();
        for (frame, exchange) in view.frames.iter_mut().zip(trace.exchanges()) {
            observed
                .apply(exchange.clone())
                .map_err(|error| problem_error("rescue", error))?;
            frame.observations = vec![observation(&observed)];
        }
        view.telemetry.push(telemetry(
            "observation-responsive incident policy",
            true,
            [(
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "verified decisions and environmental responses".into(),
            )],
        ));
        view.proposals.push(proposal(ProposalSpec { id: "undiscovered-patient", label: "Load a patient before finding them", description: "A location hint is insufficient. Physical discovery, co-location, current clinical observations, energy, and an empty evacuation seat are required." }, &initial, &incident::action(RateId::Load { patient: 0, location: Location::North })));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = evaluated_integer_front(
            &documents,
            &documents[index],
            "Incident outcomes on the same hidden scenario",
        );
        documents[index].pareto_fronts.push(front);
    }
    assemble(study, descriptor, documents)
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    [
        (
            "saved",
            "Patients evacuated",
            AccountId::World,
            Asset::SavedCount,
            ObjectiveDirectionView::Maximize,
        ),
        (
            "vitality",
            "Vitality at hospital arrival",
            AccountId::World,
            Asset::SavedVitality,
            ObjectiveDirectionView::Maximize,
        ),
        (
            "deaths",
            "Patients lost",
            AccountId::World,
            Asset::DeathCount,
            ObjectiveDirectionView::Minimize,
        ),
        (
            "fuel",
            "Fuel spent",
            AccountId::Responder,
            Asset::SpentFuel,
            ObjectiveDirectionView::Minimize,
        ),
        (
            "medicine",
            "Kits used",
            AccountId::Responder,
            Asset::UsedMedicalKit,
            ObjectiveDirectionView::Minimize,
        ),
        (
            "time",
            "Elapsed time",
            AccountId::World,
            Asset::Time,
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, account, asset, direction)| ObjectiveView {
        key: slug(key),
        label: label.into(),
        value: incident::value(world, account, asset).to_string(),
        direction,
    })
    .collect()
}
fn observation(world: &World) -> ObservationView {
    let key = incident::responder_view(world).observation_key();
    ObservationView {
        actor: ViewId::new(slug("rescue/actor/responder"), "Rescue crew"),
        label: format!(
            "Crew observations at time {} · undiscovered patients and Nature omitted",
            incident::now(world)
        ),
        visible_accounts: key
            .visible_accounts()
            .iter()
            .map(|account| AccountView {
                account: ViewId::new(
                    account_key::<incident::Ontology>(account),
                    account.studio_label(),
                ),
                balances: key
                    .balances()
                    .iter()
                    .filter(|(owner, _, _)| owner == account)
                    .map(|(_, asset, quantity)| AssetQuantityView {
                        asset: ViewId::new(
                            asset_key::<incident::Ontology>(asset),
                            asset.studio_label(),
                        ),
                        quantity: ExactQuantity(quantity.to_string()),
                    })
                    .collect(),
            })
            .collect(),
        facts: Vec::new(),
    }
}
fn scene(_: u64, world: &World) -> Option<Scene> {
    let val = |account, asset| incident::value(world, account, asset);
    let present =
        |location| location == Location::Base || val(AccountId::World, Asset::Site(location)) > 0;
    let locations = [
        (Location::Base, 420.0, 280.0),
        (Location::North, 120.0, 80.0),
        (Location::East, 720.0, 80.0),
        (Location::Harbor, 720.0, 480.0),
        (Location::South, 120.0, 480.0),
        (Location::West, 420.0, 650.0),
    ];
    let nodes = locations
        .into_iter()
        .filter(|(location, _, _)| present(*location))
        .map(|(location, x, y)| GraphNodeView {
            id: ViewId::new(
                path::write(&location),
                if location == Location::Base {
                    "Base hospital".into()
                } else {
                    format!("{location:?}")
                },
            ),
            classes: vec![slug("location")],
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let roads = [
        (Location::Base, Location::North),
        (Location::Base, Location::South),
        (Location::Base, Location::Harbor),
        (Location::North, Location::East),
        (Location::East, Location::Harbor),
        (Location::South, Location::West),
        (Location::West, Location::Harbor),
    ];
    let edges = roads
        .into_iter()
        .filter(|(a, b)| present(*a) && present(*b))
        .map(|(a, b)| GraphEdgeView {
            id: path!("road").then(&a).then(&b),
            source: path::write(&a),
            target: path::write(&b),
            label: Some("2 ticks / 1 fuel · fast 1 / 2".into()),
            classes: vec![],
        })
        .collect();
    let at = LOCATIONS
        .into_iter()
        .find(|location| val(AccountId::Responder, Asset::At(*location)) > 0)
        .unwrap_or(Location::Base);
    let mut crew = link_account::<incident::Ontology>(
        visual_entity(
            slug("crew"),
            "Evacuation crew",
            SceneGlyphView::Vehicle,
            SceneAnchorView::GraphNode {
                node: path::write(&at),
            },
            SceneEntityRoleView::Occupant,
            SceneToneView::Active,
            Some(
                if val(AccountId::Responder, Asset::Seat) == 0 {
                    "Transporting one patient"
                } else if val(AccountId::Responder, Asset::AwaitingScan) > 0 {
                    "Awaiting sensor report"
                } else {
                    "Evacuation seat available"
                }
                .into(),
            ),
        ),
        &AccountId::Responder,
    );
    crew.metrics = vec![
        asset_metric(
            slug("fuel"),
            "Fuel",
            &Asset::Fuel,
            val(AccountId::Responder, Asset::Fuel),
        ),
        asset_metric(
            slug("energy"),
            "Crew energy",
            &Asset::Energy,
            val(AccountId::Responder, Asset::Energy),
        ),
        asset_metric(
            slug("medicine"),
            "Medical kits",
            &Asset::MedicalKit,
            val(AccountId::Responder, Asset::MedicalKit),
        ),
    ];
    let mut entities = vec![crew];
    for id in 0..val(AccountId::World, Asset::PatientCount) as u8 {
        let Some(site) = LOCATIONS
            .into_iter()
            .find(|location| val(AccountId::World, Asset::Known(id, *location)) > 0)
        else {
            continue;
        };
        let patient = AccountId::Patient(id);
        let saved = val(patient, Asset::Saved) > 0;
        let dead = val(patient, Asset::Dead) > 0;
        let node = if saved {
            Location::Base
        } else if val(patient, Asset::Onboard) > 0 {
            at
        } else {
            site
        };
        let status = if saved {
            "Evacuated"
        } else if dead {
            "Died"
        } else if val(patient, Asset::LeftBehind) > 0 {
            "Left behind"
        } else if val(patient, Asset::Onboard) > 0 {
            "In transit"
        } else if val(patient, Asset::Stabilized) > 0 {
            "Stabilized · awaiting transport"
        } else {
            "Found · awaiting care"
        };
        let mut entity = link_account::<incident::Ontology>(
            visual_entity(
                path!("patient").then(&id),
                format!("Patient {}", id + 1),
                SceneGlyphView::Person,
                SceneAnchorView::GraphNode {
                    node: path::write(&node),
                },
                SceneEntityRoleView::Attachment,
                if saved {
                    SceneToneView::Success
                } else if dead || val(patient, Asset::Vitality) <= 1 {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Warning
                },
                Some(status.into()),
            ),
            &patient,
        );
        entity.metrics = vec![
            visual_metric(
                slug("vitality"),
                "Vitality",
                val(patient, Asset::Vitality),
                Some("/ 3"),
            ),
            asset_metric(
                slug("decline"),
                "Next deterioration",
                &Asset::NextDecline,
                val(patient, Asset::NextDecline),
            ),
            asset_metric(
                slug("deadline"),
                "Evacuation deadline",
                &Asset::Deadline,
                val(patient, Asset::Deadline),
            ),
        ];
        entities.push(entity);
    }
    let mut scene = Scene::graph(
        "Rescue incident · physical roads and observed casualties",
        nodes,
        edges,
        None,
    )
    .with_entities(entities)
    .with_metrics([
        asset_metric(
            slug("time"),
            "Incident time",
            &Asset::Time,
            incident::now(world),
        ),
        asset_metric(
            slug("saved"),
            "Patients evacuated",
            &Asset::SavedCount,
            val(AccountId::World, Asset::SavedCount),
        ),
        asset_metric(
            slug("deaths"),
            "Patients lost",
            &Asset::DeathCount,
            val(AccountId::World, Asset::DeathCount),
        ),
        asset_metric(
            slug("fuel"),
            "Fuel remaining",
            &Asset::Fuel,
            val(AccountId::Responder, Asset::Fuel),
        ),
    ]);
    for location in LOCATIONS.into_iter().filter(|location| present(*location)) {
        if let Some(entity) = scene
            .entities
            .iter_mut()
            .find(|entity| entity.id.key == path::write(&location))
        {
            entity.status = Some(
                if location == Location::Base {
                    if val(AccountId::World, Asset::Closed) > 0 {
                        "Incident closed"
                    } else {
                        "Hospital and one finite resupply"
                    }
                } else if val(AccountId::Responder, Asset::Surveyed(location)) > 0 {
                    "Searched"
                } else if val(AccountId::Responder, Asset::Hint(location)) > 0 {
                    "Unverified sensor hint"
                } else {
                    "Unsearched"
                }
                .into(),
            );
            if location == Location::Base {
                entity.metrics = vec![
                    asset_metric(
                        slug("time"),
                        "Incident time",
                        &Asset::Time,
                        incident::now(world),
                    ),
                    asset_metric(
                        slug("saved"),
                        "Patients evacuated",
                        &Asset::SavedCount,
                        val(AccountId::World, Asset::SavedCount),
                    ),
                    asset_metric(
                        slug("lost"),
                        "Patients lost",
                        &Asset::DeathCount,
                        val(AccountId::World, Asset::DeathCount),
                    ),
                ];
            }
        }
    }
    Some(scene)
}

// How Studio names and writes this problem's values.

impl StudioFormat for incident::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Time
            | Self::Horizon
            | Self::TimeRemaining
            | Self::NextDecline
            | Self::Deadline => QuantityFormat::COUNT.unit("ticks"),
            Self::Fuel | Self::SpentFuel | Self::Energy | Self::SpentEnergy => {
                QuantityFormat::COUNT.unit("units")
            }
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for incident::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Incident identity".into(),
            Self::ResponderIdentity => "Crew identity".into(),
            Self::NatureIdentity => "Nature identity".into(),
            Self::PatientIdentity(id) => format!("Patient {} identity", id + 1),
            Self::Active => "Incident in progress".into(),
            Self::Closed => "Incident closed".into(),
            Self::Time => "Incident time".into(),
            Self::Horizon => "Incident horizon".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::PatientCount => "Patients in the incident".into(),
            Self::Resolved => "Outcomes recorded".into(),
            Self::SavedCount => "Patients evacuated".into(),
            Self::DeathCount => "Patients who died".into(),
            Self::LeftCount => "Patients left behind".into(),
            Self::SavedVitality => "Vitality at hospital arrival".into(),
            Self::At(location) => format!("At {location:?}"),
            Self::Site(location) => format!("Incident site: {location:?}"),
            Self::Surveyed(location) => format!("{location:?} searched"),
            Self::Unsearched(location) => format!("{location:?} not yet searched"),
            Self::Known(id, location) => format!("Patient {} found at {location:?}", id + 1),
            Self::Truth(id, location) => format!("Patient {} is at {location:?}", id + 1),
            Self::Ready => "Crew ready".into(),
            Self::AwaitingScan => "Awaiting a sensor report".into(),
            Self::Sensor => "Sensor charges".into(),
            Self::UsedSensor => "Sensor charges used".into(),
            Self::ScanRound(round) => format!("Sensor reports so far: {round}"),
            Self::Signal(round, location) => {
                format!("Sensor report {} will suggest {location:?}", round + 1)
            }
            Self::Hint(location) => format!("Sensor hint: {location:?}"),
            Self::Fuel => "Fuel".into(),
            Self::SpentFuel => "Fuel spent".into(),
            Self::Energy => "Energy".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::MedicalKit => "Medical kits".into(),
            Self::UsedMedicalKit => "Medical kits used".into(),
            Self::SupplyAvailable => "Resupply available".into(),
            Self::SupplyUsed => "Resupply used".into(),
            Self::Seat => "Free evacuation seat".into(),
            Self::Carrying(id) => format!("Carrying Patient {}", id + 1),
            Self::Hidden => "Not yet found".into(),
            Self::Found => "Found".into(),
            Self::Onboard => "On board".into(),
            Self::Alive => "Alive".into(),
            Self::Saved => "Evacuated".into(),
            Self::Dead => "Died".into(),
            Self::LeftBehind => "Left behind".into(),
            Self::Vitality => "Vitality".into(),
            Self::Harm => "Vitality lost".into(),
            Self::NextDecline => "Next deterioration".into(),
            Self::Deadline => "Evacuation deadline".into(),
            Self::Treatable => "Can be stabilized".into(),
            Self::Stabilized => "Stabilized".into(),
        }
    }
}

impl StudioLabel for incident::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Incident",
            Self::Responder => "Crew",
            Self::Nature => "Nature",
            Self::Patient => "Patient",
        }
        .into()
    }
}

impl StudioLabel for incident::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Incident clock and hospital".into(),
            Self::Responder => "Evacuation crew".into(),
            Self::Nature => "Hidden patient locations and sensor reports".into(),
            Self::Patient(id) => format!("Patient {}", id + 1),
        }
    }
}

impl StudioLabel for incident::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Drive { from, to, fast } => format!(
                "Drive {from:?} → {to:?}{}",
                if *fast {
                    " at emergency speed"
                } else {
                    " conserving fuel"
                }
            ),
            Self::Search(location) => format!("Search {location:?} for casualties"),
            Self::Scan => "Spend a sensor charge on a new location hint".into(),
            Self::Report { round, location } => format!(
                "Sensor report {} suggests {location:?}; hint may be wrong",
                round + 1
            ),
            Self::Discover { patient, location } => {
                format!("Discover Patient {} at {location:?}", patient + 1)
            }
            Self::Deteriorate(patient) => format!("Patient {} loses vitality", patient + 1),
            Self::Stabilize {
                patient,
                location,
                restore,
            } => format!(
                "Stabilize Patient {} at {location:?}, {}",
                patient + 1,
                if *restore {
                    "restoring 1 vitality and extending the window"
                } else {
                    "extending the evacuation window"
                }
            ),
            Self::Load { patient, location } => format!(
                "Load Patient {} at {location:?} into the evacuation seat",
                patient + 1
            ),
            Self::Unload(patient) => format!("Admit Patient {} to the hospital", patient + 1),
            Self::Lose {
                patient,
                phase,
                cause,
            } => format!(
                "Patient {} {} {}: {}",
                patient + 1,
                if *cause == incident::Loss::Horizon {
                    "is left behind"
                } else {
                    "dies"
                },
                match phase {
                    incident::Phase::Hidden => "unfound",
                    incident::Phase::Found => "at the scene",
                    incident::Phase::Onboard => "on board",
                },
                match cause {
                    incident::Loss::Deterioration => "no vitality left",
                    incident::Loss::Deadline => "deadline passed",
                    incident::Loss::Horizon => "incident over",
                }
            ),
            Self::Resupply => "Take the hospital's finite fuel, energy and medical reserve".into(),
            Self::Wait => "Advance incident time by one tick".into(),
            Self::Close => "Close the incident with all patient outcomes recorded".into(),
        }
    }
}
