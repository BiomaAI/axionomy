//! How stochastic logistics is presented: its labels, units, scene and leaderboards.

use crate::logistics::{self, AccountId, Asset, Location, Route, World};
use axionomy::path;
use axionomy_view::{
    Format, GraphEdgeView, GraphNodeView, Keyed, Label, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, ViewId, asset_metric, link_account,
    link_balance, visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
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

impl Format for logistics::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Fuel | Self::SpentFuel => QuantityFormat::COUNT.unit("units"),
            Self::Money => QuantityFormat::COUNT.unit("credits"),
            Self::TimeRemaining | Self::ElapsedTime => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for logistics::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Vehicle => "Delivery vehicle".into(),
            Self::Order(order) => format!("Order {order:?}"),
            Self::Nature => "Weather and breakdowns".into(),
            Self::FuelStation => "Fuel station".into(),
        }
    }
}

impl Label for logistics::Asset {
    fn label(&self) -> String {
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

impl Label for logistics::RateId {
    fn label(&self) -> String {
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

impl Label for logistics::Role {
    fn label(&self) -> String {
        match self {
            Self::Vehicle => "Vehicle",
            Self::Order => "Order",
            Self::Nature => "Nature",
            Self::FuelStation => "Fuel station",
        }
        .into()
    }
}
