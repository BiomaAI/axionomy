use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::maze::{self, AccountId, Asset, Node, ObjectiveKey, RateId, Role, World};
use axionomy_search::{
    AStarSession, BfsSession,
    pareto::Objective,
    session::{Continue, WorkBudget},
};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, SceneAnchorView, SceneGlyphView,
    SceneLegendView, ScenePathStatusView, SceneSurfaceView, SceneToneView, TelemetryKindView,
    ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let initial = match instance_profile(request, descriptor) {
        InstanceProfile::Micro => maze::initial(),
        InstanceProfile::Showcase => maze::initial_showcase(),
        InstanceProfile::Stress => maze::initial_stress(),
    };
    let selected = selected_strategy(request, descriptor);
    observe_selected_search(&initial, selected, progress)?;
    let bfs = maze::solve_bfs(&initial).ok_or_else(|| problem_error("maze", "no BFS route"))?;
    let dijkstra =
        maze::solve_dijkstra(&initial).ok_or_else(|| problem_error("maze", "no Dijkstra route"))?;
    let astar = maze::solve_astar(&initial).ok_or_else(|| problem_error("maze", "no A* route"))?;
    let pareto = maze::pareto_front(&initial).map_err(|error| problem_error("maze", error))?;
    if selected.starts_with("pareto_") {
        let state = pareto.progress();
        let _ = progress.emit(
            "pareto_frontier",
            state.expanded() as u64,
            state.expanded() as u64,
            format!(
                "Exact frontier exhausted · {} states expanded · {} non-dominated routes",
                state.expanded(),
                pareto.front().len()
            ),
        );
        progress.ensure()?;
    }
    let energy_trace = frontier_trace(&pareto, true)?;
    let time_trace = frontier_trace(&pareto, false)?;

    let traces = [
        (
            "breadth_first",
            "Maze · fewest moves",
            "Breadth-first search takes the detour: fewer moves, but more energy.",
            bfs.trace().clone(),
            "breadth-first search",
            bfs.expanded(),
        ),
        (
            "dijkstra",
            "Maze · least energy without guidance",
            "Dijkstra proves the cheapest key-and-gate route while exploring without a heuristic.",
            dijkstra.trace().clone(),
            "Dijkstra",
            dijkstra.expanded(),
        ),
        (
            "a_star",
            "Maze · least energy",
            "A* uses the encoded distance estimate to find the efficient key-and-gate route with less search.",
            astar.trace().clone(),
            "A*",
            astar.expanded(),
        ),
        (
            "pareto_energy",
            "Maze Pareto · least energy",
            "The cheapest route on the frontier; every step was replayed to confirm it.",
            energy_trace,
            "exact Pareto search",
            pareto.progress().expanded(),
        ),
        (
            "pareto_time",
            "Maze Pareto · least time",
            "The fastest route on the frontier; every step was replayed to confirm it.",
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
        decorate_route_evidence(&mut document, &trace);
        document.pareto_fronts.push(front_view(&pareto, &document));
        document.telemetry.push(telemetry(
            algorithm,
            true,
            [
                (
                    TelemetryKindView::Expanded,
                    expanded as u64,
                    "states explored".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "accepted exchanges".into(),
                ),
            ],
        ));
        document.solve_observations = if strategy.starts_with("pareto_") {
            saved_pareto_observations(&pareto)
        } else {
            saved_graph_observations(&initial, strategy)?
        };
        let locked_rate = initial
            .rate_ids()
            .find(|rate| {
                matches!(
                    rate,
                    RateId::Move {
                        needs_open_door: true,
                        ..
                    }
                )
            })
            .copied()
            .ok_or_else(|| problem_error("maze", "missing locked passage"))?;
        let locked = Exchange::new(locked_rate, Quantity::new(1))
            .bind(Role::Actor, AccountId::Agent)
            .bind(Role::Environment, AccountId::World);
        let malformed = Exchange::new(RateId::TakeKey { at: Node::KeyRoom }, Quantity::new(1))
            .bind(Role::Actor, AccountId::Agent);
        document.proposals = vec![
            proposal(
                "maze",
                ProposalSpec {
                    id: "locked-door",
                    label: "Cross locked door",
                    description: "Well-formed, but the Explorer is not at the gate and the gate is still locked.",
                },
                &initial,
                &locked,
            ),
            proposal(
                "maze",
                ProposalSpec {
                    id: "missing-environment",
                    label: "Take key without World",
                    description: "Deliberately missing the World role, so there is nothing to take the key from.",
                },
                &initial,
                &malformed,
            ),
        ];
        documents.push(document);
    }
    artifact(request, descriptor, selected, documents)
}

