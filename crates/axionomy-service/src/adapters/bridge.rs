use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::bridge::{
    self, AccountId, AgentId, Asset, ObjectiveKey, RateId, Role, Side, World,
};
use axionomy_view::{
    GraphEdgeView, GraphNodeView, ObjectiveAxisView, ObjectiveDirectionView, ParetoFrontView,
    SceneAnchorView, SceneGlyphView, SceneToneView, TelemetryKindView, ViewId,
};

/// Bridge allocation: its catalog entry and how its studies are computed.
pub(crate) struct Bridge {
    descriptor: ProblemDescriptor,
}

impl Default for Bridge {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("bridge"),
                    title: "Bridge allocation",
                    summary: "Timed traffic competes for one occupied lane. Compare fixed priority, urgency-based bids and a paid concurrent detour against arrivals, deadlines and expiring reservations.",
                    instances: [
                        "One crossing round for two agents and one lane",
                        "Two traffic waves with staggered arrivals, 2–3 tick crossings, deadlines and a paid detour",
                        "Three traffic waves with persistent credit and changing deadline pressure",
                    ],
                },
                ProblemFamily::Allocation,
                path!("auction"),
                &[
                    StrategyCopy {
                        key: path!("breadth-first"),
                        label: "Plain search",
                        description: "Breadth-first routing search with immediate ready completions; timed search excludes bidding and deliberate lease expiry.",
                        algorithm: "breadth-first search",
                    },
                    StrategyCopy {
                        key: path!("first-come-a"),
                        label: "First come: A",
                        description: "Wait for Agent A, give A first priority, then serve B.",
                        algorithm: "first-come allocation",
                    },
                    StrategyCopy {
                        key: path!("first-come-b"),
                        label: "First come: B",
                        description: "Wait for Agent B, give B first priority, then serve A.",
                        algorithm: "first-come allocation",
                    },
                    StrategyCopy {
                        key: path!("auction"),
                        label: "Auction",
                        description: "Bids, winner, escrow, refunds, and crossing rights settle through indivisible changes.",
                        algorithm: "auction mechanism",
                    },
                    StrategyCopy {
                        key: path!("pareto-a"),
                        label: "Pareto: favors A",
                        description: "Micro: exact frontier. Timed traffic: A's preference among the evaluated routing and auction policies.",
                        algorithm: "exact Pareto search",
                    },
                    StrategyCopy {
                        key: path!("pareto-b"),
                        label: "Pareto: favors B",
                        description: "Micro: exact frontier. Timed traffic: B's preference among the evaluated routing and auction policies.",
                        algorithm: "exact Pareto search",
                    },
                ],
                &[
                    Capability::DeterministicSearch,
                    Capability::AtomicSettlement,
                    Capability::ExactPareto,
                    Capability::MultiAccountExchange,
                ],
            ),
        }
    }
}

