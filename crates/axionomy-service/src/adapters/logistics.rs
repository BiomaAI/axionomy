use super::*;
use axionomy_search::rollout::Unrecorded;
mod fleet;
use axionomy::{Exchange, Quantity};
use axionomy_problems::logistics::{self, AccountId, Asset, Location, Policy, Route, World};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return fleet::build(study, descriptor, profile, progress);
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
    progress.ensure()?;
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
    progress.ensure()?;
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
    progress.ensure()?;
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
    assemble(study, descriptor, documents)
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
    ParetoFrontView {
        title: "Completion vs. deliveries vs. time — sampled, not exact".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: vec![
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
            id: ViewId::new(path!("location").then(&location), format!("{location:?}")),
            classes: if focus == Some(location) {
                vec![path!("current")]
            } else if location == Location::Customer {
                vec![path!("goal")]
            } else {
                Vec::new()
            },
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let edges = logistics::ROUTES
        .into_iter()
        .map(|route| {
            let (from, to) = route_ends(route);
            let traveling = !world
                .balance(&AccountId::Vehicle, &Asset::Traveling(route))
                .is_zero();
            GraphEdgeView {
                id: path!("route").then(&route),
                source: path!("location").then(&from),
                target: path!("location").then(&to),
                label: Some(
                    match route {
                        Route::DirectOut => "Direct, outbound",
                        Route::DirectBack => "Direct, return",
                        Route::SafeOutFirst | Route::SafeOutSecond => "Safe, outbound",
                        Route::SafeBackFirst | Route::SafeBackSecond => "Safe, return",
                    }
                    .into(),
                ),
                classes: if traveling {
                    vec![path!("current"), path!("uncertain")]
                } else {
                    vec![path!("uncertain")]
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
            node: path!("location").then(&location),
        }
    } else if let Some(route) = traveling {
        SceneAnchorView::GraphEdge {
            edge: path!("route").then(&route),
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
            path!("vehicle/fleet-1"),
            "Vehicle 1",
            SceneGlyphView::Vehicle,
            vehicle_anchor.clone(),
            SceneEntityRoleView::Occupant,
            SceneToneView::Active,
            traveling.map(|route| format!("traveling the {}", road(route))),
        ),
        &AccountId::Vehicle,
        &vehicle_asset,
    );
    vehicle.metrics = vec![
        asset_metric(
            path!("fuel"),
            "Fuel",
            &Asset::Fuel,
            world.balance(&AccountId::Vehicle, &Asset::Fuel),
        ),
        asset_metric(
            path!("cargo"),
            "Orders aboard",
            &Asset::CargoOccupied,
            world.balance(&AccountId::Vehicle, &Asset::CargoOccupied),
        ),
        asset_metric(
            path!("elapsed"),
            "Elapsed",
            &Asset::ElapsedTime,
            world.balance(&AccountId::Vehicle, &Asset::ElapsedTime),
        ),
    ];
    let order_entities = logistics::ORDERS.into_iter().map(|order| {
        let order_account = AccountId::Order(order);
        let (anchor, tone, status) = if !world.balance(&order_account, &Asset::Delivered).is_zero()
        {
            (
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&Location::Customer),
                },
                SceneToneView::Success,
                "delivered",
            )
        } else if !world.balance(&order_account, &Asset::InTransit).is_zero() {
            (vehicle_anchor.clone(), SceneToneView::Active, "in transit")
        } else {
            (
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&Location::Depot),
                },
                SceneToneView::Neutral,
                "waiting",
            )
        };
        link_account::<logistics::Ontology>(
            visual_entity(
                path!("order").then(&order),
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
                path!("system/fuel-station"),
                "Fuel station",
                SceneGlyphView::Fuel,
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&Location::Depot),
                },
                SceneEntityRoleView::State,
                SceneToneView::Neutral,
                Some("refuel supply".into()),
            ),
            &AccountId::FuelStation,
        ),
        link_account::<logistics::Ontology>(
            visual_entity(
                path!("system/nature"),
                "Travel conditions",
                SceneGlyphView::Weather,
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&Location::Junction),
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
            focus.map(|location| path!("location").then(&location)),
        )
        .with_entities(
            std::iter::once(vehicle)
                .chain(order_entities)
                .chain(system_entities),
        )
        .with_metrics([
            visual_metric(
                path!("delivered"),
                "Orders delivered",
                delivered,
                Some("orders"),
            ),
            asset_metric(
                path!("remaining-time"),
                "Time remaining",
                &Asset::TimeRemaining,
                world.balance(&AccountId::Vehicle, &Asset::TimeRemaining),
            ),
            asset_metric(
                path!("fuel"),
                "Fuel remaining",
                &Asset::Fuel,
                world.balance(&AccountId::Vehicle, &Asset::Fuel),
            ),
        ]),
    )
}

