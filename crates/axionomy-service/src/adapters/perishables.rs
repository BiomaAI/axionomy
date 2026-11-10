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
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return cold_chain::build(study, descriptor, profile, progress);
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
    let mut outage_view = document(progress, DocumentSpec { problem: "perishables", strategy: "outage", title: "Perishables · power outage", description: "Ten thousand interchangeable units belong to two batches. Time and power act on the batch, so one change ages thousands of units at once.", source_label: "Perishable inventory" }, &initial, &perishables::goal(), outage.trace(), vec![
        ObjectiveView { key: path!("claims"), label: "Total claims".into(), direction: ObjectiveDirectionView::Maximize, value: outage.claim_index().total_claims().to_string() },
        ObjectiveView { key: path!("usable"), label: "Usable claims".into(), direction: ObjectiveDirectionView::Maximize, value: outage.claim_index().usable_total(outage.world()).to_string() },
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
            progress,
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
                    key: path!("usable"),
                    label: "Usable inventory".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: perishables::usable_inventory(&final_world).to_string(),
                },
                ObjectiveView {
                    key: path!("energy"),
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
    assemble(study, descriptor, documents)
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
                key: path!("usable"),
                label: "Usable inventory".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: path!("energy"),
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
        (path!("world"), "World clock"),
        (path!("ambient"), "Ambient cohort"),
        (path!("refrigerated"), "Refrigerated cohort"),
        (path!("warehouse"), "Warehouse claims"),
        (path!("fridge"), "Fridge claims"),
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
            id: path!("ambient-condition"),
            lane: path!("ambient"),
            start: 0,
            end: 1,
            label: if ambient_rotten {
                "Rotten".into()
            } else {
                "Fresh until ambient expiry".into()
            },
            classes: if ambient_rotten {
                vec![path!("failed")]
            } else {
                Vec::new()
            },
        },
        TimelineSpanView {
            id: path!("cold-condition"),
            lane: path!("refrigerated"),
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
                vec![path!("selected")]
            } else {
                vec![path!("uncertain")]
            },
        },
        TimelineSpanView {
            id: path!("warehouse-claims"),
            lane: path!("warehouse"),
            start: 0,
            end: 3,
            label: format!("{warehouse} fungible claims"),
            classes: Vec::new(),
        },
        TimelineSpanView {
            id: path!("fridge-claims"),
            lane: path!("fridge"),
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

fn cohort(cohort: Cohort) -> &'static str {
    match cohort {
        Cohort::Ambient => "ambient",
        Cohort::Refrigerated => "refrigerated",
    }
}

/// Where a batch's stock stays fresh.
fn exposure(exposure: perishables::Exposure) -> &'static str {
    match exposure {
        perishables::Exposure::Ambient => "in ambient storage",
        perishables::Exposure::Cold => "in cold storage",
        perishables::Exposure::WarmedAfterOutage => "after warming in the outage",
    }
}

fn moment(moment: Moment) -> &'static str {
    match moment {
        Moment::Harvest => "harvest",
        Moment::AmbientExpiry => "ambient expiry",
        Moment::WarmedExpiry => "warmed expiry",
        Moment::ColdExpiry => "cold expiry",
    }
}

fn sentence(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

impl StudioFormat for perishables::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::CoolingEnergy | Self::SpentCoolingEnergy => QuantityFormat::COUNT.unit("units"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for perishables::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "Clock and power".into(),
            Self::Storage(location) => format!("{location:?}"),
            Self::Cohort(batch) => format!("{} batch condition", sentence(cohort(*batch))),
        }
    }
}

impl StudioLabel for perishables::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Clock identity".into(),
            Self::LocationIdentity(location) => format!("{location:?} identity"),
            Self::CohortIdentity(batch) => {
                format!("{} batch identity", sentence(cohort(*batch)))
            }
            Self::Claim(batch) => format!("Units in the {} batch", cohort(*batch)),
            Self::Consumed(batch) => format!("Used units from the {} batch", cohort(*batch)),
            Self::Fresh(batch, held, expiry) => format!(
                "{} batch fresh {}, until {}",
                sentence(cohort(*batch)),
                exposure(*held),
                moment(*expiry)
            ),
            Self::Rotten(batch) => format!("Spoiled units from the {} batch", cohort(*batch)),
            Self::Ambient => "Ambient storage".into(),
            Self::Cold => "Cold storage".into(),
            Self::Powered => "Refrigeration running".into(),
            Self::Unpowered => "Refrigeration down".into(),
            Self::CoolingEnergy => "Cooling energy left".into(),
            Self::SpentCoolingEnergy => "Cooling energy spent".into(),
            Self::Now(at) => format!("Clock at {}", moment(*at)),
            Self::Before(at) => format!("Before {}", moment(*at)),
            Self::Reached(at) => format!("{} reached", sentence(moment(*at))),
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
            Self::Advance { from, to } => {
                format!("Advance time: {} → {}", moment(*from), moment(*to))
            }
            Self::LosePower => "Refrigeration loses power".into(),
            Self::Eat {
                cohort: batch,
                exposure: held,
            } => format!("Use fresh {} units {}", cohort(*batch), exposure(*held)),
            Self::Spoil {
                cohort: batch,
                exposure: held,
            } => format!("Spoil the {} batch {}", cohort(*batch), exposure(*held)),
            Self::Finish => "Finish the spoilage simulation".into(),
            Self::SealStoragePlan => "Commit the storage plan".into(),
        }
    }
}

impl StudioLabel for perishables::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::World => "World",
            Self::Holder => "Holder",
            Self::Storage => "Storage",
            Self::Cohort => "Batch",
            Self::SourceStorage => "Source storage",
            Self::DestinationStorage => "Destination storage",
            Self::SourceCohort => "Source batch",
            Self::DestinationCohort => "Destination batch",
        }
        .into()
    }
}