impl Problem for Bridge {
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

pub(super) fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let profile = study.instance;
    let initial = match profile {
        InstanceProfile::Micro => bridge::initial(),
        InstanceProfile::Showcase => bridge::initial_showcase(),
        InstanceProfile::Stress => bridge::initial_stress(),
    };
    let bfs = bridge::solve(&initial)
        .ok_or_else(|| problem_error(&study.problem, "BFS found no allocation"))?;
    let first_a = match profile {
        InstanceProfile::Micro => bridge::first_come_proposal(AgentId::A),
        InstanceProfile::Showcase => bridge::first_come_showcase(AgentId::A),
        InstanceProfile::Stress => bridge::first_come_stress(AgentId::A),
    }
    .ok_or_else(|| problem_error(&study.problem, "first-come A failed"))?;
    let first_b = match profile {
        InstanceProfile::Micro => bridge::first_come_proposal(AgentId::B),
        InstanceProfile::Showcase => bridge::first_come_showcase(AgentId::B),
        InstanceProfile::Stress => bridge::first_come_stress(AgentId::B),
    }
    .ok_or_else(|| problem_error(&study.problem, "first-come B failed"))?;
    let auction = match profile {
        InstanceProfile::Micro => bridge::auction_proposal(2, 1),
        InstanceProfile::Showcase => bridge::auction_showcase(),
        InstanceProfile::Stress => bridge::auction_stress(),
    }
    .ok_or_else(|| problem_error(&study.problem, "auction failed"))?;
    let pareto = matches!(profile, InstanceProfile::Micro)
        .then(|| bridge::pareto_front(&initial))
        .transpose()
        .map_err(|error| problem_error(&study.problem, error))?;
    let adaptive = bridge::timed::enabled(&initial).then(|| {
        bridge::timed::run(&initial, bridge::timed::Policy::Adaptive)
            .expect("adaptive traffic completes")
    });
    let preferred = |favored: AgentId| {
        [&first_a, &first_b, &auction]
            .into_iter()
            .chain(adaptive.as_ref())
            .max_by_key(|trace| {
                let final_world = initial.replayed(trace).expect("validated traffic");
                (
                    final_world
                        .balance(&AccountId::Agent(favored), &Asset::OnTime)
                        .to_u64(),
                    bridge::priority(&final_world, favored),
                    bridge::credit(&final_world, favored),
                )
            })
            .unwrap()
            .clone()
    };
    let pareto_a = if let Some(result) = &pareto {
        pareto_trace(result, AgentId::A, &study.problem)?
    } else {
        preferred(AgentId::A)
    };
    let pareto_b = if let Some(result) = &pareto {
        pareto_trace(result, AgentId::B, &study.problem)?
    } else {
        preferred(AgentId::B)
    };
    let pareto_expanded = pareto
        .as_ref()
        .map(|result| result.progress().expanded() as u64);
    let mut traces = vec![
        (
            path!("breadth-first"),
            "Bridge · plain search",
            "Breadth-first search over routing choices; timed traffic resolves ready completions immediately and excludes auctions and deliberate reservation expiry.",
            bfs.trace().clone(),
            "breadth-first search",
            Some(bfs.expanded() as u64),
        ),
        (
            path!("first-come-a"),
            "Bridge · Agent A first",
            "Fixed priority waits for Agent A, then gives the other traveler the lane. Arrival times and deadlines remain identical across policies.",
            first_a,
            "first-come mechanism",
            None,
        ),
        (
            path!("first-come-b"),
            "Bridge · Agent B first",
            "Fixed priority waits for Agent B, then gives the other traveler the lane. The wait can waste capacity and make later arrivals late.",
            first_b,
            "first-come mechanism",
            None,
        ),
        (
            path!("auction"),
            "Bridge · auction",
            "In timed traffic, bids respond to deadline slack and retained funds. Both travelers must arrive before settlement; the winner occupies the lane for the crossing duration.",
            auction,
            "auction mechanism",
            None,
        ),
        (
            path!("pareto-a"),
            if pareto.is_some() {
                "Bridge Pareto · Agent A"
            } else {
                "Bridge preference · Agent A"
            },
            "Micro uses the exact frontier. Larger traffic chooses the best evaluated policy for A, prioritizing on-time trips, then priority and retained credit.",
            pareto_a,
            if pareto.is_some() {
                "exact Pareto search"
            } else {
                "evaluated policy preference"
            },
            pareto_expanded,
        ),
        (
            path!("pareto-b"),
            if pareto.is_some() {
                "Bridge Pareto · Agent B"
            } else {
                "Bridge preference · Agent B"
            },
            "Micro uses the exact frontier. Larger traffic chooses the best evaluated policy for B, prioritizing on-time trips, then priority and retained credit.",
            pareto_b,
            if pareto.is_some() {
                "exact Pareto search"
            } else {
                "evaluated policy preference"
            },
            pareto_expanded,
        ),
    ];
    if let Some(trace) = adaptive {
        traces.push((path!("adaptive"), "Bridge · responsive routing", "Prioritize urgent arrivals and pay for a concurrent detour when bridge congestion would miss a deadline the alternate route can meet.", trace, "deadline-responsive routing", None));
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
                source_label: "Bridge allocation",
            },
            &initial,
            &bridge::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.telemetry.push(telemetry(
            algorithm,
            true,
            expanded
                .into_iter()
                .map(|value| (TelemetryKindView::Expanded, value, "states explored".into()))
                .chain([(
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "mechanism exchanges".into(),
                )]),
        ));
        let wrong_identity = Exchange::new(
            RateId::SubmitBid {
                agent: AgentId::A,
                amount: 2,
            },
            Quantity::new(1),
        )
        .bind(Role::Traveler, AccountId::Agent(AgentId::B))
        .bind(Role::Bridge, AccountId::Bridge);
        view.proposals.push(proposal(ProposalSpec { id: path!("impersonated-bid"), label: "Agent B submits Agent A bid", description: "Agent B tries to bid as Agent A. The rejection names the identity B does not hold." }, &initial, &wrong_identity));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = if let Some(result) = &pareto {
            front_view(result, &documents[index])
        } else {
            evaluated_front(
                &documents,
                &documents[index],
                "Every non-dominated evaluated mechanism outcome",
            )
        };
        documents[index].pareto_fronts.push(front);
    }
    Ok(documents)
}

