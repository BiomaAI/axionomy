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
    let initial = if matches!(
        instance_profile(request, descriptor),
        InstanceProfile::Micro
    ) {
        logistics::initial_micro()
    } else {
        logistics::initial()
    };
    let samples = match instance_profile(request, descriptor) {
        InstanceProfile::Micro => request.budget.clamp(2, 16),
        InstanceProfile::Showcase => request.budget.max(2),
        InstanceProfile::Stress => request.budget.max(256),
    } as usize;
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
        let mut view = document(
            DocumentSpec {
                problem: "logistics",
                strategy,
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
        .map_err(|error| problem_error("logistics", error))?;
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
            view.proposals.push(proposal(
                "logistics",
                ProposalSpec {
                    id: "load-without-roles",
                    label: "Load without vehicle/order",
                    description: "The rule exists, but no vehicle and no order were named.",
                },
                &initial,
                &malformed,
            ));
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
    let mut mcts = document(DocumentSpec { problem: "logistics", strategy: "mcts", title: "Logistics · live MCTS decision", description: "A load, then the route MCTS currently prefers. This stops mid-journey on purpose: it is a decision in progress, not a finished delivery.", source_label: "Stochastic logistics" }, &initial, &logistics::goal(), &prefix, Vec::new(), scene).map_err(|error| problem_error("logistics", error))?;
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
        title: "Completion vs. deliveries vs. time — sampled, not exact".into(),
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
    let traveling = logistics::ROUTES.into_iter().find(|route| {
        !world
            .balance(&AccountId::Vehicle, &Asset::Traveling(*route))
            .is_zero()
    });
    let vehicle_anchor = if let Some(location) = focus {
        SceneAnchorView::GraphNode {
            node: format!("location:{location:?}"),
        }
    } else if let Some(route) = traveling {
        SceneAnchorView::GraphEdge {
            edge: format!("route:{route:?}"),
            progress: Some(0.5),
        }
    } else {
        SceneAnchorView::Unanchored
    };
    let mut vehicle = visual_entity(
        "vehicle:fleet-1",
        "Vehicle 1",
        SceneGlyphView::Vehicle,
        vehicle_anchor.clone(),
        SceneToneView::Active,
        traveling.map(|route| format!("traveling {route:?}")),
    );
    vehicle.account = Some("logistics:account:vehicle".into());
    vehicle.metrics = vec![
        visual_metric(
            "fuel",
            "Fuel",
            world.balance(&AccountId::Vehicle, &Asset::Fuel),
            Some("units"),
        ),
        visual_metric(
            "cargo",
            "Cargo",
            world.balance(&AccountId::Vehicle, &Asset::CargoOccupied),
            Some("orders"),
        ),
        visual_metric(
            "elapsed",
            "Elapsed",
            world.balance(&AccountId::Vehicle, &Asset::ElapsedTime),
            Some("ticks"),
        ),
    ];
    let order_entities = logistics::ORDERS.into_iter().map(|order| {
        let order_account = AccountId::Order(order);
        let (anchor, tone, status) = if !world.balance(&order_account, &Asset::Delivered).is_zero()
        {
            (
                SceneAnchorView::GraphNode {
                    node: "location:Customer".into(),
                },
                SceneToneView::Success,
                "delivered",
            )
        } else if !world.balance(&order_account, &Asset::InTransit).is_zero() {
            (vehicle_anchor.clone(), SceneToneView::Active, "in transit")
        } else {
            (
                SceneAnchorView::GraphNode {
                    node: "location:Depot".into(),
                },
                SceneToneView::Neutral,
                "waiting",
            )
        };
        let mut entity = visual_entity(
            format!("order:{order:?}"),
            format!("Order {order:?}"),
            SceneGlyphView::Package,
            anchor,
            tone,
            Some(status.into()),
        );
        entity.account = Some(format!("logistics:account:order-{order:?}").to_ascii_lowercase());
        entity
    });
    let delivered = logistics::ORDERS
        .into_iter()
        .filter(|order| {
            !world
                .balance(&AccountId::Order(*order), &Asset::Delivered)
                .is_zero()
        })
        .count();
    Some(
        Scene::graph(
            "Route network and travel risk",
            nodes,
            edges,
            focus.map(|location| format!("location:{location:?}")),
        )
        .with_entities(std::iter::once(vehicle).chain(order_entities))
        .with_metrics([
            visual_metric("delivered", "Orders delivered", delivered, Some("orders")),
            visual_metric(
                "remaining_time",
                "Time remaining",
                world.balance(&AccountId::Vehicle, &Asset::TimeRemaining),
                Some("ticks"),
            ),
            visual_metric(
                "fuel",
                "Fuel remaining",
                world.balance(&AccountId::Vehicle, &Asset::Fuel),
                Some("units"),
            ),
        ]),
    )
}
