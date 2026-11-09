use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::bridge::{
    self, AccountId, AgentId, Asset, ObjectiveKey, RateId, Role, Side, World,
};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, SceneAnchorView, SceneGlyphView,
    SceneToneView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let profile = run.instance;
    let initial = match profile {
        InstanceProfile::Micro => bridge::initial(),
        InstanceProfile::Showcase => bridge::initial_showcase(),
        InstanceProfile::Stress => bridge::initial_stress(),
    };
    let bfs = bridge::solve(&initial)
        .ok_or_else(|| problem_error("bridge", "BFS found no allocation"))?;
    let first_a = match profile {
        InstanceProfile::Micro => bridge::first_come_proposal(AgentId::A),
        InstanceProfile::Showcase => bridge::first_come_showcase(AgentId::A),
        InstanceProfile::Stress => bridge::first_come_stress(AgentId::A),
    }
    .ok_or_else(|| problem_error("bridge", "first-come A failed"))?;
    let first_b = match profile {
        InstanceProfile::Micro => bridge::first_come_proposal(AgentId::B),
        InstanceProfile::Showcase => bridge::first_come_showcase(AgentId::B),
        InstanceProfile::Stress => bridge::first_come_stress(AgentId::B),
    }
    .ok_or_else(|| problem_error("bridge", "first-come B failed"))?;
    let auction = match profile {
        InstanceProfile::Micro => bridge::auction_proposal(2, 1),
        InstanceProfile::Showcase => bridge::auction_showcase(),
        InstanceProfile::Stress => bridge::auction_stress(),
    }
    .ok_or_else(|| problem_error("bridge", "auction failed"))?;
    let pareto = matches!(profile, InstanceProfile::Micro)
        .then(|| bridge::pareto_front(&initial))
        .transpose()
        .map_err(|error| problem_error("bridge", error))?;
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
        pareto_trace(result, AgentId::A)?
    } else {
        preferred(AgentId::A)
    };
    let pareto_b = if let Some(result) = &pareto {
        pareto_trace(result, AgentId::B)?
    } else {
        preferred(AgentId::B)
    };
    let pareto_expanded = pareto
        .as_ref()
        .map(|result| result.progress().expanded() as u64);
    let mut traces = vec![
        (
            "breadth-first",
            "Bridge · plain search",
            "Breadth-first search over routing choices; timed traffic resolves ready completions immediately and excludes auctions and deliberate reservation expiry.",
            bfs.trace().clone(),
            "breadth-first search",
            Some(bfs.expanded() as u64),
        ),
        (
            "first-come-a",
            "Bridge · Agent A first",
            "Fixed priority waits for Agent A, then gives the other traveler the lane. Arrival times and deadlines remain identical across policies.",
            first_a,
            "first-come mechanism",
            None,
        ),
        (
            "first-come-b",
            "Bridge · Agent B first",
            "Fixed priority waits for Agent B, then gives the other traveler the lane. The wait can waste capacity and make later arrivals late.",
            first_b,
            "first-come mechanism",
            None,
        ),
        (
            "auction",
            "Bridge · auction",
            "In timed traffic, bids respond to deadline slack and retained funds. Both travelers must arrive before settlement; the winner occupies the lane for the crossing duration.",
            auction,
            "auction mechanism",
            None,
        ),
        (
            "pareto-a",
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
            "pareto-b",
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
        traces.push(("adaptive", "Bridge · responsive routing", "Prioritize urgent arrivals and pay for a concurrent detour when bridge congestion would miss a deadline the alternate route can meet.", trace, "deadline-responsive routing", None));
    }
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("bridge", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "bridge",
                strategy,
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
        .map_err(|error| problem_error("bridge", error))?;
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
        view.proposals.push(proposal(ProposalSpec { id: "impersonated-bid", label: "Agent B submits Agent A bid", description: "Agent B tries to bid as Agent A. The rejection names the identity B does not hold." }, &initial, &wrong_identity));
        documents.push(view);
    }
    for index in 0..documents.len() {
        let front = if let Some(result) = &pareto {
            front_view(result, &documents[index])
        } else {
            candidate_front_view(&documents, &documents[index])
        };
        documents[index].pareto_fronts.push(front);
    }
    artifact(run, descriptor, documents)
}

