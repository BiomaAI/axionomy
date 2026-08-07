use super::*;
use axionomy::{Exchange, Quantity, Trace};
use axionomy_problems::scheduling::{
    self, AccountId, Asset, Job, Machine, ObjectiveKey, Operation, RateId, World,
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
    let initial = match profile {
        InstanceProfile::Micro => scheduling::initial(),
        InstanceProfile::Showcase => scheduling::initial_showcase(),
        InstanceProfile::Stress => scheduling::initial_stress(),
    };
    let best = scheduling::solve_best_first(&initial)
        .ok_or_else(|| problem_error("scheduling", "best-first found no schedule"))?;
    let bounded = scheduling::branch_optimize(&initial)
        .ok_or_else(|| problem_error("scheduling", "optimizer found no schedule"))?;
    let pareto =
        scheduling::pareto_front(&initial).map_err(|error| problem_error("scheduling", error))?;
    let one = frontier_trace(&pareto, true)?;
    let two = frontier_trace(&pareto, false)?;
    let traces = [
        (
            "best_first",
            "Scheduling · best-first",
            "Generic search minimizes the encoded makespan.",
            best.trace().clone(),
            "best-first search",
            Some(best.expanded() as u64),
        ),
        (
            "bounded_optimizer",
            "Scheduling · bounded optimizer",
            "A caller-owned depth-first branch optimizer proposes a replay-verified schedule.",
            bounded.trace().clone(),
            "bounded branch optimization",
            None,
        ),
        (
            "pareto_job_one",
            "Scheduling Pareto · Job One first",
            "The exact frontier allocation favoring Job One completion.",
            one,
            "exact Pareto search",
            Some(pareto.progress().expanded() as u64),
        ),
        (
            "pareto_job_two",
            "Scheduling Pareto · Job Two first",
            "The exact frontier allocation favoring Job Two completion.",
            two,
            "exact Pareto search",
            Some(pareto.progress().expanded() as u64),
        ),
    ];
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("scheduling", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "scheduling",
                strategy,
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
        .map_err(|error| problem_error("scheduling", error))?;
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
                        "branches/states expanded".into(),
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
            view.proposals.push(proposal("scheduling", ProposalSpec { id: "unbound-operation", label: "Schedule without slots", description: "The operation rate exists, but required job, slot, and schedule roles are unbound." }, &initial, &malformed));
        }
        documents.push(view);
    }
    let impossible = if matches!(profile, InstanceProfile::Showcase | InstanceProfile::Stress) {
        scheduling::impossible_showcase()
    } else {
        scheduling::impossible()
    };
    let mut impossible_view = document(DocumentSpec { problem: "scheduling", strategy: "infeasible_horizon", title: "Scheduling · insufficient horizon", description: "A two-slot horizon is a replayable unschedulable model instance, not an exception hidden by the optimizer.", source_label: "Job-shop scheduling" }, &impossible, &scheduling::goal(), &Trace::new(), Vec::new(), scene).map_err(|error| problem_error("scheduling", error))?;
    impossible_view.telemetry.push(telemetry(
        "exhaustive feasibility",
        true,
        [(TelemetryKindView::Message, 0, "no complete schedule".into())],
    ));
    documents.push(impossible_view);
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn frontier_trace(
    result: &scheduling::ParetoResult,
    one_first: bool,
) -> Result<Trace<RateId, scheduling::Role, AccountId>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .min_by_key(|entry| {
            let one = objective_value(
                entry.objectives().objectives(),
                ObjectiveKey::JobOneCompletion,
            );
            let two = objective_value(
                entry.objectives().objectives(),
                ObjectiveKey::JobTwoCompletion,
            );
            if one_first { (one, two) } else { (two, one) }
        })
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error("scheduling", "empty Pareto frontier"))
}
fn objective_value(values: &[Objective<ObjectiveKey, u64>], key: ObjectiveKey) -> u64 {
    values
        .iter()
        .find(|value| value.key() == &key)
        .map_or(0, |value| *value.value())
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView {
            key: "job_one".into(),
            label: "Job One completion".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: scheduling::completion_time(world, Job::One).to_string(),
        },
        ObjectiveView {
            key: "job_two".into(),
            label: "Job Two completion".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: scheduling::completion_time(world, Job::Two).to_string(),
        },
    ]
}
fn front_view(result: &scheduling::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|o| o.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Replay-verified completion allocation frontier".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: "job_one".into(),
                label: "Job One".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "job_two".into(),
                label: "Job Two".into(),
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
                    label: format!("Job One {} · Job Two {}", values[0], values[1]),
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
    let lanes = [Machine::One, Machine::Two, Machine::Three]
        .into_iter()
        .map(|machine| TimelineLaneView {
            id: ViewId::new(
                format!("machine:{machine:?}"),
                format!("Machine {machine:?}"),
            ),
            classes: Vec::new(),
        })
        .collect();
    let mut spans = Vec::new();
    for machine in [Machine::One, Machine::Two, Machine::Three] {
        for operation in [
            Operation::OneA,
            Operation::OneB,
            Operation::OneC,
            Operation::TwoA,
            Operation::TwoB,
            Operation::TwoC,
        ] {
            let occupied = (0..12)
                .filter(|time| {
                    !world
                        .balance(
                            &AccountId::Slot(machine, *time),
                            &Asset::Reserved(operation),
                        )
                        .is_zero()
                })
                .collect::<Vec<_>>();
            if let (Some(start), Some(end)) = (occupied.first(), occupied.last()) {
                spans.push(TimelineSpanView {
                    id: format!("{machine:?}:{operation:?}"),
                    lane: format!("machine:{machine:?}"),
                    start: u64::from(*start),
                    end: u64::from(*end + 1),
                    label: format!("{operation:?}"),
                    classes: vec![
                        format!(
                            "job-{:?}",
                            if matches!(
                                operation,
                                Operation::OneA | Operation::OneB | Operation::OneC
                            ) {
                                Job::One
                            } else {
                                Job::Two
                            }
                        )
                        .to_lowercase(),
                    ],
                });
            }
        }
    }
    Some(Scene::timeline(
        "Encoded machine reservations",
        lanes,
        spans,
        Some(scheduling::encoded_makespan(world)),
    ))
}
