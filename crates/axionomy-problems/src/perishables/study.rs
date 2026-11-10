//! The perishable inventory as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    perishables::{self, ObjectiveKey},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::path;
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, ServiceError, StudyKey, StudyProgress, document, frontier_trace, problem_error,
    proposal, telemetry,
};
use axionomy_view::{
    ObjectiveAxisView, ObjectiveDirectionView, ObjectiveView, ParetoFrontView, TelemetryKindView,
    ViewDocument, exact_front, exact_values,
};

/// Perishable inventory: its catalog entry and how its studies are computed.
pub struct Perishables {
    descriptor: ProblemDescriptor,
}

impl Default for Perishables {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("perishables"),
                    title: "Perishable inventory",
                    summary: "Manage scarce refrigerated space, thermal history and daily food obligations. Cooling and processing compete for electricity; backup generation and deliveries compete for fuel.",
                    instances: [
                        "One hundred units split between two batches",
                        "Ten crates across six possible cohorts, six days, finite cold storage and a two-day outage",
                        "Twelve crates, tighter cold storage, less fuel, eight days and a three-day outage",
                    ],
                },
                ProblemFamily::TemporalSimulation,
                path!("outage"),
                &[
                    StrategyCopy {
                        key: path!("outage"),
                        label: "Power outage",
                        description: "Prioritize refrigeration, then revisit shipment, processing and backup decisions each day.",
                        algorithm: "indexed temporal effects",
                    },
                    StrategyCopy {
                        key: path!("pareto-inventory"),
                        label: "Pareto: save stock",
                        description: "Micro: exact storage frontier. Larger cold chains: a processing-first policy on the same outage schedule.",
                        algorithm: "exact Pareto search",
                    },
                    StrategyCopy {
                        key: path!("pareto-energy"),
                        label: "Pareto: save energy",
                        description: "Micro: minimum-energy storage frontier. Larger cold chains: alternate-day shipping to conserve fuel, with missed demand kept visible.",
                        algorithm: "exact Pareto search",
                    },
                ],
                &[
                    Capability::TemporalEffects,
                    Capability::FungibleCohorts,
                    Capability::NonFungibleFacts,
                    Capability::ExactPareto,
                ],
            ),
        }
    }
}

impl Problem for Perishables {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, ServiceError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let profile = study.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return super::cold_chain::study::build(study, profile, progress);
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
        .map_err(|error| problem_error(&study.problem, error))?;
    let front = perishables::storage_plan_front_with_transfer(&initial, transfer)
        .map_err(|error| problem_error(&study.problem, error))?;
    let inventory_trace = frontier_trace(
        &front,
        (
            ObjectiveKey::UsableInventory,
            ObjectiveDirectionView::Maximize,
        ),
        (
            ObjectiveKey::CoolingEnergy,
            ObjectiveDirectionView::Minimize,
        ),
        &study.problem,
    )?;
    let energy_trace = frontier_trace(
        &front,
        (
            ObjectiveKey::CoolingEnergy,
            ObjectiveDirectionView::Minimize,
        ),
        (
            ObjectiveKey::UsableInventory,
            ObjectiveDirectionView::Maximize,
        ),
        &study.problem,
    )?;
    let mut outage_view = document(progress, DocumentSpec { problem: &study.problem, document: path!("outage"), title: "Perishables · power outage", description: "Ten thousand interchangeable units belong to two batches. Time and power act on the batch, so one change ages thousands of units at once.", source_label: "Perishable inventory" }, &initial, &perishables::goal(), outage.trace(), vec![
        ObjectiveView::new(path!("claims"), "Total claims", ObjectiveDirectionView::Maximize, outage.claim_index().total_claims()),
        ObjectiveView::new(path!("usable"), "Usable claims", ObjectiveDirectionView::Maximize, outage.claim_index().usable_total(outage.world())),
    ], scene).map_err(|error| problem_error(&study.problem, error))?;
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
    outage_view.proposals.push(proposal(ProposalSpec { id: path!("oversized-transfer"), label: "Move 8,000 claims to fridge", description: "The warehouse holds only 7,000 units. The check reports the exact shortfall and moves nothing." }, &initial, &perishables::move_to_fridge(8_000)));

    let mut documents = vec![outage_view];
    for (strategy, title, description, trace) in [
        (
            path!("pareto-inventory"),
            "Perishables Pareto · save stock",
            "The storage plan that keeps the most usable stock.",
            inventory_trace,
        ),
        (
            path!("pareto-energy"),
            "Perishables Pareto · save cooling energy",
            "The storage plan that spends the least cooling energy.",
            energy_trace,
        ),
    ] {
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
                source_label: "Perishable inventory",
            },
            &initial,
            &perishables::storage_plan_goal(),
            &trace,
            vec![
                ObjectiveView::new(
                    path!("usable"),
                    "Usable inventory",
                    ObjectiveDirectionView::Maximize,
                    perishables::usable_inventory(&final_world),
                ),
                ObjectiveView::new(
                    path!("energy"),
                    "Cooling energy",
                    ObjectiveDirectionView::Minimize,
                    perishables::spent_cooling_energy(&final_world),
                ),
            ],
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
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
    Ok(documents)
}

fn front_view(result: &perishables::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    exact_front(
        "Usable stock vs. cooling energy",
        vec![
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
        selected,
        result.front().entries().iter().map(exact_values),
        |values| format!("{} usable · {} energy", values[0], values[1]),
    )
}
