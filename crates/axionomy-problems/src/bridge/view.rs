//! How the bridge allocation is presented: its labels, units, scene and leaderboards.

use crate::bridge::{self, AccountId, AgentId, Asset, Side, World};
use axionomy::path;
use axionomy_view::{
    Format, GraphEdgeView, GraphNodeView, Keyed, Label, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, ViewId, asset_metric, debug_labels,
    link_balance, visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let timed = bridge::timed::enabled(world);
    let clock = bridge::timed::now(world);
    let mut places = vec![
        (path!("west"), "West bank", 70.0, 100.0),
        (path!("bridge"), "Capacity-one bridge", 320.0, 100.0),
        (path!("east"), "East bank", 570.0, 100.0),
    ];
    if timed {
        places.push((path!("detour"), "Paid alternate route", 320.0, 300.0));
    }
    let nodes = places
        .into_iter()
        .map(|(key, label, x, y)| GraphNodeView {
            classes: if key == path!("bridge") {
                vec![path!("resource")]
            } else {
                vec![]
            },
            id: ViewId::new(key, label),
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let mut routes = vec![
        (path!("west"), path!("bridge"), "reserve lane".to_string()),
        (
            path!("bridge"),
            path!("east"),
            if timed {
                "A: 3 ticks · B: 2 ticks".into()
            } else {
                "cross atomically".into()
            },
        ),
    ];
    if timed {
        routes.extend([
            (path!("west"), path!("detour"), "1 credit".into()),
            (
                path!("detour"),
                path!("east"),
                "3 ticks · independent capacity".into(),
            ),
        ]);
    }
    let edges = routes
        .into_iter()
        .map(|(from, to, label)| GraphEdgeView {
            id: path!("route").join(&from).join(&to),
            source: from,
            target: to,
            label: Some(label),
            classes: vec![],
        })
        .collect();
    let agents = [AgentId::A, AgentId::B].into_iter().map(|agent| {
        let value = |asset| world.balance(&AccountId::Agent(agent), &asset).to_u64();
        let east = value(Asset::At(Side::East)) > 0;
        let crossing = value(Asset::InTransit) > 0;
        let detour = value(Asset::OnDetour) > 0;
        let (node, asset, status) = if crossing {
            (
                path!("bridge"),
                Asset::InTransit,
                format!("crossing until {}", value(Asset::ReadyAt)),
            )
        } else if detour {
            (
                path!("detour"),
                Asset::OnDetour,
                format!("detouring until {}", value(Asset::ReadyAt)),
            )
        } else if east {
            (path!("east"), Asset::At(Side::East), "arrived".into())
        } else if timed && clock < value(Asset::Arrival) {
            (
                path!("west"),
                Asset::At(Side::West),
                format!("arrives at {}", value(Asset::Arrival)),
            )
        } else if value(Asset::CrossingRight) > 0 {
            (
                path!("west"),
                Asset::At(Side::West),
                format!("reserved through {}", value(Asset::LeaseUntil)),
            )
        } else if value(Asset::Submitted) > 0 {
            (path!("west"), Asset::At(Side::West), "bid in escrow".into())
        } else {
            (path!("west"), Asset::At(Side::West), "waiting".into())
        };
        let mut entity = link_balance::<bridge::Ontology>(
            visual_entity(
                path!("traveler").then(&agent),
                format!("Agent {agent:?}"),
                SceneGlyphView::Agent,
                SceneAnchorView::GraphNode { node },
                SceneEntityRoleView::Occupant,
                if east {
                    SceneToneView::Success
                } else {
                    SceneToneView::Active
                },
                Some(status),
            ),
            &AccountId::Agent(agent),
            &asset,
        );
        entity.metrics = vec![asset_metric(
            path!("credit"),
            "Credit",
            &Asset::Credit,
            bridge::credit(world, agent),
        )];
        if timed {
            entity.metrics.extend([
                asset_metric(
                    path!("deadline"),
                    "Deadline",
                    &Asset::Deadline,
                    value(Asset::Deadline),
                ),
                asset_metric(
                    path!("ontime"),
                    "On-time trips",
                    &Asset::OnTime,
                    value(Asset::OnTime),
                ),
                asset_metric(
                    path!("late"),
                    "Late trips",
                    &Asset::Late,
                    value(Asset::Late),
                ),
            ]);
        }
        entity
    });
    let mut scene = Scene::graph("Timed traffic and one shared lane", nodes, edges, None)
        .with_entities(agents)
        .with_metrics([
            visual_metric(
                path!("capacity"),
                "Bridge capacity",
                if world
                    .balance(&AccountId::Bridge, &Asset::CapacityFree)
                    .is_zero()
                {
                    "reserved / occupied"
                } else {
                    "available"
                },
                None,
            ),
            visual_metric(
                path!("priority-a"),
                "A priority",
                bridge::priority(world, AgentId::A),
                None,
            ),
            visual_metric(
                path!("priority-b"),
                "B priority",
                bridge::priority(world, AgentId::B),
                None,
            ),
        ]);
    if timed {
        scene.metrics.extend([
            asset_metric(path!("world-time"), "World time", &Asset::Now, clock),
            visual_metric(
                path!("wave"),
                "Traffic wave",
                world.balance(&AccountId::Bridge, &Asset::Round).to_u64() + 1,
                None,
            ),
            visual_metric(
                path!("ontime"),
                "On-time trips",
                [AgentId::A, AgentId::B]
                    .into_iter()
                    .map(|agent| {
                        world
                            .balance(&AccountId::Agent(agent), &Asset::OnTime)
                            .to_u64()
                    })
                    .sum::<u64>(),
                None,
            ),
            visual_metric(
                path!("late"),
                "Late trips",
                [AgentId::A, AgentId::B]
                    .into_iter()
                    .map(|agent| {
                        world
                            .balance(&AccountId::Agent(agent), &Asset::Late)
                            .to_u64()
                    })
                    .sum::<u64>(),
                None,
            ),
        ]);
    }
    Some(scene)
}

// How Studio names and writes this problem's values.

/// Energy reads in units and credit in credits; every time the bridge keeps
/// is a tick of its world clock.
impl Format for bridge::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy => QuantityFormat::COUNT.unit("units"),
            Self::Credit | Self::Escrow | Self::SpentCredit => {
                QuantityFormat::COUNT.unit("credits")
            }
            Self::Now
            | Self::TimeRemaining
            | Self::Arrival
            | Self::Deadline
            | Self::LeaseUntil
            | Self::ReadyAt => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for bridge::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("Agent {agent:?}"),
            Self::Bridge => "Single-lane bridge".into(),
        }
    }
}