fn candidate_front_view(documents: &[ViewDocument], selected: &ViewDocument) -> ParetoFrontView {
    let rows = documents
        .iter()
        .map(|document| {
            let values = document
                .objectives
                .iter()
                .map(|objective| objective.value.parse::<u64>().unwrap_or(0))
                .collect::<Vec<_>>();
            (document, values)
        })
        .collect::<Vec<_>>();
    let non_dominated =
        rows.iter().filter(|(_, candidate)| {
            !rows.iter().any(|(_, other)| {
                other.iter().zip(candidate).zip(&selected.objectives).all(
                    |((left, right), axis)| match axis.direction {
                        ObjectiveDirectionView::Maximize => left >= right,
                        ObjectiveDirectionView::Minimize => left <= right,
                    },
                ) && other.iter().zip(candidate).zip(&selected.objectives).any(
                    |((left, right), axis)| match axis.direction {
                        ObjectiveDirectionView::Maximize => left > right,
                        ObjectiveDirectionView::Minimize => left < right,
                    },
                )
            })
        });
    ParetoFrontView {
        title: "Every non-dominated evaluated mechanism outcome".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: selected
            .objectives
            .iter()
            .map(|objective| ObjectiveAxisView {
                key: objective.key.clone(),
                label: objective.label.clone(),
                direction: objective.direction,
            })
            .collect(),
        points: non_dominated
            .map(|(document, values)| ParetoPointView {
                label: document.title.clone(),
                selected: document.id == selected.id,
                values: values.iter().map(|value| value.to_string()).collect(),
            })
            .collect(),
    }
}

