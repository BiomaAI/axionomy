//! The bridge allocation as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    bridge::{self, AccountId, AgentId, Asset, ObjectiveKey, RateId, Role, World},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::{Exchange, Quantity, path};
use axionomy_study::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProblemPath, ProposalSpec, StudyError, StudyKey, StudyProgress, document, problem_error,
    proposal, telemetry,
};
use axionomy_view::{
    Keyed, ObjectiveAxisView, ObjectiveDirectionView, ObjectiveView, ParetoFrontView,
    TelemetryKindView, ViewDocument, evaluated_front, exact_front, objective_value,
};

/// Bridge allocation: its catalog entry and how its studies are computed.
pub struct Bridge {
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
    ) -> Result<Vec<ViewDocument>, StudyError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
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
) -> Result<axionomy::Trace<bridge::Ontology>, StudyError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;
    use axionomy_study::StudyRequest;
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
