//! The rescue incident as a study: the documents of its Showcase and Stress instances.

use super::view::scene;
use crate::rescue::incident::{self, AccountId, Asset, Location, Policy, RateId, World};
use axionomy::path;
use axionomy_study::{
    DocumentSpec, InstanceProfile, ProposalSpec, StudyError, StudyKey, StudyProgress, document,
    problem_error, proposal, simulate, telemetry,
};
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, Label, ObjectiveDirectionView, ObjectiveView,
    ObservationView, TelemetryKindView, ViewDocument, ViewId, account_key, asset_key,
    evaluated_front,
};

pub(crate) fn build(
    study: &StudyKey,
    profile: InstanceProfile,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let initial = incident::initial(matches!(profile, InstanceProfile::Stress), study.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            path!("observe-then-follow"),
            "Rescue · responsive triage",
            "Spend limited sensor charges on fallible hints, search sectors, prioritize observed casualties, and choose treatment or rapid evacuation against continuing deterioration.",
            Policy::Triage,
        ),
        (
            path!("direct-north"),
            "Rescue · rapid sector sweep",
            "Begin north without a scan, search successive sectors, and use fast transport. Each rescued patient occupies the only evacuation seat until hospital arrival.",
            Policy::RapidNorth,
        ),
        (
            path!("stabilize-first"),
            "Rescue · stabilize before transport",
            "Treat discovered patients before loading when a kit remains. Restored vitality and delayed deterioration cost time that other casualties may not have.",
            Policy::StabilizeFirst,
        ),
    ] {
        let (trace, final_world) = simulate(&study.problem, &initial, |world| {
            incident::run(world, policy)
        })?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
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
        .map_err(|error| problem_error(&study.problem, error))?;
        view.observations.push(observation(&initial));
        let mut observed = initial.clone();
        for (frame, exchange) in view.frames.iter_mut().zip(trace.exchanges()) {
            observed
                .apply(exchange.clone())
                .map_err(|error| problem_error(&study.problem, error))?;
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
        view.proposals.push(proposal(ProposalSpec { id: path!("undiscovered-patient"), label: "Load a patient before finding them", description: "A location hint is insufficient. Physical discovery, co-location, current clinical observations, energy, and an empty evacuation seat are required." }, &initial, &incident::action(RateId::Load { patient: 0, location: Location::North })));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = evaluated_front(
            &documents,
            &documents[index],
            "Incident outcomes on the same hidden scenario",
        );
        documents[index].pareto_fronts.push(front);
    }
    Ok(documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    [
        (
            path!("saved"),
            "Patients evacuated",
            AccountId::World,
            Asset::SavedCount,
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("vitality"),
            "Vitality at hospital arrival",
            AccountId::World,
            Asset::SavedVitality,
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("deaths"),
            "Patients lost",
            AccountId::World,
            Asset::DeathCount,
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("fuel"),
            "Fuel spent",
            AccountId::Responder,
            Asset::SpentFuel,
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("medicine"),
            "Kits used",
            AccountId::Responder,
            Asset::UsedMedicalKit,
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("time"),
            "Elapsed time",
            AccountId::World,
            Asset::Time,
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, account, asset, direction)| {
        ObjectiveView::new(
            key,
            label,
            direction,
            incident::value(world, account, asset),
        )
    })
    .collect()
}

fn observation(world: &World) -> ObservationView {
    let key = incident::responder_view(world).observation_key();
    ObservationView {
        actor: ViewId::new(path!("rescue/actor/responder"), "Rescue crew"),
        label: format!(
            "Crew observations at time {} · undiscovered patients and Nature omitted",
            incident::now(world)
        ),
        visible_accounts: key
            .visible_accounts()
            .iter()
            .map(|account| AccountView {
                account: ViewId::new(account_key::<incident::Ontology>(account), account.label()),
                balances: key
                    .balances()
                    .iter()
                    .filter(|(owner, _, _)| owner == account)
                    .map(|(_, asset, quantity)| AssetQuantityView {
                        asset: ViewId::new(asset_key::<incident::Ontology>(asset), asset.label()),
                        quantity: ExactQuantity(quantity.to_string()),
                    })
                    .collect(),
            })
            .collect(),
        facts: Vec::new(),
    }
}
