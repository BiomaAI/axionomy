//! The logistics fleet as a study: the documents of its Showcase and Stress instances.

use super::view::scene;
use crate::logistics::fleet::{
    self as fleet, AccountId, Asset, Location, Policy, RateId, VANS, Van, World,
};
use crate::support::Size;
use axionomy::path;
use axionomy_study::{
    DocumentSpec, ProposalSpec, StudyError, StudyKey, StudyProgress, Work, document, problem_error,
    proposal, simulate, telemetry,
};
use axionomy_view::{
    Labeled, ObjectiveDirectionView, ObjectiveView, SearchObservationKindView, TelemetryKindView,
    ViewDocument, evaluated_front,
};

pub(crate) fn build(
    study: &StudyKey,
    profile: Size,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let initial = fleet::initial(matches!(profile, Size::Stress), study.seed);
    let dispatch = Work::new(
        Labeled::new(path!("fleet-dispatch"), "Fleet dispatch"),
        Labeled::new(path!("fleet-dispatch"), "Fleet dispatch"),
        SearchObservationKindView::Phase,
    );
    let mut documents = Vec::new();
    for (strategy, title, description, policy) in [
        (
            path!("direct"),
            "Logistics · fastest-route fleet",
            "Dispatch two different vans through the shortest available road paths. Express links save hops but consume extra fuel and can cause breakdowns; the fleet responds with shared towing and repair.",
            Policy::Fastest,
        ),
        (
            path!("reliable"),
            "Logistics · reliable-route fleet",
            "Avoid the risky express link, respect the single occupied trunk road, and revisit delivery, refueling and maintenance choices as orders arrive. Heavy freight and cold cargo require different vehicle capacity.",
            Policy::Reliable,
        ),
        (
            path!("mcts"),
            "Logistics · adaptive delivery",
            "Use responsive fleet dispatch and hub relays: pack spare cold-van capacity, stage compatible freight in the finite hub, and let the other van collect it. Every shipment, service and payment is a core exchange. This larger-profile policy is not MCTS; the Micro fixture retains receding-horizon MCTS.",
            Policy::Relay,
        ),
    ] {
        let _ = progress.emit(
            &dispatch,
            documents.len() as u64,
            3,
            format!(
                "Computing fleet policy {} of 3: {title}",
                documents.len() + 1
            ),
        );
        progress.checkpoint()?;
        let (trace, final_world) =
            simulate(&study.problem, &initial, |world| fleet::run(world, policy))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
                title,
                description,
                source_label: "Heterogeneous delivery fleet",
            },
            &initial,
            &fleet::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.telemetry.push(telemetry(
            "responsive fleet dispatch with public route planning",
            true,
            [(
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "verified fleet decisions and realized route tickets".into(),
            )],
        ));
        view.proposals.push(proposal(ProposalSpec { id: path!("wrong-vehicle"), label: "Put refrigerated cargo in the freight van", description: "The freight van has more payload space but no refrigerated slot. Cargo compatibility is enforced by the exchange, alongside arrival and physical location." }, &initial, &fleet::action(RateId::Load { van: Van::Freight, order: 0, at: Location::Depot })));
        documents.push(view);
    }
    let _ = progress.emit(
        &dispatch,
        3,
        3,
        "Three fleet policies completed and replay verified",
    );
    progress.checkpoint()?;
    for index in 0..documents.len() {
        let front = evaluated_front(
            &documents,
            &documents[index],
            "Fleet policies on the same orders and vehicle-route tickets",
        );
        documents[index].pareto_fronts.push(front);
    }
    Ok(documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    let val = |account, asset| fleet::value(world, account, asset);
    [
        (
            path!("delivered"),
            "Orders delivered",
            val(AccountId::World, Asset::DeliveredCount),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("on-time"),
            "Orders on time",
            val(AccountId::World, Asset::OnTimeCount),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("expired"),
            "Orders expired",
            val(AccountId::World, Asset::ExpiredCount),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("cash"),
            "Fleet cash remaining",
            val(AccountId::World, Asset::Money),
            ObjectiveDirectionView::Maximize,
        ),
        (
            path!("fuel"),
            "Fuel used",
            VANS.into_iter()
                .map(|van| val(AccountId::Vehicle(van), Asset::SpentFuel))
                .sum(),
            ObjectiveDirectionView::Minimize,
        ),
        (
            path!("time"),
            "Elapsed fleet time",
            fleet::now(world),
            ObjectiveDirectionView::Minimize,
        ),
    ]
    .into_iter()
    .map(|(key, label, value, direction)| ObjectiveView::new(key, label, direction, value))
    .collect()
}