fn pareto_trace(
    result: &bridge::ParetoResult,
    favored: AgentId,
    problem: &ProblemPath,
) -> Result<axionomy::Trace<bridge::Ontology>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .max_by_key(|entry| {
            let value = |key| objective_value(entry, &key);
            let other = if favored == AgentId::A {
                AgentId::B
            } else {
                AgentId::A
            };
            (
                value(ObjectiveKey::Priority(favored)),
                value(ObjectiveKey::Credit(favored)),
                value(ObjectiveKey::Priority(other)),
                value(ObjectiveKey::Credit(other)),
            )
        })
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error(problem, "empty Pareto frontier"))
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    let mut values: Vec<_> = [AgentId::A, AgentId::B]
        .into_iter()
        .flat_map(|agent| {
            [
                ObjectiveView::new(
                    path!("priority").then(&agent),
                    format!("Agent {agent:?} priority"),
                    ObjectiveDirectionView::Maximize,
                    bridge::priority(world, agent),
                ),
                ObjectiveView::new(
                    path!("credit").then(&agent),
                    format!("Agent {agent:?} credit"),
                    ObjectiveDirectionView::Maximize,
                    bridge::credit(world, agent),
                ),
            ]
        })
        .collect();
    if bridge::timed::enabled(world) {
        for agent in [AgentId::A, AgentId::B] {
            values.push(ObjectiveView::new(
                path!("ontime").then(&agent),
                format!("Agent {agent:?} on-time trips"),
                ObjectiveDirectionView::Maximize,
                world.balance(&AccountId::Agent(agent), &Asset::OnTime),
            ));
        }
        values.push(ObjectiveView::new(
            path!("elapsed"),
            "Elapsed time",
            ObjectiveDirectionView::Minimize,
            bridge::timed::now(world),
        ));
    }
    values
}

fn front_view(result: &bridge::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let keys = [
        ObjectiveKey::Priority(AgentId::A),
        ObjectiveKey::Credit(AgentId::A),
        ObjectiveKey::Priority(AgentId::B),
        ObjectiveKey::Credit(AgentId::B),
    ];
    exact_front(
        "Crossings vs. credit kept",
        ["A priority", "A credit", "B priority", "B credit"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| ObjectiveAxisView {
                key: path!("axis").then(&index),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
            })
            .collect(),
        selected,
        result.front().entries().iter().map(|entry| {
            keys.iter()
                .map(|key| Quantity::from(objective_value(entry, key)))
                .collect()
        }),
        |values| {
            format!(
                "A {}/{} · B {}/{}",
                values[0], values[1], values[2], values[3]
            )
        },
    )
}

fn scene(_: u64, world: &World) -> Option<Scene> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StudyRequest;
    #[test]
    fn exact_front_pairs_each_travelers_priority_with_their_credit() {
        let study = registry()
            .study(StudyRequest::new(path!("bridge")).with_instance(InstanceProfile::Micro))
            .unwrap();
        for document in study.documents {
            for point in &document.pareto_fronts[0].points {
                let values: Vec<u64> = point
                    .values
                    .iter()
                    .map(|value| value.parse().unwrap())
                    .collect();
                assert_eq!(values[0] + values[2], 1);
                assert_eq!((values[1], values[3]), (2, 2));
                if point.selected {
                    assert_eq!(
                        point.values,
                        document
                            .objectives
                            .iter()
                            .map(|o| o.value.to_string())
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

// How Studio names and writes this problem's values.

/// Energy reads in units and credit in credits; every time the bridge keeps
/// is a tick of its world clock.
impl StudioFormat for bridge::Asset {
    fn studio_format(&self) -> QuantityFormat {
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

impl StudioLabel for bridge::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("Agent {agent:?}"),
            Self::Bridge => "Single-lane bridge".into(),
        }
    }
}

impl StudioLabel for bridge::Asset {
    fn studio_label(&self) -> String {
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

impl StudioLabel for bridge::RateId {
    fn studio_label(&self) -> String {
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
