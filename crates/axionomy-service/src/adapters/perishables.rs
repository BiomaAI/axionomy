use super::*;
use axionomy_problems::perishables::{
    self, AccountId, Asset, Cohort, Location, Moment, ObjectiveKey, RateId, Role, World,
};
use axionomy_search::pareto::Objective;
use axionomy_view::{
    FrontierCompletenessView, ObjectiveAxisView, ObjectiveDirectionView, ParetoFrontView,
    ParetoPointView, TelemetryKindView, TimelineLaneView, TimelineSpanView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let profile = instance_profile(request, descriptor);
    let (initial, transfer) = match profile {
        InstanceProfile::Micro => (perishables::initial_with_inventory(70, 30), 10),
        InstanceProfile::Showcase => (perishables::initial(), perishables::DEFAULT_TRANSFER),
        InstanceProfile::Stress => (
            perishables::initial_with_inventory(700_000, 300_000),
            100_000,
        ),
    };
    let outage = perishables::run_outage_scenario(&initial, transfer)
        .map_err(|error| problem_error("perishables", error))?;
    let front = perishables::storage_plan_front_with_transfer(&initial, transfer)
        .map_err(|error| problem_error("perishables", error))?;
    let inventory_trace = frontier_trace(&front, true)?;
    let energy_trace = frontier_trace(&front, false)?;
    let mut outage_view = document(DocumentSpec { problem: "perishables", strategy: "outage", title: "Perishables · power outage", description: "Ten thousand fungible claims share two non-fungible cohort condition facts; time and power effects update cohorts rather than individual units.", source_label: "Perishable inventory" }, &initial, &perishables::goal(), outage.trace(), vec![
        ObjectiveView { key: "claims".into(), label: "Total claims".into(), direction: ObjectiveDirectionView::Maximize, value: outage.claim_index().total_claims().to_string() },
        ObjectiveView { key: "usable".into(), label: "Usable claims".into(), direction: ObjectiveDirectionView::Maximize, value: outage.claim_index().usable_total(outage.world()).to_string() },
    ], scene).map_err(|error| problem_error("perishables", error))?;
    outage_view.telemetry.push(telemetry(
        "indexed temporal effect agenda",
        true,
        [
            (
                TelemetryKindView::Generated,
                outage.trace().exchanges().len() as u64,
                "accepted temporal exchanges".into(),
            ),
            (
                TelemetryKindView::Expanded,
                outage
                    .effects()
                    .iter()
                    .map(|report| report.applied().len())
                    .sum::<usize>() as u64,
                "effects applied".into(),
            ),
            (
                TelemetryKindView::Message,
                outage
                    .effects()
                    .iter()
                    .map(|report| report.stale().len())
                    .sum::<usize>() as u64,
                "stale effects skipped".into(),
            ),
        ],
    ));
    outage_view.proposals.push(proposal("perishables", ProposalSpec { id: "oversized-transfer", label: "Move 8,000 claims to fridge", description: "The warehouse has only 7,000 ambient claims, so the assessment returns the exact missing amount without mutating inventory." }, &initial, &perishables::move_to_fridge(8_000)));

    let mut documents = vec![outage_view];
    for (strategy, title, description, trace) in [
        (
            "pareto_inventory",
            "Perishables Pareto · preserve inventory",
            "The exact storage commitment maximizing usable inventory before simulated decay.",
            inventory_trace,
        ),
        (
            "pareto_energy",
            "Perishables Pareto · save cooling energy",
            "The exact storage commitment minimizing cooling energy while retaining a non-dominated outcome.",
            energy_trace,
        ),
    ] {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("perishables", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "perishables",
                strategy,
                title,
                description,
                source_label: "Perishable inventory",
            },
            &initial,
            &perishables::storage_plan_goal(),
            &trace,
            vec![
                ObjectiveView {
                    key: "usable".into(),
                    label: "Usable inventory".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: perishables::usable_inventory(&final_world).to_string(),
                },
                ObjectiveView {
                    key: "energy".into(),
                    label: "Cooling energy".into(),
                    direction: ObjectiveDirectionView::Minimize,
                    value: perishables::spent_cooling_energy(&final_world).to_string(),
                },
            ],
            scene,
        )
        .map_err(|error| problem_error("perishables", error))?;
        view.pareto_fronts.push(front_view(&front, &view));
        view.telemetry.push(telemetry(
            "exact bounded Pareto storage search",
            true,
            [
                (
                    TelemetryKindView::Expanded,
                    front.progress().expanded() as u64,
                    "states expanded".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "storage exchanges".into(),
                ),
            ],
        ));
        documents.push(view);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn frontier_trace(
    result: &perishables::ParetoResult,
    inventory_first: bool,
) -> Result<axionomy::Trace<RateId, Role, AccountId>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .max_by_key(|entry| {
            let usable = objective_value(
                entry.objectives().objectives(),
                ObjectiveKey::UsableInventory,
            );
            let energy =
                objective_value(entry.objectives().objectives(), ObjectiveKey::CoolingEnergy);
            if inventory_first {
                (usable, u64::MAX - energy)
            } else {
                (u64::MAX - energy, usable)
            }
        })
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error("perishables", "empty Pareto frontier"))
}
fn objective_value(values: &[Objective<ObjectiveKey, u64>], key: ObjectiveKey) -> u64 {
    values
        .iter()
        .find(|o| o.key() == &key)
        .map_or(0, |o| *o.value())
}
fn front_view(result: &perishables::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|o| o.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Usable inventory / cooling energy frontier".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: "usable".into(),
                label: "Usable inventory".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: "energy".into(),
                label: "Cooling energy".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|o| o.value().to_string())
                    .collect::<Vec<_>>();
                ParetoPointView {
                    label: format!("{} usable · {} energy", values[0], values[1]),
                    selected: values
                        .iter()
                        .map(String::as_str)
                        .eq(selected.iter().copied()),
                    values,
                }
            })
            .collect(),
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let lanes = [
        ("world", "World clock"),
        ("ambient", "Ambient cohort"),
        ("refrigerated", "Refrigerated cohort"),
        ("warehouse", "Warehouse claims"),
        ("fridge", "Fridge claims"),
    ]
    .into_iter()
    .map(|(key, label)| TimelineLaneView {
        id: ViewId::new(key, label),
        classes: Vec::new(),
    })
    .collect();
    let moment = [
        Moment::Harvest,
        Moment::AmbientExpiry,
        Moment::WarmedExpiry,
        Moment::ColdExpiry,
    ]
    .into_iter()
    .position(|moment| {
        !world
            .balance(&AccountId::World, &Asset::Now(moment))
            .is_zero()
    })
    .unwrap_or(0) as u64;
    let ambient_rotten = !world
        .balance(
            &AccountId::Cohort(Cohort::Ambient),
            &Asset::Rotten(Cohort::Ambient),
        )
        .is_zero();
    let cold_rotten = !world
        .balance(
            &AccountId::Cohort(Cohort::Refrigerated),
            &Asset::Rotten(Cohort::Refrigerated),
        )
        .is_zero();
    let powered = !world
        .balance(&AccountId::Storage(Location::Fridge), &Asset::Powered)
        .is_zero();
    let warehouse = world.balance(
        &AccountId::Storage(Location::Warehouse),
        &Asset::Claim(Cohort::Ambient),
    );
    let fridge = world.balance(
        &AccountId::Storage(Location::Fridge),
        &Asset::Claim(Cohort::Refrigerated),
    );
    let spans = vec![
        TimelineSpanView {
            id: "ambient-condition".into(),
            lane: "ambient".into(),
            start: 0,
            end: 1,
            label: if ambient_rotten {
                "Rotten".into()
            } else {
                "Fresh until ambient expiry".into()
            },
            classes: if ambient_rotten {
                vec!["failed".into()]
            } else {
                Vec::new()
            },
        },
        TimelineSpanView {
            id: "cold-condition".into(),
            lane: "refrigerated".into(),
            start: 0,
            end: if powered { 3 } else { 2 },
            label: if cold_rotten {
                "Rotten after outage".into()
            } else if powered {
                "Cold + powered".into()
            } else {
                "Warming after outage".into()
            },
            classes: if powered {
                vec!["selected".into()]
            } else {
                vec!["uncertain".into()]
            },
        },
        TimelineSpanView {
            id: "warehouse-claims".into(),
            lane: "warehouse".into(),
            start: 0,
            end: 3,
            label: format!("{warehouse} fungible claims"),
            classes: Vec::new(),
        },
        TimelineSpanView {
            id: "fridge-claims".into(),
            lane: "fridge".into(),
            start: 0,
            end: 3,
            label: format!("{fridge} fungible claims"),
            classes: Vec::new(),
        },
    ];
    Some(Scene::Timeline {
        title: "Cohort conditions, claims, storage, and event time".into(),
        lanes,
        spans,
        cursor: Some(moment),
    })
}
