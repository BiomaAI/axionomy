use super::*;
mod fleet;
use axionomy::{Exchange, Quantity};
use axionomy_problems::logistics::{self, AccountId, Asset, Location, Policy, Route, World};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = run.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return fleet::build(run, descriptor, profile, progress);
    }
    let initial = if matches!(run.instance, InstanceProfile::Micro) {
        logistics::initial_micro()
    } else {
        logistics::initial()
    };
    let samples = match run.instance {
        InstanceProfile::Micro => run.budget.clamp(2, 16),
        InstanceProfile::Showcase => run.budget.max(2),
        InstanceProfile::Stress => run.budget.max(256),
    } as usize;
    let estimate = logistics::monte_carlo_with_risk_progress(
        &initial,
        samples,
        logistics::RiskCriterion::Mean,
        samples.clamp(1, 8),
        |state| {
            progress.emit(
                slug("monte-carlo"),
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
                slug("pareto-sampling"),
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
        let rollout = logistics::run_policy(&initial, policy, run.seed);
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

    let planned = logistics::run_planned_with_progress(
        &initial,
        samples,
        run.seed,
        run.seed ^ 0x4c4f_4749_5354_4943,
        samples.clamp(1, 8),
        |decision, state| {
            progress.emit(
                slug("mcts"),
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
    .map_err(|error| problem_error("logistics", error))?;
    progress.ensure()?;
    let planned =
        planned.ok_or_else(|| problem_error("logistics", "MCTS planning was interrupted"))?;
    let description = format!(
        "Replan after deliveries and disruptions, execute every selected action, and retain the final outcome: {:?}. Search randomness is separate from realized weather. The sampled frontier compares the two fixed-policy baselines, not this planner.",
        planned.stop
    );
    let mut mcts = document(
        DocumentSpec {
            problem: "logistics",
            strategy: "mcts",
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
    .map_err(|error| problem_error("logistics", error))?;
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
    artifact(run, descriptor, documents)
}

fn objectives(rollout: &logistics::MissionRollout) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView {
            key: slug("completed"),
            label: "Completed".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: u8::from(rollout.completed()).to_string(),
        },
        ObjectiveView {
            key: slug("delivered"),
            label: "Orders delivered".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: rollout.delivered().to_string(),
        },
        ObjectiveView {
            key: slug("time"),
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
                key: slug("completion"),
                label: "Completion".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: slug("delivered"),
                label: "Delivered".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: slug("time"),
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
            id: ViewId::new(
                slug(format!("location/{location:?}")),
                format!("{location:?}"),
            ),
            classes: if focus == Some(location) {
                vec![slug("current")]
            } else if location == Location::Customer {
                vec![slug("goal")]
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
                id: slug(format!("route/{route:?}")),
                source: slug(format!("location/{from:?}")),
                target: slug(format!("location/{to:?}")),
                label: Some(format!("{route:?}")),
                classes: if traveling {
                    vec![slug("current"), slug("uncertain")]
                } else {
                    vec![slug("uncertain")]
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
            node: slug(format!("location/{location:?}")),
        }
    } else if let Some(route) = traveling {
        SceneAnchorView::GraphEdge {
            edge: slug(format!("route/{route:?}")),
            progress: Some(0.5),
        }
    } else {
        SceneAnchorView::Unanchored
    };
    let vehicle_asset = focus.map_or_else(
        || Asset::Traveling(traveling.expect("vehicle is located or traveling")),
        Asset::At,
    );
    let mut vehicle = link_balance::<logistics::Ontology>(
        visual_entity(
            slug("vehicle/fleet-1"),
            "Vehicle 1",
            SceneGlyphView::Vehicle,
            vehicle_anchor.clone(),
            SceneEntityRoleView::Occupant,
            SceneToneView::Active,
            traveling.map(|route| format!("traveling {route:?}")),
        ),
        &AccountId::Vehicle,
        &vehicle_asset,
    );
    vehicle.metrics = vec![
        visual_metric(
            slug("fuel"),
            "Fuel",
            world.balance(&AccountId::Vehicle, &Asset::Fuel),
            Some("units"),
        ),
        visual_metric(
            slug("cargo"),
            "Cargo",
            world.balance(&AccountId::Vehicle, &Asset::CargoOccupied),
            Some("orders"),
        ),
        visual_metric(
            slug("elapsed"),
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
                    node: slug("location/Customer"),
                },
                SceneToneView::Success,
                "delivered",
            )
        } else if !world.balance(&order_account, &Asset::InTransit).is_zero() {
            (vehicle_anchor.clone(), SceneToneView::Active, "in transit")
        } else {
            (
                SceneAnchorView::GraphNode {
                    node: slug("location/Depot"),
                },
                SceneToneView::Neutral,
                "waiting",
            )
        };
        link_account::<logistics::Ontology>(
            visual_entity(
                slug(format!("order/{order:?}")),
                format!("Order {order:?}"),
                SceneGlyphView::Package,
                anchor,
                SceneEntityRoleView::Attachment,
                tone,
                Some(status.into()),
            ),
            &order_account,
        )
    });
    let system_entities = [
        link_account::<logistics::Ontology>(
            visual_entity(
                slug("system/fuel-station"),
                "Fuel station",
                SceneGlyphView::Fuel,
                SceneAnchorView::GraphNode {
                    node: slug("location/Depot"),
                },
                SceneEntityRoleView::State,
                SceneToneView::Neutral,
                Some("refuel supply".into()),
            ),
            &AccountId::FuelStation,
        ),
        link_account::<logistics::Ontology>(
            visual_entity(
                slug("system/nature"),
                "Travel conditions",
                SceneGlyphView::Weather,
                SceneAnchorView::GraphNode {
                    node: slug("location/Junction"),
                },
                SceneEntityRoleView::Context,
                SceneToneView::Uncertain,
                Some("sampled chance".into()),
            ),
            &AccountId::Nature,
        ),
    ];
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
            focus.map(|location| slug(format!("location/{location:?}"))),
        )
        .with_entities(
            std::iter::once(vehicle)
                .chain(order_entities)
                .chain(system_entities),
        )
        .with_metrics([
            visual_metric(
                slug("delivered"),
                "Orders delivered",
                delivered,
                Some("orders"),
            ),
            visual_metric(
                slug("remaining-time"),
                "Time remaining",
                world.balance(&AccountId::Vehicle, &Asset::TimeRemaining),
                Some("ticks"),
            ),
            visual_metric(
                slug("fuel"),
                "Fuel remaining",
                world.balance(&AccountId::Vehicle, &Asset::Fuel),
                Some("units"),
            ),
        ]),
    )
}

// How Studio names and writes this problem's values.

counted!(logistics::Asset);

impl StudioLabel for logistics::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Vehicle => "Delivery vehicle".into(),
            Self::Order(order) => format!("Order {order:?}"),
            Self::Nature => "Weather and breakdowns".into(),
            Self::FuelStation => "Fuel station".into(),
        }
    }
}

