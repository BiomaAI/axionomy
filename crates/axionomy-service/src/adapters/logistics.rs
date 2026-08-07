use super::*;
use axionomy::{Exchange, Quantity, Trace};
use axionomy_problems::logistics::{
    self, AccountId, Asset, Location, Policy, RateId, Route, World,
};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let initial = logistics::initial();
    let samples = request.budget.max(2) as usize;
    let estimate = logistics::monte_carlo_with_risk_progress(
        &initial,
        samples,
        logistics::RiskCriterion::Mean,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                "monte_carlo",
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
    progress.ensure()?;
    let estimate = estimate.ok_or_else(|| problem_error("logistics", "Monte Carlo failed"))?;
    let front =
        logistics::policy_front_with_progress(&initial, samples, samples.clamp(1, 8), |state| {
            progress.emit(
                "pareto_sampling",
                state.completed_samples() as u64,
                state.total_samples() as u64,
                format!(
                    "{}/{} multi-objective policy rollouts",
                    state.completed_samples(),
                    state.total_samples()
                ),
            )
        });
    progress.ensure()?;
    let front = front.ok_or_else(|| problem_error("logistics", "policy front failed"))?;
    let mut documents = Vec::new();
    for (strategy, policy) in [("direct", Policy::Direct), ("reliable", Policy::Reliable)] {
        let rollout = logistics::run_policy(&initial, policy, request.seed);
        let mut view = document(DocumentSpec { problem: "logistics", strategy, title: if policy == Policy::Direct { "Logistics · direct route" } else { "Logistics · reliable route" }, description: if policy == Policy::Direct { "A shorter policy repeatedly accepts encoded weather and breakdown outcomes." } else { "A longer policy trades time for higher completion reliability across recurrent chance events." }, source_label: "Stochastic logistics" }, &initial, &logistics::goal(), rollout.trace(), objectives(&rollout), scene).map_err(|error| problem_error("logistics", error))?;
        view.pareto_fronts.push(policy_front(&front, policy));
        let stats = estimate
            .estimate(policy)
            .ok_or_else(|| problem_error("logistics", "missing policy estimate"))?;
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
            view.proposals.push(proposal("logistics", ProposalSpec { id: "load-without-roles", label: "Load without vehicle/order", description: "The rate is known, but the required vehicle and order bindings are missing." }, &initial, &malformed));
        }
        documents.push(view);
    }

    let mut loaded = initial.fork();
    let load = logistics::candidates(&loaded)
        .into_iter()
        .find(|exchange| matches!(exchange.rate(), RateId::Load(_)))
        .ok_or_else(|| problem_error("logistics", "no load action"))?;
    loaded
        .apply(load.clone())
        .map_err(|error| problem_error("logistics", format!("{error:?}")))?;
    let decision = logistics::plan_action_with_progress(
        &loaded,
        samples,
        request.seed,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                "mcts",
                state.iterations() as u64,
                state.target_iterations() as u64,
                format!(
                    "{}/{} MCTS iterations · {} nodes · {} root actions",
                    state.iterations(),
                    state.target_iterations(),
                    state.nodes(),
                    state.root_children()
                ),
            )
        },
    )
    .map_err(|error| problem_error("logistics", format!("{error:?}")))?;
    progress.ensure()?;
    let decision =
        decision.ok_or_else(|| problem_error("logistics", "MCTS planning was interrupted"))?;
    let mut prefix = Trace::new();
    prefix.push(load);
    prefix.push(decision.action().clone());
    let mut mcts = document(DocumentSpec { problem: "logistics", strategy: "mcts", title: "Logistics · live MCTS decision", description: "A load action followed by the current MCTS route decision; this prefix is deliberately branchable rather than presented as a completed mission.", source_label: "Stochastic logistics" }, &initial, &logistics::goal(), &prefix, Vec::new(), scene).map_err(|error| problem_error("logistics", error))?;
    mcts.telemetry.push(telemetry(
        "Monte Carlo tree search",
        false,
        [
            (
                TelemetryKindView::Iteration,
                decision.iterations() as u64,
                "tree iterations".into(),
            ),
            (
                TelemetryKindView::Generated,
                decision.children().len() as u64,
                "root actions".into(),
            ),
        ],
    ));
    mcts.pareto_fronts
        .push(policy_front(&front, estimate.chosen()));
    documents.push(mcts);
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn objectives(rollout: &logistics::MissionRollout) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView {
            key: "completed".into(),
            label: "Completed".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: u8::from(rollout.completed()).to_string(),
        },
        ObjectiveView {
            key: "delivered".into(),
            label: "Orders delivered".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: rollout.delivered().to_string(),
        },
        ObjectiveView {
            key: "time".into(),
            label: "Elapsed time".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: rollout.elapsed_time().to_string(),
        },
    ]
}
fn policy_front(front: &logistics::PolicyFront, selected: Policy) -> ParetoFrontView {
    ParetoFrontView {
        title: "Sampled completion / delivery / time frontier".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: vec![
            ObjectiveAxisView {
                key: "completion".into(),
                label: "Completion".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: "delivered".into(),
                label: "Delivered".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: "time".into(),
                label: "Mean time".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points: front
            .entries()
            .iter()
            .map(|entry| {
                let policy = *entry.payload().policy();
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|o| format!("{:.3}", o.value()))
                    .collect();
                ParetoPointView {
                    label: format!("{policy:?}"),
                    values,
                    selected: policy == selected,
                }
            })
            .collect(),
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        (Location::Depot, 70.0, 150.0),
        (Location::Junction, 320.0, 150.0),
        (Location::Customer, 570.0, 150.0),
    ];
    let focus = locations
        .into_iter()
        .find(|(location, _, _)| {
            !world
                .balance(&AccountId::Vehicle, &Asset::At(*location))
                .is_zero()
        })
        .map(|(location, _, _)| location);
    let nodes = locations
        .into_iter()
        .map(|(location, x, y)| GraphNodeView {
            id: ViewId::new(format!("location:{location:?}"), format!("{location:?}")),
            classes: if focus == Some(location) {
                vec!["current".into()]
            } else if location == Location::Customer {
                vec!["goal".into()]
            } else {
                Vec::new()
            },
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let route_ends = |route| match route {
        Route::DirectOut => (Location::Depot, Location::Customer),
        Route::DirectBack => (Location::Customer, Location::Depot),
        Route::SafeOutFirst => (Location::Depot, Location::Junction),
        Route::SafeOutSecond => (Location::Junction, Location::Customer),
        Route::SafeBackFirst => (Location::Customer, Location::Junction),
        Route::SafeBackSecond => (Location::Junction, Location::Depot),
    };
    let edges = logistics::ROUTES
        .into_iter()
        .map(|route| {
            let (from, to) = route_ends(route);
            let traveling = !world
                .balance(&AccountId::Vehicle, &Asset::Traveling(route))
                .is_zero();
            GraphEdgeView {
                id: format!("route:{route:?}"),
                source: format!("location:{from:?}"),
                target: format!("location:{to:?}"),
                label: Some(format!("{route:?}")),
                classes: if traveling {
                    vec!["current".into(), "uncertain".into()]
                } else {
                    vec!["uncertain".into()]
                },
            }
        })
        .collect();
    Some(Scene::Graph {
        title: "Encoded route network and stochastic travel".into(),
        nodes,
        edges,
        focus: focus.map(|location| format!("location:{location:?}")),
    })
}
