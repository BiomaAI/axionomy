//! The job-shop schedule as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    scheduling::{self, Job, ObjectiveKey, World},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::{Exchange, Quantity, Trace, path};
use axionomy_study::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, StudyError, StudyKey, StudyProgress, document, frontier_trace, problem_error,
    proposal, telemetry,
};
use axionomy_view::{
    ObjectiveAxisView, ObjectiveDirectionView, ObjectiveView, ParetoFrontView, TelemetryKindView,
    ViewDocument, exact_front, exact_values,
};

/// Job-shop scheduling: its catalog entry and how its studies are computed.
pub struct Scheduling {
    descriptor: ProblemDescriptor,
}

impl Default for Scheduling {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("scheduling"),
                    title: "Job-shop scheduling",
                    summary: "Route arriving jobs through queues and actual execution. Alternative machines, setup changes, shared crews and a recoverable machine failure change which schedule finishes first.",
                    instances: [
                        "Two jobs on the smallest horizon that still forces a machine conflict",
                        "Six operations, three alternative machines, two crews and a recoverable disruption at tick three",
                        "One shared crew, later job arrival and a longer repair create a different execution bottleneck",
                    ],
                },
                ProblemFamily::Scheduling,
                path!("bounded-optimizer"),
                &[
                    StrategyCopy {
                        key: path!("best-first"),
                        label: "Shortest makespan",
                        description: "Best-first search on the finish time recorded in the schedule.",
                        algorithm: "best-first search",
                    },
                    StrategyCopy {
                        key: path!("bounded-optimizer"),
                        label: "Branch optimizer",
                        description: "Depth-first branch-and-bound abandons schedules it can already prove are worse.",
                        algorithm: "branch-and-bound",
                    },
                    StrategyCopy {
                        key: path!("pareto-job-one"),
                        label: "Pareto: Job One first",
                        description: "The frontier schedule that finishes Job One earliest.",
                        algorithm: "exact Pareto search",
                    },
                    StrategyCopy {
                        key: path!("pareto-job-two"),
                        label: "Pareto: Job Two first",
                        description: "The frontier schedule that finishes Job Two earliest.",
                        algorithm: "exact Pareto search",
                    },
                ],
                &[
                    Capability::WeightedSearch,
                    Capability::BranchOptimization,
                    Capability::ExactPareto,
                    Capability::FeasibilityAssessment,
                ],
            ),
        }
    }
}

impl Problem for Scheduling {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, StudyError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let profile = study.instance;
    let initial = match profile {
        InstanceProfile::Micro => scheduling::initial(),
        InstanceProfile::Showcase => scheduling::initial_showcase(),
        InstanceProfile::Stress => scheduling::initial_stress(),
    };
    let best = scheduling::solve_best_first(&initial)
        .ok_or_else(|| problem_error(&study.problem, "best-first found no schedule"))?;
    let bounded = scheduling::branch_optimize(&initial)
        .ok_or_else(|| problem_error(&study.problem, "optimizer found no schedule"))?;
    let pareto =
        scheduling::pareto_front(&initial).map_err(|error| problem_error(&study.problem, error))?;
    let one = frontier_trace(
        &pareto,
        (
            ObjectiveKey::JobOneCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        (
            ObjectiveKey::JobTwoCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        &study.problem,
    )?;
    let two = frontier_trace(
        &pareto,
        (
            ObjectiveKey::JobTwoCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        (
            ObjectiveKey::JobOneCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        &study.problem,
    )?;
    let traces = [
        (
            path!("best-first"),
            "Scheduling · shortest makespan",
            "A generic search for the earliest overall finish time.",
            best.trace().clone(),
            "best-first search",
            Some(best.expanded() as u64),
        ),
        (
            path!("bounded-optimizer"),
            "Scheduling · branch optimizer",
            "Branch-and-bound proposes a schedule; the economy replays every step before accepting it.",
            bounded.trace().clone(),
            "bounded branch optimization",
            None,
        ),
        (
            path!("pareto-job-one"),
            "Scheduling Pareto · Job One first",
            "The frontier schedule that gets Job One done soonest — at Job Two's expense.",
            one,
            "exact Pareto search",
            Some(pareto.progress().expanded() as u64),
        ),
        (
            path!("pareto-job-two"),
            "Scheduling Pareto · Job Two first",
            "The frontier schedule that gets Job Two done soonest — at Job One's expense.",
            two,
            "exact Pareto search",
            Some(pareto.progress().expanded() as u64),
        ),
    ];
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
                source_label: "Job-shop scheduling",
            },
            &initial,
            &scheduling::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.pareto_fronts.push(front_view(&pareto, &view));
        view.telemetry.push(telemetry(
            algorithm,
            true,
            expanded
                .into_iter()
                .map(|value| {
                    (
                        TelemetryKindView::Expanded,
                        value,
                        "branches and states explored".into(),
                    )
                })
                .chain([(
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "scheduled transitions".into(),
                )]),
        ));
        if let Some(candidate) = scheduling::candidates(&initial).first() {
            let malformed = Exchange::new(*candidate.rate(), Quantity::new(1));
            view.proposals.push(proposal(ProposalSpec { id: path!("unbound-operation"), label: "Schedule without slots", description: "The operation exists, but no job, slot, or schedule was named for it." }, &initial, &malformed));
        }
        documents.push(view);
    }
    let impossible = if matches!(profile, InstanceProfile::Showcase | InstanceProfile::Stress) {
        scheduling::impossible_showcase()
    } else {
        scheduling::impossible()
    };
    let mut impossible_view = document(progress, DocumentSpec { problem: &study.problem, document: path!("infeasible-horizon"), title: "Scheduling · not enough time slots", description: "With only two slots the work cannot fit. That is an answer you can inspect, not an error the optimizer swallowed.", source_label: "Job-shop scheduling" }, &impossible, &scheduling::goal(), &Trace::new(), Vec::new(), scene).map_err(|error| problem_error(&study.problem, error))?;
    impossible_view.telemetry.push(telemetry(
        "exhaustive feasibility",
        true,
        [(TelemetryKindView::Message, 0, "no complete schedule".into())],
    ));
    documents.push(impossible_view);
    Ok(documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView::new(
            path!("job-one"),
            "Job One completion",
            ObjectiveDirectionView::Minimize,
            scheduling::completion_time(world, Job::One),
        ),
        ObjectiveView::new(
            path!("job-two"),
            "Job Two completion",
            ObjectiveDirectionView::Minimize,
            scheduling::completion_time(world, Job::Two),
        ),
    ]
}

fn front_view(result: &scheduling::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    exact_front(
        "Job One finish vs. Job Two finish",
        vec![
            ObjectiveAxisView {
                key: path!("job-one"),
                label: "Job One".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: path!("job-two"),
                label: "Job Two".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        selected,
        result.front().entries().iter().map(exact_values),
        |values| format!("Job One {} · Job Two {}", values[0], values[1]),
    )
}