impl StudioLabel for logistics::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::At(location) => format!("Vehicle at {location:?}"),
            Self::Traveling(route) => format!("Traveling on {route:?}"),
            Self::Fuel => "Fuel left".into(),
            Self::SpentFuel => "Fuel spent".into(),
            Self::Money => "Money".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::CargoSpace => "Cargo space available".into(),
            Self::CargoOccupied => "Cargo space occupied".into(),
            Self::RepairTool => "Repair tool".into(),
            Self::Waiting => "Waiting on a travel outcome".into(),
            Self::InTransit => "Order in transit".into(),
            Self::Delivered => "Order delivered".into(),
            Self::Package(order) => format!("Package for Order {order:?}"),
            Self::WeatherReady => "Nature ready for travel".into(),
            Self::OutcomeWeight(route, outcome) => {
                format!("Weight: {route:?} is {outcome:?}")
            }
            Self::Outcome(route, outcome) => format!("{route:?} outcome: {outcome:?}"),
        }
    }
}

impl StudioLabel for logistics::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Load(order) => format!("Load Order {order:?}"),
            Self::Depart(route) => format!("Depart via {route:?}"),
            Self::Resolve(route, outcome) => format!("Draw {route:?} outcome: {outcome:?}"),
            Self::Arrive(route, outcome) => format!("Arrive via {route:?} after {outcome:?}"),
            Self::Repair(route) => format!("Repair after breakdown on {route:?}"),
            Self::Deliver(order) => format!("Deliver Order {order:?}"),
            Self::Refuel => "Refuel the vehicle".into(),
        }
    }
}

debug_labels!(logistics::Role);