fn pareto_trace(
    result: &bridge::ParetoResult,
    favored: AgentId,
) -> Result<axionomy::Trace<bridge::Ontology>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .max_by_key(|entry| {
            let value = |key| {
                entry
                    .objectives()
                    .objectives()
                    .iter()
                    .find(|o| o.key() == &key)
                    .map_or(0, |o| *o.value())
            };
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
        .ok_or_else(|| problem_error("bridge", "empty Pareto frontier"))
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    let mut values: Vec<_> = [AgentId::A, AgentId::B]
        .into_iter()
        .flat_map(|agent| {
            [
                ObjectiveView {
                    key: slug(format!("priority-{agent:?}")),
                    label: format!("Agent {agent:?} priority"),
                    direction: ObjectiveDirectionView::Maximize,
                    value: bridge::priority(world, agent).to_string(),
                },
                ObjectiveView {
                    key: slug(format!("credit-{agent:?}")),
                    label: format!("Agent {agent:?} credit"),
                    direction: ObjectiveDirectionView::Maximize,
                    value: bridge::credit(world, agent).to_string(),
                },
            ]
        })
        .collect();
    if bridge::timed::enabled(world) {
        for agent in [AgentId::A, AgentId::B] {
            values.push(ObjectiveView {
                key: slug(format!("ontime-{agent:?}")),
                label: format!("Agent {agent:?} on-time trips"),
                direction: ObjectiveDirectionView::Maximize,
                value: world
                    .balance(&AccountId::Agent(agent), &Asset::OnTime)
                    .to_string(),
            });
        }
        values.push(ObjectiveView {
            key: slug("elapsed"),
            label: "Elapsed time".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: bridge::timed::now(world).to_string(),
        });
    }
    values
}

fn front_view(result: &bridge::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|o| o.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Crossings vs. credit kept".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: ["A priority", "A credit", "B priority", "B credit"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| ObjectiveAxisView {
                key: slug(format!("axis-{index}")),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
            })
            .collect(),
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let values = [
                    ObjectiveKey::Priority(AgentId::A),
                    ObjectiveKey::Credit(AgentId::A),
                    ObjectiveKey::Priority(AgentId::B),
                    ObjectiveKey::Credit(AgentId::B),
                ]
                .into_iter()
                .map(|key| {
                    entry
                        .objectives()
                        .objectives()
                        .iter()
                        .find(|objective| objective.key() == &key)
                        .expect("bridge objective exists")
                        .value()
                        .to_string()
                })
                .collect::<Vec<_>>();
                ParetoPointView {
                    label: format!(
                        "A {}/{} · B {}/{}",
                        values[0], values[1], values[2], values[3]
                    ),
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
    let timed = bridge::timed::enabled(world);
    let clock = bridge::timed::now(world);
    let mut places = vec![
        ("west", "West bank", 70.0, 100.0),
        ("bridge", "Capacity-one bridge", 320.0, 100.0),
        ("east", "East bank", 570.0, 100.0),
    ];
    if timed {
        places.push(("detour", "Paid alternate route", 320.0, 300.0));
    }
    let nodes = places
        .into_iter()
        .map(|(key, label, x, y)| GraphNodeView {
            id: ViewId::new(slug(key), label),
            classes: if key == "bridge" {
                vec![slug("resource")]
            } else {
                vec![]
            },
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let mut routes = vec![
        ("west", "bridge", "reserve lane".to_string()),
        (
            "bridge",
            "east",
            if timed {
                "A: 3 ticks · B: 2 ticks".into()
            } else {
                "cross atomically".into()
            },
        ),
    ];
    if timed {
        routes.extend([
            ("west", "detour", "1 credit".into()),
            ("detour", "east", "3 ticks · independent capacity".into()),
        ]);
    }
    let edges = routes
        .into_iter()
        .map(|(from, to, label)| GraphEdgeView {
            id: slug(format!("route/{from}/{to}")),
            source: slug(from),
            target: slug(to),
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
                "bridge",
                Asset::InTransit,
                format!("crossing until {}", value(Asset::ReadyAt)),
            )
        } else if detour {
            (
                "detour",
                Asset::OnDetour,
                format!("detouring until {}", value(Asset::ReadyAt)),
            )
        } else if east {
            ("east", Asset::At(Side::East), "arrived".into())
        } else if timed && clock < value(Asset::Arrival) {
            (
                "west",
                Asset::At(Side::West),
                format!("arrives at {}", value(Asset::Arrival)),
            )
        } else if value(Asset::CrossingRight) > 0 {
            (
                "west",
                Asset::At(Side::West),
                format!("reserved through {}", value(Asset::LeaseUntil)),
            )
        } else if value(Asset::Submitted) > 0 {
            ("west", Asset::At(Side::West), "bid in escrow".into())
        } else {
            ("west", Asset::At(Side::West), "waiting".into())
        };
        let mut entity = link_balance::<bridge::Ontology>(
            visual_entity(
                slug(format!("traveler/{agent:?}")),
                format!("Agent {agent:?}"),
                SceneGlyphView::Agent,
                SceneAnchorView::GraphNode { node: slug(node) },
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
            slug("credit"),
            "Credit",
            &Asset::Credit,
            bridge::credit(world, agent),
        )];
        if timed {
            entity.metrics.extend([
                asset_metric(
                    slug("deadline"),
                    "Deadline",
                    &Asset::Deadline,
                    value(Asset::Deadline),
                ),
                asset_metric(
                    slug("ontime"),
                    "On-time trips",
                    &Asset::OnTime,
                    value(Asset::OnTime),
                ),
                asset_metric(slug("late"), "Late trips", &Asset::Late, value(Asset::Late)),
            ]);
        }
        entity
    });
    let mut scene = Scene::graph("Timed traffic and one shared lane", nodes, edges, None)
        .with_entities(agents)
        .with_metrics([
            visual_metric(
                slug("capacity"),
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
                slug("priority-a"),
                "A priority",
                bridge::priority(world, AgentId::A),
                None,
            ),
            visual_metric(
                slug("priority-b"),
                "B priority",
                bridge::priority(world, AgentId::B),
                None,
            ),
        ]);
    if timed {
        scene.metrics.extend([
            asset_metric(slug("world-time"), "World time", &Asset::Now, clock),
            visual_metric(
                slug("wave"),
                "Traffic wave",
                world.balance(&AccountId::Bridge, &Asset::Round).to_u64() + 1,
                None,
            ),
            visual_metric(
                slug("ontime"),
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
                slug("late"),
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
    use crate::{ReferenceService, RunRequest};
    #[test]
    fn exact_front_pairs_each_travelers_priority_with_their_credit() {
        let artifact = ReferenceService
            .run(RunRequest::new(slug("bridge")).with_instance(InstanceProfile::Micro))
            .unwrap();
        for document in artifact.documents {
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
                            .map(|o| o.value.clone())
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
