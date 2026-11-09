use super::*;
use axionomy_problems::rescue::incident::{
    self as rescue, AccountId, Asset, LOCATIONS, Location, Policy, RateId, World,
};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    profile: InstanceProfile,
) -> Result<RunArtifact, ServiceError> {
    let initial = rescue::initial(matches!(profile, InstanceProfile::Stress), run.seed);
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
        let trace = rescue::run(&initial, policy);
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("rescue", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "rescue",
                strategy,
                title,
                description,
                source_label: "Multi-patient incident",
            },
            &initial,
            &rescue::goal(),
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
        view.proposals.push(proposal(ProposalSpec { id: "undiscovered-patient", label: "Load a patient before finding them", description: "A location hint is insufficient. Physical discovery, co-location, current clinical observations, energy, and an empty evacuation seat are required." }, &initial, &rescue::action(RateId::Load { patient: 0, location: Location::North })));
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
    artifact(run, descriptor, documents)
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
        value: rescue::value(world, account, asset).to_string(),
        direction,
    })
    .collect()
}
fn observation(world: &World) -> ObservationView {
    let key = rescue::responder_view(world).observation_key();
    ObservationView {
        actor: ViewId::new(slug("rescue/actor/responder"), "Rescue crew"),
        label: format!(
            "Crew observations at time {} · undiscovered patients and Nature omitted",
            rescue::now(world)
        ),
        visible_accounts: key
            .visible_accounts()
            .iter()
            .map(|account| AccountView {
                account: ViewId::new(
                    slug(format!("rescue/observed/{account:?}")),
                    account.studio_label(),
                ),
                balances: key
                    .balances()
                    .iter()
                    .filter(|(owner, _, _)| owner == account)
                    .map(|(_, asset, quantity)| AssetQuantityView {
                        asset: ViewId::new(
                            slug(format!("rescue/observed/{asset:?}")),
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
    let val = |account, asset| rescue::value(world, account, asset);
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
                slug(format!("{location:?}")),
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
            id: slug(format!("road/{a:?}/{b:?}")),
            source: slug(format!("{a:?}")),
            target: slug(format!("{b:?}")),
            label: Some("2 ticks / 1 fuel · fast 1 / 2".into()),
            classes: vec![],
        })
        .collect();
    let at = LOCATIONS
        .into_iter()
        .find(|location| val(AccountId::Responder, Asset::At(*location)) > 0)
        .unwrap_or(Location::Base);
    let mut crew = link_account(
        visual_entity(
            slug("crew"),
            "Evacuation crew",
            SceneGlyphView::Vehicle,
            SceneAnchorView::GraphNode {
                node: slug(format!("{at:?}")),
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
        slug("rescue/incident/account/responder"),
    );
    crew.metrics = vec![
        visual_metric(
            slug("fuel"),
            "Fuel",
            val(AccountId::Responder, Asset::Fuel),
            None,
        ),
        visual_metric(
            slug("energy"),
            "Crew energy",
            val(AccountId::Responder, Asset::Energy),
            None,
        ),
        visual_metric(
            slug("medicine"),
            "Medical kits",
            val(AccountId::Responder, Asset::MedicalKit),
            None,
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
        let mut entity = link_account(
            visual_entity(
                slug(format!("patient/{id}")),
                format!("Patient {}", id + 1),
                SceneGlyphView::Person,
                SceneAnchorView::GraphNode {
                    node: slug(format!("{node:?}")),
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
            slug(format!("rescue/incident/account/patient-{id}")),
        );
        entity.metrics = vec![
            visual_metric(
                slug("vitality"),
                "Vitality",
                val(patient, Asset::Vitality),
                Some("/ 3"),
            ),
            visual_metric(
                slug("decline"),
                "Next deterioration",
                val(patient, Asset::NextDecline),
                Some("tick"),
            ),
            visual_metric(
                slug("deadline"),
                "Evacuation deadline",
                val(patient, Asset::Deadline),
                Some("tick"),
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
        visual_metric(slug("time"), "Incident time", rescue::now(world), None),
        visual_metric(
            slug("saved"),
            "Patients evacuated",
            val(AccountId::World, Asset::SavedCount),
            None,
        ),
        visual_metric(
            slug("deaths"),
            "Patients lost",
            val(AccountId::World, Asset::DeathCount),
            None,
        ),
        visual_metric(
            slug("fuel"),
            "Fuel remaining",
            val(AccountId::Responder, Asset::Fuel),
            None,
        ),
    ]);
    for location in LOCATIONS.into_iter().filter(|location| present(*location)) {
        if let Some(entity) = scene
            .entities
            .iter_mut()
            .find(|entity| entity.id.key == slug(format!("{location:?}")))
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
                    visual_metric(slug("time"), "Incident time", rescue::now(world), None),
                    visual_metric(
                        slug("saved"),
                        "Patients evacuated",
                        val(AccountId::World, Asset::SavedCount),
                        None,
                    ),
                    visual_metric(
                        slug("lost"),
                        "Patients lost",
                        val(AccountId::World, Asset::DeathCount),
                        None,
                    ),
                ];
            }
        }
    }
    Some(scene)
}
