//! Stochastic logistics as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    logistics::{self, Policy},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::{Exchange, Quantity, path};
use axionomy_search::rollout::Unrecorded;
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, ServiceError, StudyKey, StudyProgress, Work, document, problem_error, proposal,
    telemetry,
};
use axionomy_view::{
    ObjectiveAxisView, ObjectiveDirectionView, ObjectiveView, ParetoFrontView,
    SearchObservationKindView, TelemetryKindView, ViewDocument, sampled_policy_front,
};

/// Stochastic logistics: its catalog entry and how its studies are computed.
pub struct Logistics {
    descriptor: ProblemDescriptor,
}

impl Default for Logistics {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("logistics"),
                    title: "Stochastic logistics",
                    summary: "Coordinate refrigerated and heavy-freight vans across three customers. Arriving orders, delivery windows, a shared trunk road, finite fuel, occupied service bays and hub handoffs interact with route disruptions.",
                    instances: [
                        "Two deliveries with a compact route and a small sample budget",
                        "Two vans, four heterogeneous orders, three customers, one trunk-road slot, two-unit relay hub and finite services",
                        "Six arriving orders, more route delays, a longer horizon, finite fuel and three repair parts",
                    ],
                },
                ProblemFamily::StochasticPlanning,
                path!("reliable"),
                &[
                    StrategyCopy {
                        key: path!("direct"),
                        label: "Fastest routes",
                        description: "Use express routes and respond to realized breakdowns; Micro retains sampled single-vehicle evaluation.",
                        algorithm: "Responsive fleet dispatch; Micro Monte Carlo",
                    },
                    StrategyCopy {
                        key: path!("reliable"),
                        label: "Reliable routes",
                        description: "Avoid the express link and coordinate deliveries around shared road and service capacity.",
                        algorithm: "Responsive fleet dispatch; Micro Monte Carlo",
                    },
                    StrategyCopy {
                        key: path!("mcts"),
                        label: "Relay fleet / Micro MCTS",
                        description: "Larger fleets bundle compatible cargo and hand it off through the hub; Micro retains full receding-horizon Monte Carlo tree search.",
                        algorithm: "Responsive hub relay; Micro Monte Carlo tree search",
                    },
                ],
                &[
                    Capability::Chance,
                    Capability::MonteCarlo,
                    Capability::Mcts,
                    Capability::ApproximatePareto,
                ],
            ),
        }
    }
}

