//! The mission expedition as a study: the documents of its Showcase and Stress instances.

use super::view::scene;
use crate::mission::expedition::{
    self, AGENTS, AccountId, Agent, Asset, Location, Policy, RateId, World,
};
use axionomy::path;
use axionomy_service::{
    DocumentSpec, InstanceProfile, ProposalSpec, ServiceError, StudyKey, StudyProgress, document,
    problem_error, proposal, simulate, telemetry,
};
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, Keyed, Label, ObjectiveDirectionView,
    ObjectiveView, ObservationView, TelemetryKindView, ViewDocument, ViewId, account_key,
    asset_key, evaluated_front,
};

pub(crate) fn build(
    study: &StudyKey,
    profile: InstanceProfile,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let initial = expedition::initial(matches!(profile, InstanceProfile::Stress), study.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            path!("coordinated"),
            "Mission · independent agents, shared plan",
            "Scout discovers sites and spends scarce radio charges to brief Medic. Independent travel overlaps; extraction requires both teammates, current stabilization and a free evacuation seat.",
            Policy::RadioCoordination,
        ),
        (
            path!("direct-north"),
            "Mission · north-first sector search",
            "Scout starts north without sensing. Intelligence travels through in-person briefings, and each teammate responds to the same changing hazards and clinical deadlines.",
            Policy::NorthFirst,
        ),
        (
            path!("rendezvous"),
            "Mission · rendezvous before deployment",
            "Save radio charges by meeting at base to share confirmed discoveries. The extra journeys compete with archive recovery and timely casualty evacuation.",
            Policy::Rendezvous,
        ),
    ] {
        let (trace, final_world) = simulate(&study.problem, &initial, |world| {
            expedition::run(world, policy)
        })?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
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
        .map_err(|error| problem_error(&study.problem, error))?;
        view.observations = AGENTS
            .into_iter()
            .map(|agent| observation(&initial, agent))
            .collect();
        let mut observed = initial.clone();
        for (frame, exchange) in view.frames.iter_mut().zip(trace.exchanges()) {
            observed
                .apply(exchange.clone())
                .map_err(|error| problem_error(&study.problem, error))?;
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
        view.proposals.push(proposal(ProposalSpec { id: path!("extract-without-intelligence"), label: "Extract a casualty before discovery or briefing", description: "Both agents need the confirmed site, physical co-location, free work capacity, energy, and a stabilized patient. An unverified sensor hint cannot satisfy these conditions." }, &initial, &expedition::action(RateId::Extract { objective: 0, location: Location::North })));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = evaluated_front(
            &documents,
            &documents[index],
            "Team outcomes on the same hidden sites and hazards",
        );
        documents[index].pareto_fronts.push(front);
    }
    Ok(documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    let val = |account, asset| expedition::value(world, account, asset);
    [
        (
            path!("completed"),
            "Objectives completed",
            val(AccountId::World, Asset::Completed),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("failed"),
            "Objectives lost",
            val(AccountId::World, Asset::FailedCount),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("vitality"),
            "Evacuated vitality",
            val(AccountId::World, Asset::SavedVitality),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("radio"),
            "Radio charges spent",
            val(AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("energy"),
            "Team energy spent",
            AGENTS
                .into_iter()
                .map(|agent| val(AccountId::Agent(agent), Asset::SpentEnergy))
                .sum(),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("time"),
            "Elapsed mission time",
            expedition::now(world),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView::new(key, label, direction, value))
    .collect()
}

fn observation(world: &World, agent: Agent) -> ObservationView {
    let key = expedition::agent_view(world, agent).observation_key();
    ObservationView {
        actor: ViewId::new(path!("mission/actor").then(&agent), format!("{agent:?}")),
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
                    account.label(),
                ),
                balances: key
                    .balances()
                    .iter()
                    .filter(|(owner, _, _)| owner == account)
                    .map(|(_, asset, quantity)| AssetQuantityView {
                        asset: ViewId::new(asset_key::<expedition::Ontology>(asset), asset.label()),
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
                asset: ViewId::new(asset_key::<expedition::Ontology>(asset), asset.label()),
                quantity: ExactQuantity(quantity.to_string()),
            })
            .collect(),
    }
}
