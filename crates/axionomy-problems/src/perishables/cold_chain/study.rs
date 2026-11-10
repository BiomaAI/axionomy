//! The cold chain as a study: the documents of its Showcase and Stress instances.

use super::view::scene;
use crate::perishables::cold_chain::{
    self as chain, AccountId, Asset, Place, Policy, RateId, World,
};
use axionomy::path;
use axionomy_service::{
    DocumentSpec, InstanceProfile, ProposalSpec, ServiceError, StudyKey, StudyProgress, document,
    problem_error, proposal, telemetry,
};
use axionomy_view::{
    ObjectiveDirectionView, ObjectiveView, TelemetryKindView, ViewDocument, evaluated_front,
};

pub(crate) fn build(
    study: &StudyKey,
    profile: InstanceProfile,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let initial = chain::initial(matches!(profile, InstanceProfile::Stress), study.seed);
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            path!("outage"),
            "Perishables · protect the cold chain",
            "Power finite cold storage first, deliver urgent fresh stock, process the remainder and use backup fuel when electricity runs short. Transport draws from the same fuel reserve.",
            Policy::ProtectCold,
        ),
        (
            path!("pareto-inventory"),
            "Perishables · process before cooling",
            "Preserve ambient stock before powering refrigeration. Processing consumes electricity and labor, then the policy revisits shipment and storage choices each day.",
            Policy::ProcessFirst,
        ),
        (
            path!("pareto-energy"),
            "Perishables · conserve transport fuel",
            "Ship on alternating days and process what the energy budget permits. Fewer trips save fuel but can leave food obligations unmet.",
            Policy::ConserveFuel,
        ),
        (
            path!("storage-buffer"),
            "Perishables · rebalance cold storage",
            "Ship refrigerated stock to free capacity, split a warm cohort to fit that space, and move it into cooling while preserving its prior heat exposure.",
            Policy::RebalanceCold,
        ),
    ] {
        let trace = chain::run(&initial, policy);
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error(&study.problem, error))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
                title,
                description,
                source_label: "Cold-chain operations",
            },
            &initial,
            &chain::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.telemetry.push(telemetry(
            "daily resource-responsive policy",
            true,
            [(
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "verified physical and temporal exchanges".into(),
            )],
        ));
        view.proposals.push(proposal(ProposalSpec { id: path!("overfilled-cold-store"), label: "Move the entire ambient cohort into full cold storage", description: "The refrigerated space is occupied. Goods cannot move until enough physical capacity is released." }, &initial, &chain::action(RateId::Move { batch: 0, from: Place::Warehouse, to: Place::Fridge }, 1)));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = evaluated_front(
            &documents,
            &documents[index],
            "Non-dominated policies on the same outage schedule",
        );
        documents[index].pareto_fronts.push(front);
    }
    Ok(documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    let usable = chain::total(world, Asset::Fresh)
        + chain::total(world, Asset::Preserved)
        + chain::total(world, Asset::Delivered);
    [
        (
            path!("saved"),
            "Food saved or delivered",
            usable,
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("delivered"),
            "Food delivered",
            chain::total(world, Asset::Delivered),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("revenue"),
            "Sales revenue",
            chain::value(world, AccountId::World, Asset::Money),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("energy"),
            "Energy used",
            chain::total(world, Asset::SpentEnergy),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("fuel"),
            "Fuel used",
            chain::total(world, Asset::GeneratorFuel) + chain::total(world, Asset::TransportFuel),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("missed"),
            "Missed food obligations",
            chain::total(world, Asset::MissedDemand),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView::new(key, label, direction, value))
    .collect()
}