impl Problem for Logistics {
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
        return super::fleet::study::build(study, profile, progress);
    }
    let initial = if matches!(study.instance, InstanceProfile::Micro) {
        logistics::initial_micro()
    } else {
        logistics::initial()
    };
    let samples = match study.instance {
        InstanceProfile::Micro => study.budget.clamp(2, 16),
        InstanceProfile::Showcase => study.budget.max(2),
        InstanceProfile::Stress => study.budget.max(256),
    } as usize;
    let (monte_carlo, pareto_sampling) = (Work::monte_carlo(), Work::pareto_sampling());
    let mcts = Work::new(
        path!("mcts"),
        path!("mcts"),
        SearchObservationKindView::Tree,
    );
    let estimate = logistics::monte_carlo_with_risk_progress(
        &initial,
        samples,
        logistics::RiskCriterion::Mean,
        &mut Unrecorded,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                &monte_carlo,
                state.completed_samples() as u64,
                state.total_samples() as u64,
                format!(
                    "{}/{} logistics policy rollouts",
                    state.completed_samples(),
                    state.total_samples()
                ),
            )
        },
    );
    progress.checkpoint()?;
    let estimate = estimate.ok_or_else(|| problem_error(&study.problem, "Monte Carlo failed"))?;
    let front = logistics::policy_front_with_progress(
        &initial,
        samples,
        &mut Unrecorded,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                &pareto_sampling,
                state.completed_samples() as u64,
                state.total_samples() as u64,
                format!(
                    "{}/{} multi-objective policy rollouts",
                    state.completed_samples(),
                    state.total_samples()
                ),
            )
        },
    );
    progress.checkpoint()?;
    let front = front.ok_or_else(|| problem_error(&study.problem, "policy front failed"))?;
    let mut documents = Vec::new();
    for (strategy, policy) in [
        (path!("direct"), Policy::Direct),
        (path!("reliable"), Policy::Reliable),
    ] {
        let rollout = logistics::run_policy(&mut initial.clone(), policy, study.seed);
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
                title: if policy == Policy::Direct {
                    "Logistics · direct route"
                } else {
                    "Logistics · reliable route"
                },
                description: if policy == Policy::Direct {
                    "The short route, taking whatever weather and breakdowns the run produces."
                } else {
                    "A longer route that spends time to finish more often."
                },
                source_label: "Stochastic logistics",
            },
            &initial,
            &logistics::goal(),
            rollout.trace(),
            objectives(&rollout),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.pareto_fronts.push(policy_front(&front, policy));
        let stats = estimate
            .estimate(policy)
            .ok_or_else(|| problem_error(&study.problem, "missing policy estimate"))?;
        view.telemetry.push(telemetry(
            "Monte Carlo rollouts",
            false,
            [
                (
                    TelemetryKindView::Sample,
                    stats.samples() as u64,
                    "rollouts sampled".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    rollout.steps() as u64,
                    "selected rollout exchanges".into(),
                ),
            ],
        ));
        if let Some(load) = logistics::candidates(&initial).first() {
            let malformed = Exchange::new(*load.rate(), Quantity::new(1));
            view.proposals.push(proposal(
                ProposalSpec {
                    id: path!("load-without-roles"),
                    label: "Load without vehicle/order",
                    description: "The rule exists, but no vehicle and no order were named.",
                },
                &initial,
                &malformed,
            ));
        }
        documents.push(view);
    }

    let planned = logistics::run_planned_with_progress(
        &initial,
        samples,
        study.seed,
        study.seed ^ 0x4c4f_4749_5354_4943,
        samples.clamp(1, 8),
        |decision, state| {
            progress.emit(
                &mcts,
                state.iterations() as u64,
                state.target_iterations() as u64,
                format!(
                    "Decision {} · {}/{} MCTS iterations · {} nodes · {} root actions",
                    decision + 1,
                    state.iterations(),
                    state.target_iterations(),
                    state.nodes(),
                    state.root_children()
                ),
            )
        },
    )
    .map_err(|error| problem_error(&study.problem, error))?;
    progress.checkpoint()?;
    let planned =
        planned.ok_or_else(|| problem_error(&study.problem, "MCTS planning was interrupted"))?;
    let description = format!(
        "Replan after deliveries and disruptions, execute every selected action, and retain the final outcome: {}. Search randomness is separate from realized weather. The sampled frontier compares the two fixed-policy baselines, not this planner.",
        match planned.stop {
            logistics::PlanningStop::DeliveredAllOrders => "every order delivered",
            logistics::PlanningStop::NoFeasibleAction => "no feasible action left",
            logistics::PlanningStop::Horizon => "the planning horizon reached",
        }
    );
    let mut mcts = document(
        progress,
        DocumentSpec {
            problem: &study.problem,
            document: path!("mcts"),
            title: "Logistics · adaptive delivery",
            description: &description,
            source_label: "Stochastic logistics",
        },
        &initial,
        &logistics::goal(),
        planned.rollout.trace(),
        objectives(&planned.rollout),
        scene,
    )
    .map_err(|error| problem_error(&study.problem, error))?;
    mcts.telemetry.push(telemetry(
        "Repeated Monte Carlo tree search",
        false,
        [
            (
                TelemetryKindView::Iteration,
                planned
                    .decisions
                    .iter()
                    .map(|step| step.decision.iterations() as u64)
                    .sum(),
                "iterations for committed decisions".into(),
            ),
            (
                TelemetryKindView::Generated,
                planned.decisions.len() as u64,
                "search decisions actually executed".into(),
            ),
            (
                TelemetryKindView::Transitions,
                planned.rollout.steps() as u64,
                "exchanges through delivery or explicit failure".into(),
            ),
        ],
    ));
    documents.push(mcts);
    Ok(documents)
}

fn objectives(rollout: &logistics::MissionRollout) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView::new(
            path!("completed"),
            "Completed",
            ObjectiveDirectionView::Maximize,
            u64::from(rollout.completed()),
        ),
        ObjectiveView::new(
            path!("delivered"),
            "Orders delivered",
            ObjectiveDirectionView::Maximize,
            rollout.delivered() as u64,
        ),
        ObjectiveView::new(
            path!("time"),
            "Elapsed time",
            ObjectiveDirectionView::Minimize,
            rollout.elapsed_time(),
        ),
    ]
}

fn policy_front(front: &logistics::PolicyFront, selected: Policy) -> ParetoFrontView {
    sampled_policy_front(
        "Completion vs. deliveries vs. time — sampled, not exact",
        vec![
            ObjectiveAxisView {
                key: path!("completion"),
                label: "Completion".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: path!("delivered"),
                label: "Delivered".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: path!("time"),
                label: "Mean time".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        front,
        |policy| format!("{policy:?}"),
        |policy| policy == selected,
    )
}