fn route_ends(route: Route) -> (Location, Location) {
    match route {
        Route::DirectOut => (Location::Depot, Location::Customer),
        Route::DirectBack => (Location::Customer, Location::Depot),
        Route::SafeOutFirst => (Location::Depot, Location::Junction),
        Route::SafeOutSecond => (Location::Junction, Location::Customer),
        Route::SafeBackFirst => (Location::Customer, Location::Junction),
        Route::SafeBackSecond => (Location::Junction, Location::Depot),
    }
}

// How Studio names and writes this problem's values.

/// A route as people read it: whether it is the risky direct road or a leg
/// of the safe road through the junction, and where it runs.
fn road(route: Route) -> String {
    let (from, to) = route_ends(route);
    let kind = if matches!(route, Route::DirectOut | Route::DirectBack) {
        "direct"
    } else {
        "safe"
    };
    format!("{kind} road {from:?} → {to:?}")
}

fn travel(outcome: logistics::TravelOutcome) -> &'static str {
    match outcome {
        logistics::TravelOutcome::Clear => "a clear trip",
        logistics::TravelOutcome::Delayed => "a delay",
        logistics::TravelOutcome::Breakdown => "a breakdown",
    }
}

impl StudioFormat for logistics::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Fuel | Self::SpentFuel => QuantityFormat::COUNT.unit("units"),
            Self::Money => QuantityFormat::COUNT.unit("credits"),
            Self::TimeRemaining | Self::ElapsedTime => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

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
            Self::Traveling(route) => format!("Traveling the {}", road(*route)),
            Self::Fuel => "Fuel left".into(),
            Self::SpentFuel => "Fuel spent".into(),
            Self::Money => "Money".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::CargoSpace => "Cargo space available".into(),
            Self::CargoOccupied => "Cargo space occupied".into(),
            Self::RepairTool => "Repair tool".into(),
            Self::Waiting => "Order waiting at the depot".into(),
            Self::InTransit => "Order in transit".into(),
            Self::Delivered => "Order delivered".into(),
            Self::Package(order) => format!("Package for Order {order:?}"),
            Self::WeatherReady => "Ready to draw travel conditions".into(),
            Self::OutcomeWeight(route, outcome) => {
                format!("Odds of {} on the {}", travel(*outcome), road(*route))
            }
            Self::Outcome(route, outcome) => {
                format!("Drawn: {} on the {}", travel(*outcome), road(*route))
            }
        }
    }
}

impl StudioLabel for logistics::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Load(order) => format!("Load Order {order:?}"),
            Self::Depart(route) => format!("Depart on the {}", road(*route)),
            Self::Resolve(route, outcome) => {
                format!("Draw {} on the {}", travel(*outcome), road(*route))
            }
            Self::Arrive(route, outcome) => {
                format!("Arrive via the {} after {}", road(*route), travel(*outcome))
            }
            Self::Repair(route) => format!("Repair a breakdown on the {}", road(*route)),
            Self::Deliver(order) => format!("Deliver Order {order:?}"),
            Self::Refuel => "Refuel the vehicle".into(),
        }
    }
}

impl StudioLabel for logistics::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::Vehicle => "Vehicle",
            Self::Order => "Order",
            Self::Nature => "Nature",
            Self::FuelStation => "Fuel station",
        }
        .into()
    }
}
