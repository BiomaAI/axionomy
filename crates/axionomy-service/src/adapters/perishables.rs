use super::*;
use axionomy_problems::perishables::{
    self, AccountId, Asset, Cohort, Location, Moment, ObjectiveKey, World,
};
use axionomy_search::pareto::Objective;
use axionomy_view::{
    FrontierCompletenessView, ObjectiveAxisView, ObjectiveDirectionView, ParetoFrontView,
    ParetoPointView, TelemetryKindView, TimelineLaneView, TimelineSpanView, ViewId,
};

mod cold_chain;

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let profile = run.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return cold_chain::build(run, descriptor, profile);
    }
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
    let mut outage_view = document(DocumentSpec { problem: "perishables", strategy: "outage", title: "Perishables · power outage", description: "Ten thousand interchangeable units belong to two batches. Time and power act on the batch, so one change ages thousands of units at once.", source_label: "Perishable inventory" }, &initial, &perishables::goal(), outage.trace(), vec![
        ObjectiveView { key: slug("claims"), label: "Total claims".into(), direction: ObjectiveDirectionView::Maximize, value: outage.claim_index().total_claims().to_string() },
        ObjectiveView { key: slug("usable"), label: "Usable claims".into(), direction: ObjectiveDirectionView::Maximize, value: outage.claim_index().usable_total(outage.world()).to_string() },
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
    outage_view.proposals.push(proposal(ProposalSpec { id: "oversized-transfer", label: "Move 8,000 claims to fridge", description: "The warehouse holds only 7,000 units. The check reports the exact shortfall and moves nothing." }, &initial, &perishables::move_to_fridge(8_000)));

    let mut documents = vec![outage_view];
    for (strategy, title, description, trace) in [
        (
            "pareto-inventory",
            "Perishables Pareto · save stock",
            "The storage plan that keeps the most usable stock.",
            inventory_trace,
        ),
        (
            "pareto-energy",
            "Perishables Pareto · save cooling energy",
            "The storage plan that spends the least cooling energy.",
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
                    key: slug("usable"),
                    label: "Usable inventory".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: perishables::usable_inventory(&final_world).to_string(),
                },
                ObjectiveView {
                    key: slug("energy"),
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
                    "states explored".into(),
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
    artifact(run, descriptor, documents)
}

fn frontier_trace(
    result: &perishables::ParetoResult,
    inventory_first: bool,
) -> Result<axionomy::Trace<perishables::Ontology>, ServiceError> {
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
        title: "Usable stock vs. cooling energy".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: slug("usable"),
                label: "Usable inventory".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: slug("energy"),
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
        id: ViewId::new(slug(key), label),
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
            id: slug("ambient-condition"),
            lane: slug("ambient"),
            start: 0,
            end: 1,
            label: if ambient_rotten {
                "Rotten".into()
            } else {
                "Fresh until ambient expiry".into()
            },
            classes: if ambient_rotten {
                vec![slug("failed")]
            } else {
                Vec::new()
            },
        },
        TimelineSpanView {
            id: slug("cold-condition"),
            lane: slug("refrigerated"),
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
                vec![slug("selected")]
            } else {
                vec![slug("uncertain")]
            },
        },
        TimelineSpanView {
            id: slug("warehouse-claims"),
            lane: slug("warehouse"),
            start: 0,
            end: 3,
            label: format!("{warehouse} fungible claims"),
            classes: Vec::new(),
        },
        TimelineSpanView {
            id: slug("fridge-claims"),
            lane: slug("fridge"),
            start: 0,
            end: 3,
            label: format!("{fridge} fungible claims"),
            classes: Vec::new(),
        },
    ];
    Some(Scene::timeline(
        "Batches, storage, and the spoilage clock",
        lanes,
        spans,
        Some(moment),
    ))
}

// How Studio names and writes this problem's values.

counted!(perishables::Asset);

impl StudioLabel for perishables::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Clock and power".into(),
            Self::Storage(location) => format!("{location:?}"),
            Self::Cohort(cohort) => format!("{cohort:?} batch condition"),
        }
    }
}

impl StudioLabel for perishables::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "This account is the world clock".into(),
            Self::LocationIdentity(location) => format!("This account is the {location:?}"),
            Self::CohortIdentity(cohort) => format!("This is the {cohort:?} batch"),
            Self::Claim(cohort) => format!("Units in the {cohort:?} batch"),
            Self::Consumed(cohort) => format!("Used units from the {cohort:?} batch"),
            Self::Fresh(cohort, exposure, expiry) => format!(
                "{cohort:?} batch, still fresh under {exposure:?} exposure, expires at {expiry:?}"
            ),
            Self::Rotten(cohort) => format!("Spoiled units from the {cohort:?} batch"),
            Self::Ambient => "Ambient storage".into(),
            Self::Cold => "Cold storage".into(),
            Self::Powered => "Refrigeration running".into(),
            Self::Unpowered => "Refrigeration down".into(),
            Self::CoolingEnergy => "Cooling energy left".into(),
            Self::SpentCoolingEnergy => "Cooling energy spent".into(),
            Self::Now(moment) => format!("Clock is at {moment:?}"),
            Self::Before(moment) => format!("Before {moment:?}"),
            Self::Reached(moment) => format!("{moment:?} passed"),
            Self::Active => "Inventory simulation running".into(),
            Self::Solved => "Inventory simulation complete".into(),
            Self::Planning => "Storage plan open".into(),
            Self::PlanSolved => "Storage plan complete".into(),
        }
    }
}

impl StudioLabel for perishables::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::MoveToFridge => "Move ambient batch units into the fridge".into(),
            Self::Advance { from, to } => format!("Advance time: {from:?} → {to:?}"),
            Self::LosePower => "Refrigeration loses power".into(),
            Self::Eat { cohort, exposure } => {
                format!("Use fresh {cohort:?} units under {exposure:?} exposure")
            }
            Self::Spoil { cohort, exposure } => {
                format!("Spoil {cohort:?} batch under {exposure:?} exposure")
            }
            Self::Finish => "Finish the spoilage simulation".into(),
            Self::SealStoragePlan => "Commit the storage plan".into(),
        }
    }
}

debug_labels!(perishables::Role);