impl Label for bridge::Asset {
    fn label(&self) -> String {
        match self {
            Self::AgentIdentity(agent) => format!("Identity: Agent {agent:?}"),
            Self::BridgeIdentity => "Bridge identity".into(),
            Self::At(Side::West) => "At the west bank".into(),
            Self::At(Side::East) => "At the east bank".into(),
            Self::Energy => "Crossing energy".into(),
            Self::SpentEnergy => "Energy spent crossing".into(),
            Self::Credit => "Credit available".into(),
            Self::Escrow => "Credit in escrow".into(),
            Self::SpentCredit => "Credit spent".into(),
            Self::Bid(1) => "Bid of 1 credit".into(),
            Self::Bid(amount) => format!("Bid of {amount} credits"),
            Self::CanBid => "Can bid or claim".into(),
            Self::Submitted => "Bid submitted".into(),
            Self::CrossingRight => "Crossing right".into(),
            Self::FirstTurn => "First crossing open".into(),
            Self::SecondTurn => "Second crossing open".into(),
            Self::PriorityBenefit => "Priority crossings won".into(),
            Self::Waiting => "Waiting for the lane".into(),
            Self::Crossed => "Crossed this round".into(),
            Self::CompletedTrip => "Completed prior rounds".into(),
            Self::RoundsRemaining => "Rounds remaining after this one".into(),
            Self::CapacityFree => "Lane available".into(),
            Self::Active => "Allocation in progress".into(),
            Self::Solved => "Allocation complete".into(),
            Self::Now => "World time".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::Arrival => "Arrival time".into(),
            Self::Deadline => "Arrival deadline".into(),
            Self::LeaseUntil => "Reservation valid through".into(),
            Self::ReadyAt => "Travel completes at".into(),
            Self::InTransit => "On the bridge".into(),
            Self::OnDetour => "On the alternate route".into(),
            Self::Detouring => "Detouring".into(),
            Self::OnTime => "On-time trips".into(),
            Self::Late => "Late trips".into(),
            Self::MissedReservation => "Expired reservations".into(),
            Self::Round => "Prior traffic waves".into(),
        }
    }
}

impl Label for bridge::RateId {
    fn label(&self) -> String {
        match self {
            Self::SubmitBid { agent, amount } => format!("Agent {agent:?} bids {amount}"),
            Self::Resolve {
                winner,
                winning_bid,
                losing_bid,
            } => format!("Settle auction: Agent {winner:?} wins {winning_bid} to {losing_bid}"),
            Self::ClaimFirst { agent } => format!("Agent {agent:?} claims first crossing"),
            Self::ClaimSecond { agent } => format!("Agent {agent:?} claims second crossing"),
            Self::YieldToWaiting { agent } => format!("Grant waiting Agent {agent:?} the lane"),
            Self::Cross { agent } => format!("Agent {agent:?} crosses"),
            Self::ResetRound => "Start the next allocation round".into(),
            Self::Finish => "Complete all crossing rounds".into(),
            Self::Advance { to } => format!("Advance world time to {to}"),
            Self::Complete {
                agent,
                detour,
                late,
            } => format!(
                "Agent {agent:?} finishes {} {}",
                if *detour { "detour" } else { "crossing" },
                if *late { "late" } else { "on time" }
            ),
            Self::Detour {
                agent,
                waiting: false,
            } => format!("Agent {agent:?} skips the lane and pays for the alternate route"),
            Self::Detour {
                agent,
                waiting: true,
            } => format!("Agent {agent:?} stops waiting and pays for the alternate route"),
            Self::ExpireReservation { agent } => {
                format!("Release Agent {agent:?}'s expired reservation")
            }
            Self::WithdrawBid { agent, amount } => {
                format!("Agent {agent:?} withdraws bid {amount}")
            }
        }
    }
}

debug_labels!(bridge::Role);