#[derive(Clone, Copy)]
struct MazeExpansion {
    node: Node,
    has_key: bool,
    gate_open: bool,
    energy: u64,
    time: u64,
}

fn capture_expansion(world: &World) -> Option<MazeExpansion> {
    Some(MazeExpansion {
        node: maze::position(world)?,
        has_key: maze::has_key(world),
        gate_open: maze::gate_is_open(world),
        energy: world.balance(&AccountId::Agent, &Asset::Energy).get(),
        time: world.balance(&AccountId::Agent, &Asset::Time).get(),
    })
}

fn zero_heuristic(_: &World) -> u64 {
    0
}

fn expansion_label(expansion: MazeExpansion) -> String {
    format!(
        "Expanded {} · {} · gate {} · E {} · T {}",
        expansion.node.studio_label(),
        if expansion.has_key {
            "carrying key"
        } else {
            "no key"
        },
        if expansion.gate_open {
            "open"
        } else {
            "locked"
        },
        expansion.energy,
        expansion.time,
    )
}

fn expansion_subject(expansion: MazeExpansion) -> ViewId {
    ViewId::new(
        format!("node:{:?}", expansion.node).to_ascii_lowercase(),
        expansion.node.studio_label(),
    )
}

fn saved_graph_observation(
    phase: &str,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
) -> SearchObservationView {
    SearchObservationView {
        sequence: state.expanded() as u64,
        phase: phase.into(),
        algorithm: phase.trim_end_matches("_frontier").into(),
        kind: SearchObservationKindView::Frontier,
        label: expansion_label(expansion),
        completed: state.expanded() as u64,
        total: 0,
        subjects: vec![expansion_subject(expansion)],
        metrics: vec![
            visual_metric("expanded", "States expanded", state.expanded(), None),
            visual_metric("generated", "States generated", state.generated(), None),
            visual_metric("frontier", "Frontier", state.frontier(), None),
            visual_metric("visited", "States visited", state.visited(), None),
        ],
    }
}

fn retain_observation(
    observations: &mut Vec<SearchObservationView>,
    observation: SearchObservationView,
) {
    if observations.len() == 256 {
        observations.remove(0);
    }
    observations.push(observation);
}

fn saved_graph_observations(
    initial: &World,
    strategy: &str,
) -> Result<Vec<SearchObservationView>, ServiceError> {
    let expanded = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut observations = Vec::new();
    if strategy == "breadth_first" {
        let capture = std::rc::Rc::clone(&expanded);
        let mut session = BfsSession::new(initial, maze::goal(), move |world| {
            *capture.borrow_mut() = capture_expansion(world);
            maze::candidates(world)
        });
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                let observation = saved_graph_observation(
                    "breadth_first_frontier",
                    session.progress(),
                    expansion,
                );
                retain_observation(&mut observations, observation);
            }
        }
    } else {
        let capture = std::rc::Rc::clone(&expanded);
        let (phase, heuristic): (&str, fn(&World) -> u64) = if strategy == "dijkstra" {
            ("dijkstra_frontier", zero_heuristic)
        } else {
            ("a_star_frontier", maze::heuristic)
        };
        let mut session = AStarSession::new(
            initial,
            maze::goal(),
            move |world| {
                *capture.borrow_mut() = capture_expansion(world);
                maze::candidates(world)
            },
            maze::move_energy,
            heuristic,
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                let observation = saved_graph_observation(phase, session.progress(), expansion);
                retain_observation(&mut observations, observation);
            }
        }
    }
    if observations.is_empty() {
        return Err(problem_error(
            "maze",
            "search produced no observable expansions",
        ));
    }
    Ok(observations)
}

