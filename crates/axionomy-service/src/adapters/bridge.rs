use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::bridge::{
    self, AccountId, AgentId, Asset, ObjectiveKey, RateId, Role, Side, World,
};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let showcase = !matches!(
        instance_profile(request, descriptor),
        InstanceProfile::Micro
    );
    let initial = if showcase {
        bridge::initial_showcase()
    } else {
        bridge::initial()
    };
    let bfs = bridge::solve(&initial)
        .ok_or_else(|| problem_error("bridge", "BFS found no allocation"))?;
    let first_a = if showcase {
        bridge::first_come_showcase(AgentId::A)
    } else {
        bridge::first_come_proposal(AgentId::A)
    }
    .ok_or_else(|| problem_error("bridge", "first-come A failed"))?;
    let first_b = if showcase {
        bridge::first_come_showcase(AgentId::B)
    } else {
        bridge::first_come_proposal(AgentId::B)
    }
    .ok_or_else(|| problem_error("bridge", "first-come B failed"))?;
    let auction = if showcase {
        bridge::auction_showcase()
    } else {
        bridge::auction_proposal(2, 1)
    }
    .ok_or_else(|| problem_error("bridge", "auction failed"))?;
    let pareto = (!showcase)
        .then(|| bridge::pareto_front(&initial))
        .transpose()
        .map_err(|error| problem_error("bridge", error))?;
    let pareto_a = if let Some(result) = &pareto {
        pareto_trace(result, AgentId::A)?
    } else {
        first_a.clone()
    };
    let pareto_b = if let Some(result) = &pareto {
        pareto_trace(result, AgentId::B)?
    } else {
        first_b.clone()
    };
    let pareto_expanded = pareto
        .as_ref()
        .map(|result| result.progress().expanded() as u64);
    let traces = [
        (
            "breadth_first",
            "Bridge · generic BFS",
            "Generic search finds a valid capacity-one crossing allocation.",
            bfs.trace().clone(),
            "breadth-first search",
            Some(bfs.expanded() as u64),
        ),
        (
            "first_come_a",
            "Bridge · Agent A first",
            "First-come policy allocates the first crossing right to Agent A.",
            first_a,
            "first-come mechanism",
            None,
        ),
        (
            "first_come_b",
            "Bridge · Agent B first",
            "First-come policy allocates the first crossing right to Agent B.",
            first_b,
            "first-come mechanism",
            None,
        ),
        (
            "auction",
            "Bridge · atomic auction",
            "Bid escrow, winner resolution, capacity rights, and refunds form replay-verified atomic exchanges.",
            auction,
            "auction mechanism",
            None,
        ),
        (
            "pareto_a",
            "Bridge Pareto · Agent A",
            "The exact allocation frontier member favoring Agent A.",
            pareto_a,
            "exact Pareto search",
            pareto_expanded,
        ),
        (
            "pareto_b",
            "Bridge Pareto · Agent B",
            "The exact allocation frontier member favoring Agent B.",
            pareto_b,
            "exact Pareto search",
            pareto_expanded,
        ),
    ];
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
                .map(|value| (TelemetryKindView::Expanded, value, "states expanded".into()))
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
        view.proposals.push(proposal("bridge", ProposalSpec { id: "impersonated-bid", label: "Agent B submits Agent A bid", description: "The role binding conflicts with the encoded agent identity and is explained as an asset shortfall." }, &initial, &wrong_identity));
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
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn candidate_front_view(documents: &[ViewDocument], selected: &ViewDocument) -> ParetoFrontView {
    let rows = documents
        .iter()
        .filter(|document| document.objectives.len() == 4)
        .map(|document| {
            let values = document
                .objectives
                .iter()
                .map(|objective| objective.value.parse::<u64>().unwrap_or(0))
                .collect::<Vec<_>>();
            (document, values)
        })
        .collect::<Vec<_>>();
    let non_dominated = rows.iter().filter(|(_, candidate)| {
        !rows.iter().any(|(_, other)| {
            other
                .iter()
                .zip(candidate)
                .all(|(left, right)| left >= right)
                && other
                    .iter()
                    .zip(candidate)
                    .any(|(left, right)| left > right)
        })
    });
    ParetoFrontView {
        title: "Evaluated two-round mechanism frontier".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: ["A priority", "A credit", "B priority", "B credit"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| ObjectiveAxisView {
                key: format!("axis_{index}"),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
            })
            .collect(),
        points: non_dominated
            .map(|(document, values)| ParetoPointView {
                label: document.title.clone(),
                selected: document.id == selected.id,
                values: values.into_iter().map(|value| value.to_string()).collect(),
            })
            .collect(),
    }
}

fn pareto_trace(
    result: &bridge::ParetoResult,
    favored: AgentId,
) -> Result<axionomy::Trace<RateId, Role, AccountId>, ServiceError> {
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
    [AgentId::A, AgentId::B]
        .into_iter()
        .flat_map(|agent| {
            [
                ObjectiveView {
                    key: format!("priority_{agent:?}").to_lowercase(),
                    label: format!("Agent {agent:?} priority"),
                    direction: ObjectiveDirectionView::Maximize,
                    value: bridge::priority(world, agent).to_string(),
                },
                ObjectiveView {
                    key: format!("credit_{agent:?}").to_lowercase(),
                    label: format!("Agent {agent:?} credit"),
                    direction: ObjectiveDirectionView::Maximize,
                    value: bridge::credit(world, agent).to_string(),
                },
            ]
        })
        .collect()
}

fn front_view(result: &bridge::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|o| o.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Priority and retained-credit frontier".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: ["A priority", "A credit", "B priority", "B credit"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| ObjectiveAxisView {
                key: format!("axis_{index}"),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
            })
            .collect(),
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|o| o.value().to_string())
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
    let nodes = [
        ("west", "West bank", 70.0),
        ("bridge", "Capacity-one bridge", 320.0),
        ("east", "East bank", 570.0),
    ]
    .into_iter()
    .map(|(key, label, x)| GraphNodeView {
        id: ViewId::new(key, label),
        classes: if key == "bridge" {
            vec!["resource".into()]
        } else {
            Vec::new()
        },
        x: Some(x),
        y: Some(130.0),
    })
    .collect();
    let edges = [AgentId::A, AgentId::B]
        .into_iter()
        .map(|agent| {
            let east = !world
                .balance(&AccountId::Agent(agent), &Asset::At(Side::East))
                .is_zero();
            GraphEdgeView {
                id: format!("agent:{agent:?}"),
                source: if east { "bridge".into() } else { "west".into() },
                target: if east { "east".into() } else { "bridge".into() },
                label: Some(format!("Agent {agent:?}")),
                classes: if east {
                    vec!["completed".into()]
                } else {
                    Vec::new()
                },
            }
        })
        .collect();
    Some(Scene::Graph {
        title: "Agents, scarce capacity, and crossing allocation".into(),
        nodes,
        edges,
        focus: None,
    })
}
