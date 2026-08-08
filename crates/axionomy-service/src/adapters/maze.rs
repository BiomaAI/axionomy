use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::maze::{self, AccountId, Asset, Node, ObjectiveKey, RateId, Role, World};
use axionomy_search::pareto::Objective;
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let initial = match instance_profile(request, descriptor) {
        InstanceProfile::Micro => maze::initial(),
        InstanceProfile::Showcase => maze::initial_showcase(),
        InstanceProfile::Stress => maze::initial_stress(),
    };
    let bfs = maze::solve_bfs(&initial).ok_or_else(|| problem_error("maze", "no BFS route"))?;
    let astar = maze::solve_astar(&initial).ok_or_else(|| problem_error("maze", "no A* route"))?;
    let pareto = maze::pareto_front(&initial).map_err(|error| problem_error("maze", error))?;
    let energy_trace = frontier_trace(&pareto, true)?;
    let time_trace = frontier_trace(&pareto, false)?;

    let traces = [
        (
            "breadth_first",
            "Maze · fewest exchanges",
            "Breadth-first search selects the shallow detour.",
            bfs.trace().clone(),
            "breadth-first search",
            bfs.expanded(),
        ),
        (
            "a_star",
            "Maze · least energy",
            "A* follows the longer key-and-door route because encoded energy is lower.",
            astar.trace().clone(),
            "A*",
            astar.expanded(),
        ),
        (
            "pareto_energy",
            "Maze Pareto · least energy",
            "The energy-first member of the exact replay-verified frontier.",
            energy_trace,
            "exact Pareto search",
            pareto.progress().expanded(),
        ),
        (
            "pareto_time",
            "Maze Pareto · least time",
            "The time-first member of the exact replay-verified frontier.",
            time_trace,
            "exact Pareto search",
            pareto.progress().expanded(),
        ),
    ];
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("maze", error))?;
        let mut document = document(
            DocumentSpec {
                problem: "maze",
                strategy,
                title,
                description,
                source_label: "Key-door maze",
            },
            &initial,
            &maze::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("maze", error))?;
        document.pareto_fronts.push(front_view(&pareto, &document));
        document.telemetry.push(telemetry(
            algorithm,
            true,
            [
                (
                    TelemetryKindView::Expanded,
                    expanded as u64,
                    "states expanded".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "accepted exchanges".into(),
                ),
            ],
        ));
        let locked = Exchange::new(
            RateId::Move {
                from: Node::KeyRoom,
                to: Node::Door,
                energy: 2,
                needs_open_door: true,
            },
            Quantity::new(1),
        )
        .bind(Role::Actor, AccountId::Agent)
        .bind(Role::Environment, AccountId::World);
        let malformed =
            Exchange::new(RateId::TakeKey, Quantity::new(1)).bind(Role::Actor, AccountId::Agent);
        document.proposals = vec![
            proposal(
                "maze",
                ProposalSpec {
                    id: "locked-door",
                    label: "Cross locked door",
                    description: "Structurally valid, but the initial state lacks both the key-room position and open-door fact.",
                },
                &initial,
                &locked,
            ),
            proposal(
                "maze",
                ProposalSpec {
                    id: "missing-environment",
                    label: "Take key without World",
                    description: "Malformed because the Environment role is intentionally unbound.",
                },
                &initial,
                &malformed,
            ),
        ];
        documents.push(document);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn frontier_trace(
    result: &maze::ParetoResult,
    energy_first: bool,
) -> Result<axionomy::Trace<RateId, Role, AccountId>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .min_by_key(|entry| {
            let energy = objective_value(entry.objectives().objectives(), ObjectiveKey::Energy);
            let time = objective_value(entry.objectives().objectives(), ObjectiveKey::Time);
            if energy_first {
                (energy, time)
            } else {
                (time, energy)
            }
        })
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error("maze", "empty Pareto frontier"))
}

fn objective_value(values: &[Objective<ObjectiveKey, u64>], key: ObjectiveKey) -> u64 {
    values
        .iter()
        .find(|objective| objective.key() == &key)
        .map_or(0, |objective| *objective.value())
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView {
            key: "energy".into(),
            label: "Energy spent".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: maze::spent_energy(world).to_string(),
        },
        ObjectiveView {
            key: "time".into(),
            label: "Time spent".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: maze::spent_time(world).to_string(),
        },
    ]
}

fn front_view(result: &maze::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|objective| objective.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Replay-verified energy/time frontier".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: "energy".into(),
                label: "Energy spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "time".into(),
                label: "Time spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|objective| objective.value().to_string())
                    .collect::<Vec<_>>();
                ParetoPointView {
                    label: format!("{} energy · {} time", values[0], values[1]),
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
    let nodes = maze::nodes(world);
    let focus = nodes.iter().copied().find(|node| {
        !world
            .balance(&AccountId::Agent, &Asset::At(*node))
            .is_zero()
    });
    let positions = |node| match node {
        Node::Start => (70.0, 155.0),
        Node::Atrium => (150.0, 20.0),
        Node::Gallery => (240.0, 20.0),
        Node::Archive => (330.0, 20.0),
        Node::Scriptorium => (420.0, 20.0),
        Node::KeyRoom => (510.0, 20.0),
        Node::Door => (600.0, 20.0),
        Node::Vault => (690.0, 20.0),
        Node::Garden => (150.0, 100.0),
        Node::Market => (240.0, 100.0),
        Node::Canal => (330.0, 100.0),
        Node::Docks => (420.0, 100.0),
        Node::Foundry => (510.0, 100.0),
        Node::Tower => (600.0, 100.0),
        Node::Observatory => (690.0, 100.0),
        Node::Tunnel => (170.0, 205.0),
        Node::Ridge => (280.0, 205.0),
        Node::Ruins => (390.0, 205.0),
        Node::Bridge => (500.0, 205.0),
        Node::Chapel => (610.0, 205.0),
        Node::Detour => (350.0, 285.0),
        Node::Exit => (800.0, 155.0),
    };
    let node_key = |node| format!("node:{node:?}").to_lowercase();
    let graph_nodes = nodes
        .iter()
        .copied()
        .map(|node| {
            let (x, y) = positions(node);
            let mut classes = Vec::new();
            if focus == Some(node) {
                classes.push("current".into());
            }
            if node == Node::Exit {
                classes.push("goal".into());
            }
            GraphNodeView {
                id: ViewId::new(node_key(node), format!("{node:?}")),
                classes,
                x: Some(x),
                y: Some(y),
            }
        })
        .collect();
    let graph_edges = world
        .rate_ids()
        .filter_map(|rate| match rate {
            RateId::Move {
                from,
                to,
                energy,
                needs_open_door,
            } => Some(GraphEdgeView {
                id: format!("edge:{from:?}:{to:?}"),
                source: node_key(*from),
                target: node_key(*to),
                label: Some(if *needs_open_door {
                    format!("{energy} energy · door")
                } else {
                    format!("{energy} energy")
                }),
                classes: if *needs_open_door
                    && world.balance(&AccountId::World, &Asset::Open).is_zero()
                {
                    vec!["locked".into()]
                } else {
                    Vec::new()
                },
            }),
            _ => None,
        })
        .collect();
    Some(Scene::graph(
        "Encoded maze topology",
        graph_nodes,
        graph_edges,
        focus.map(node_key),
    ))
}
