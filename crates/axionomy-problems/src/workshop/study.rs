//! The workshop as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    support::{ProblemCopy, StrategyCopy, problem},
    workshop::{self, AccountId, ObjectiveKey, RateId, World},
};
use axionomy::path;
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, ServiceError, StudyKey, StudyProgress, document, frontier_trace, problem_error,
    proposal, telemetry,
};
use axionomy_view::{
    ObjectiveDirectionView, ObjectiveView, ParetoFrontView, TelemetryKindView, ViewDocument,
    exact_front, exact_values, objective_axes, objective_value,
};

/// Workshop: its catalog entry and how its studies are computed.
pub struct Workshop {
    descriptor: ProblemDescriptor,
}

impl Default for Workshop {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("workshop"),
                    title: "Workshop",
                    summary: "Choose funded contracts, prepare components, manage tool wear and deliver products. Exact tradeoffs expose faster work, lower waste and higher revenue.",
                    instances: [
                        "Two chairs made from raw stock",
                        "Choose two of three chair/table contracts with components, tool wear, repair, upgrades and salvage",
                        "Choose at least three of five contracts under shared wood, time, labor and repair constraints",
                    ],
                },
                ProblemFamily::Production,
                path!("minimum-waste"),
                &[
                    StrategyCopy {
                        key: path!("breadth-first"),
                        label: "Fewest steps",
                        description: "Breadth-first search: the fewest recipe firings, whatever they waste.",
                        algorithm: "breadth-first search",
                    },
                    StrategyCopy {
                        key: path!("minimum-waste"),
                        label: "Least waste",
                        description: "Best-first search guided by scrap accumulated in the workshop account.",
                        algorithm: "best-first search",
                    },
                    StrategyCopy {
                        key: path!("pareto-waste"),
                        label: "Pareto: least waste",
                        description: "The lowest-scrap point on the exact Pareto frontier.",
                        algorithm: "exact Pareto search",
                    },
                    StrategyCopy {
                        key: path!("pareto-time"),
                        label: "Pareto: least time",
                        description: "The fastest point on that exact frontier.",
                        algorithm: "exact Pareto search",
                    },
                ],
                &[
                    Capability::DeterministicSearch,
                    Capability::WeightedSearch,
                    Capability::ExactPareto,
                    Capability::FeasibilityAssessment,
                ],
            ),
        }
    }
}

impl Problem for Workshop {
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
    let initial = match study.instance {
        InstanceProfile::Micro => workshop::initial(),
        InstanceProfile::Showcase => workshop::initial_showcase(),
        InstanceProfile::Stress => workshop::initial_stress(),
    };
    let bfs = workshop::solve_bfs(&initial)
        .ok_or_else(|| problem_error(&study.problem, "BFS found no plan"))?;
    let best = workshop::minimize_waste(&initial)
        .ok_or_else(|| problem_error(&study.problem, "best-first found no plan"))?;
    let pareto =
        workshop::pareto_front(&initial).map_err(|error| problem_error(&study.problem, error))?;
    let waste_trace = frontier_trace(
        &pareto,
        (ObjectiveKey::Waste, ObjectiveDirectionView::Minimize),
        (ObjectiveKey::Time, ObjectiveDirectionView::Minimize),
        &study.problem,
    )?;
    let time_trace = frontier_trace(
        &pareto,
        (ObjectiveKey::Time, ObjectiveDirectionView::Minimize),
        (ObjectiveKey::Waste, ObjectiveDirectionView::Minimize),
        &study.problem,
    )?;
    let mut traces = vec![
        (
            path!("breadth-first"),
            "Workshop · fewest steps",
            "Breadth-first search minimises the number of steps, ignoring how much material is wasted.",
            bfs.trace().clone(),
            "breadth-first search",
            bfs.expanded() as u64,
        ),
        (
            path!("minimum-waste"),
            "Workshop · least waste",
            "Best-first search minimizes irreversible scrap; unused offcuts become scrap when the shift closes.",
            best.trace().clone(),
            "best-first search",
            best.expanded() as u64,
        ),
        (
            path!("pareto-waste"),
            "Workshop Pareto · least waste",
            "A least-waste plan on the frontier of scrap, time and earned revenue.",
            waste_trace,
            "exact Pareto search",
            pareto.progress().expanded() as u64,
        ),
        (
            path!("pareto-time"),
            "Workshop Pareto · least time",
            "A fastest plan on the frontier, including setup, component production, repair and delivery.",
            time_trace,
            "exact Pareto search",
            pareto.progress().expanded() as u64,
        ),
    ];
    if initial.account(&AccountId::Customer(0)).is_some() {
        let entry = pareto
            .front()
            .entries()
            .iter()
            .max_by_key(|entry| objective_value(entry, &ObjectiveKey::Revenue))
            .unwrap();
        traces.push((path!("pareto-revenue"), "Workshop Pareto · highest revenue",
            "The highest contract revenue on the exact frontier; tables require more material, labor and tooling than chairs.",
            entry.payload().clone(), "exact Pareto search", pareto.progress().expanded() as u64));
    }
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
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
                source_label: "Stoichiometric workshop",
            },
            &initial,
            &workshop::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.pareto_fronts.push(front_view(&pareto, &view));
        view.telemetry.push(telemetry(
            algorithm,
            true,
            [
                (
                    TelemetryKindView::Expanded,
                    expanded,
                    "states explored".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "production exchanges".into(),
                ),
            ],
        ));
        let counterfeit = workshop::action(RateId::CounterfeitChair);
        view.proposals.push(proposal(ProposalSpec { id: path!("counterfeit"), label: "Counterfeit chair", description: "This rule would create material out of nothing, and the conservation law rejects it." }, &initial, &counterfeit));
        documents.push(view);
    }
    Ok(documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    workshop::objectives(world)
        .objectives()
        .iter()
        .map(|objective| {
            let (key, label, direction) = match objective.key() {
                ObjectiveKey::Waste => (path!("waste"), "Waste", ObjectiveDirectionView::Minimize),
                ObjectiveKey::Time => (
                    path!("time"),
                    "Process time",
                    ObjectiveDirectionView::Minimize,
                ),
                ObjectiveKey::Revenue => (
                    path!("revenue"),
                    "Contract revenue",
                    ObjectiveDirectionView::Maximize,
                ),
            };
            ObjectiveView::new(key, label, direction, *objective.value())
        })
        .collect()
}

fn front_view(result: &workshop::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    exact_front(
        "Production tradeoffs — every non-dominated plan",
        objective_axes(selected),
        selected,
        result.front().entries().iter().map(exact_values),
        |values| match values {
            [waste, time, credit] => format!("{waste} waste · {time} time · {credit} credit"),
            _ => format!("{} waste · {} time", values[0], values[1]),
        },
    )
}