fn saved_pareto_observations(result: &maze::ParetoResult) -> Vec<SearchObservationView> {
    let progress = result.progress();
    vec![SearchObservationView {
        sequence: 0,
        phase: "pareto_frontier".into(),
        algorithm: "exact_pareto_search".into(),
        kind: SearchObservationKindView::Frontier,
        label: format!(
            "Exact frontier exhausted · {} terminal routes · {} non-dominated tradeoffs",
            progress.terminal_outcomes(),
            progress.pareto_outcomes()
        ),
        completed: progress.expanded() as u64,
        total: progress.expanded() as u64,
        subjects: vec![ViewId::new("node:exit", "Exit")],
        metrics: vec![
            visual_metric("expanded", "States expanded", progress.expanded(), None),
            visual_metric("generated", "States generated", progress.generated(), None),
            visual_metric(
                "terminal",
                "Terminal routes",
                progress.terminal_outcomes(),
                None,
            ),
            visual_metric(
                "pareto",
                "Pareto outcomes",
                progress.pareto_outcomes(),
                None,
            ),
        ],
    }]
}

fn emit_expansion(
    phase: &str,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
    progress: &mut ProgressSink<'_>,
) -> Result<(), ServiceError> {
    let _ = progress.graph_with_subjects(
        phase,
        state,
        expansion_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.ensure()
}

fn observe_selected_search(
    initial: &World,
    strategy: &str,
    progress: &mut ProgressSink<'_>,
) -> Result<(), ServiceError> {
    if strategy.starts_with("pareto_") {
        return Ok(());
    }
    let expanded = std::rc::Rc::new(std::cell::RefCell::new(None));
    if strategy == "breadth_first" {
        let capture = std::rc::Rc::clone(&expanded);
        let mut session = BfsSession::new(initial, maze::goal(), move |world| {
            *capture.borrow_mut() = capture_expansion(world);
            maze::candidates(world)
        });
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                emit_expansion(
                    "breadth_first_frontier",
                    session.progress(),
                    expansion,
                    progress,
                )?;
            }
        }
    } else {
        let capture = std::rc::Rc::clone(&expanded);
        let heuristic = if strategy == "dijkstra" {
            zero_heuristic
        } else {
            maze::heuristic
        };
        let mut session = AStarSession::new(
            initial,
            maze::goal(),
            move |world| {
                *capture.borrow_mut() = capture_expansion(world);
                maze::candidates(world)
            },
            maze::move_energy,
            heuristic,
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                emit_expansion(
                    if strategy == "dijkstra" {
                        "dijkstra_frontier"
                    } else {
                        "a_star_frontier"
                    },
                    session.progress(),
                    expansion,
                    progress,
                )?;
            }
        }
    }
    Ok(())
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
        title: "Energy vs. time — every non-dominated route".into(),
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
    let stress = nodes.contains(&Node::Archive);
    let focus = nodes.iter().copied().find(|node| {
        !world
            .balance(&AccountId::Agent, &Asset::At(*node))
            .is_zero()
    });
    let positions = |node| match (stress, node) {
        (_, Node::Start) => (40.0, 300.0),
        (_, Node::Atrium) => (220.0, 260.0),
        (_, Node::Library) => (400.0, 90.0),
        (_, Node::KeyRoom) => (590.0, 40.0),
        (_, Node::Gallery) => (500.0, 260.0),
        (_, Node::Gate) => (690.0, 260.0),
        (_, Node::Vault) => (870.0, 260.0),
        (_, Node::Exit) => (1060.0, 300.0),
        (_, Node::Garden) => (220.0, 460.0),
        (_, Node::Market) => (410.0, 460.0),
        (_, Node::Canal) => (610.0, 460.0),
        (_, Node::Bridge) => (820.0, 460.0),
        (_, Node::Workshop) => (410.0, 650.0),
        (_, Node::Tunnel) => (110.0, 650.0),
        (_, Node::Detour) => (720.0, 650.0),
        (true, Node::Archive) => (220.0, 50.0),
        (true, Node::Scriptorium) => (400.0, -90.0),
        (true, Node::Docks) => (300.0, 840.0),
        (true, Node::Foundry) => (500.0, 840.0),
        (true, Node::Tower) => (700.0, 840.0),
        (true, Node::Observatory) => (980.0, 650.0),
        (true, Node::Ridge) => (80.0, 850.0),
        (true, Node::Ruins) => (180.0, 1030.0),
        (true, Node::Chapel) => (600.0, 1030.0),
        // Nodes outside a profile are never rendered, but retaining a total
        // match keeps the projection robust if another profile is introduced.
        (false, Node::Archive) => (220.0, 50.0),
        (false, Node::Scriptorium) => (400.0, -90.0),
        (false, Node::Docks) => (300.0, 840.0),
        (false, Node::Foundry) => (500.0, 840.0),
        (false, Node::Tower) => (700.0, 840.0),
        (false, Node::Observatory) => (980.0, 650.0),
        (false, Node::Ridge) => (80.0, 850.0),
        (false, Node::Ruins) => (180.0, 1030.0),
        (false, Node::Chapel) => (600.0, 1030.0),
    };
    let node_key = |node| format!("node:{node:?}").to_lowercase();
    let graph_nodes = nodes
        .iter()
        .copied()
        .map(|node| {
            let (x, y) = positions(node);
            let mut classes = if node == Node::Gate {
                vec!["facility".into()]
            } else {
                vec!["location".into()]
            };
            if focus == Some(node) {
                classes.push("current".into());
            }
            if node == Node::Exit {
                classes.push("goal".into());
            }
            GraphNodeView {
                id: ViewId::new(node_key(node), node.studio_label()),
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
                    format!("{energy} energy · gate must be open")
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
    let agent = focus.map(|node| {
        let energy = world.balance(&AccountId::Agent, &Asset::Energy);
        let time = world.balance(&AccountId::Agent, &Asset::Time);
        let mut entity = link_balance(
            visual_entity(
                "agent:explorer",
                "Explorer",
                SceneGlyphView::Agent,
                SceneAnchorView::GraphNode {
                    node: node_key(node),
                },
                SceneEntityRoleView::Occupant,
                if node == Node::Exit {
                    SceneToneView::Success
                } else {
                    SceneToneView::Active
                },
                Some(if node == Node::Exit {
                    format!("escaped · E {} · T {}", energy, time)
                } else {
                    format!("E {} · T {}", energy, time)
                }),
            ),
            "maze:account:agent",
            format!("maze:asset:at-{node:?}").to_ascii_lowercase(),
        );
        entity.metrics = vec![
            visual_metric("energy", "Energy", energy, Some("units")),
            visual_metric("time", "Time", time, Some("ticks")),
        ];
        entity
    });
    let key = if !world.balance(&AccountId::World, &Asset::Key).is_zero() {
        Some(link_balance(
            visual_entity(
                "maze:key:brass",
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::GraphNode {
                    node: node_key(Node::KeyRoom),
                },
                SceneEntityRoleView::Attachment,
                SceneToneView::Warning,
                Some("available".into()),
            ),
            "maze:account:world",
            "maze:asset:key",
        ))
    } else if !world.balance(&AccountId::Agent, &Asset::Key).is_zero() {
        Some(link_balance(
            visual_entity(
                "maze:key:brass",
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::Entity {
                    entity: "agent:explorer".into(),
                },
                SceneEntityRoleView::Attachment,
                SceneToneView::Active,
                Some("carried".into()),
            ),
            "maze:account:agent",
            "maze:asset:key",
        ))
    } else {
        None
    };
    let gate_open = !world.balance(&AccountId::World, &Asset::Open).is_zero();
    let gate = link_balance(
        visual_entity(
            "maze:gate:state",
            "Vault gate",
            SceneGlyphView::Door,
            SceneAnchorView::GraphNode {
                node: node_key(Node::Gate),
            },
            SceneEntityRoleView::State,
            if gate_open {
                SceneToneView::Success
            } else {
                SceneToneView::Danger
            },
            Some(if gate_open { "open" } else { "locked" }.into()),
        ),
        "maze:account:world",
        if gate_open {
            "maze:asset:open"
        } else {
            "maze:asset:locked"
        },
    );
    let mut scene = Scene::graph(
        "The Vault District — routes, key, and gate",
        graph_nodes,
        graph_edges,
        focus.map(node_key),
    )
    .with_entities(agent.into_iter().chain(key).chain([gate]))
    .with_metrics([
        visual_metric(
            "energy_left",
            "Explorer energy",
            world.balance(&AccountId::Agent, &Asset::Energy),
            Some("units"),
        ),
        visual_metric(
            "time_left",
            "Time remaining",
            world.balance(&AccountId::Agent, &Asset::Time),
            Some("ticks"),
        ),
        visual_metric(
            "key_state",
            "Key",
            if world.balance(&AccountId::World, &Asset::Key).is_zero() {
                if world.balance(&AccountId::Agent, &Asset::Key).is_zero() {
                    "used"
                } else {
                    "carried"
                }
            } else {
                "waiting"
            },
            None,
        ),
        visual_metric(
            "gate_state",
            "Vault gate",
            if gate_open { "open" } else { "locked" },
            None,
        ),
    ]);
    scene.legend = vec![
        SceneLegendView {
            label: "Room".into(),
            glyph: SceneGlyphView::Location,
            tone: SceneToneView::Neutral,
        },
        SceneLegendView {
            label: "Explorer".into(),
            glyph: SceneGlyphView::Agent,
            tone: SceneToneView::Active,
        },
        SceneLegendView {
            label: "Key".into(),
            glyph: SceneGlyphView::Key,
            tone: SceneToneView::Warning,
        },
        SceneLegendView {
            label: if gate_open {
                "Open gate"
            } else {
                "Locked gate"
            }
            .into(),
            glyph: SceneGlyphView::Door,
            tone: if gate_open {
                SceneToneView::Success
            } else {
                SceneToneView::Danger
            },
        },
        SceneLegendView {
            label: "Exit".into(),
            glyph: SceneGlyphView::Goal,
            tone: SceneToneView::Goal,
        },
    ];
    Some(scene)
}

fn route_id(rate: &RateId) -> Option<String> {
    match rate {
        RateId::Move { from, to, .. } => Some(format!("edge:{from:?}:{to:?}")),
        _ => None,
    }
}

fn decorate_route_evidence(
    document: &mut ViewDocument,
    trace: &axionomy::Trace<RateId, Role, AccountId>,
) {
    let mut traversed = std::collections::BTreeSet::new();
    decorate_scene_paths(document.initial.scene.as_mut(), &traversed, None);
    for (frame, exchange) in document.frames.iter_mut().zip(trace.exchanges()) {
        decorate_scene_paths(frame.before.scene.as_mut(), &traversed, None);
        let current = route_id(exchange.rate());
        decorate_scene_paths(frame.after.scene.as_mut(), &traversed, current.as_deref());
        if let Some(current) = current {
            traversed.insert(current);
        }
    }
}

fn decorate_scene_paths(
    scene: Option<&mut Scene>,
    traversed: &std::collections::BTreeSet<String>,
    current: Option<&str>,
) {
    let Some(scene) = scene else { return };
    if let SceneSurfaceView::Graph { edges, .. } = &mut scene.surface {
        for edge in edges {
            edge.classes
                .retain(|class| class != "current" && class != "traversed");
            if current == Some(edge.id.as_str()) {
                edge.classes.push("current".into());
            } else if traversed.contains(&edge.id) {
                edge.classes.push("traversed".into());
            }
        }
    }
    for path in &mut scene.paths {
        path.status = if current == Some(path.id.as_str()) {
            ScenePathStatusView::Current
        } else if traversed.contains(&path.id) {
            ScenePathStatusView::Traversed
        } else if matches!(path.status, ScenePathStatusView::Blocked) {
            ScenePathStatusView::Blocked
        } else {
            ScenePathStatusView::Available
        };
    }
}
