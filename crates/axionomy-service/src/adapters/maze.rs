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
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let initial = match study.instance {
        InstanceProfile::Micro => maze::initial(),
        InstanceProfile::Showcase => maze::initial_showcase(),
        InstanceProfile::Stress => maze::initial_stress(),
    };
    let selected = study.strategy.as_str();
    observe_selected_search(&initial, selected, progress)?;
    let bfs = maze::solve_bfs(&initial).ok_or_else(|| problem_error("maze", "no BFS route"))?;
    let dijkstra =
        maze::solve_dijkstra(&initial).ok_or_else(|| problem_error("maze", "no Dijkstra route"))?;
    let astar = maze::solve_astar(&initial).ok_or_else(|| problem_error("maze", "no A* route"))?;
    let pareto = maze::pareto_front(&initial).map_err(|error| problem_error("maze", error))?;
    if selected.starts_with("pareto-") {
        let state = pareto.progress();
        let _ = progress.emit(
            slug("pareto-frontier"),
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
            "breadth-first",
            "Maze · fewest moves",
            "Breadth-first search minimizes exchange count. In larger mazes, untreated injuries can make a short sequence take longer than a prepared route.",
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
            "a-star",
            "Maze · least energy",
            "A* uses the encoded distance estimate to find the efficient key-and-gate route with less search.",
            astar.trace().clone(),
            "A*",
            astar.expanded(),
        ),
        (
            "pareto-energy",
            "Maze Pareto · least energy",
            "The cheapest route on the frontier; every step was replayed to confirm it.",
            energy_trace,
            "exact Pareto search",
            pareto.progress().expanded(),
        ),
        (
            "pareto-time",
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
        document.thinking = if strategy.starts_with("pareto-") {
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
                ProposalSpec {
                    id: "locked-door",
                    label: "Cross locked door",
                    description: "Well-formed, but the Explorer is not at the gate and the gate is still locked.",
                },
                &initial,
                &locked,
            ),
            proposal(
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
    assemble(study, descriptor, documents)
}

#[derive(Clone, Copy)]
struct MazeExpansion {
    node: Node,
    has_key: bool,
    gate_open: bool,
    energy: u64,
    time: u64,
    injury: u64,
    protection: u64,
    vault_power: bool,
}

fn capture_expansion(world: &World) -> Option<MazeExpansion> {
    Some(MazeExpansion {
        node: maze::position(world)?,
        has_key: maze::has_key(world),
        gate_open: maze::gate_is_open(world),
        energy: world.balance(&AccountId::Agent, &Asset::Energy).to_u64(),
        time: world.balance(&AccountId::Agent, &Asset::Time).to_u64(),
        injury: world.balance(&AccountId::Agent, &Asset::Injury).to_u64(),
        protection: world.balance(&AccountId::Agent, &Asset::Shield).to_u64(),
        vault_power: !world
            .balance(&AccountId::World, &Asset::PowerVault)
            .is_zero(),
    })
}

fn zero_heuristic(_: &World) -> u64 {
    0
}

fn expansion_label(expansion: MazeExpansion) -> String {
    format!(
        "Expanded {} · {} · gate {} · E left {} · T left {} · injury {} · protection {} · vault power {}",
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
        expansion.injury,
        expansion.protection,
        expansion.vault_power,
    )
}

fn expansion_subject(expansion: MazeExpansion) -> ViewId {
    ViewId::new(
        slug(format!("node/{:?}", expansion.node)),
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
        phase: slug(phase),
        algorithm: slug(phase.trim_end_matches("-frontier")),
        kind: SearchObservationKindView::Frontier,
        label: expansion_label(expansion),
        completed: state.expanded() as u64,
        total: 0,
        subjects: vec![expansion_subject(expansion)],
        metrics: vec![
            visual_metric(slug("expanded"), "States expanded", state.expanded(), None),
            visual_metric(
                slug("generated"),
                "States generated",
                state.generated(),
                None,
            ),
            visual_metric(slug("frontier"), "Frontier", state.frontier(), None),
            visual_metric(slug("visited"), "States visited", state.visited(), None),
        ],
    }
}

fn solution_label(expansion: MazeExpansion) -> String {
    format!(
        "Solution accepted at {} · E left {} · T left {}",
        expansion.node.studio_label(),
        expansion.energy,
        expansion.time,
    )
}

fn saved_solution_observation(
    phase: &str,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
) -> SearchObservationView {
    SearchObservationView {
        sequence: state.expanded() as u64 + 1,
        phase: slug(phase),
        algorithm: slug(phase.trim_end_matches("-solution")),
        kind: SearchObservationKindView::Incumbent,
        label: solution_label(expansion),
        completed: state.expanded() as u64,
        total: state.expanded() as u64,
        subjects: vec![expansion_subject(expansion)],
        metrics: vec![
            visual_metric(slug("expanded"), "States expanded", state.expanded(), None),
            visual_metric(
                slug("generated"),
                "States generated",
                state.generated(),
                None,
            ),
            visual_metric(slug("visited"), "States visited", state.visited(), None),
            visual_metric(
                slug("solution-cost"),
                "Solution cost",
                state.solution_depth().unwrap_or_default(),
                None,
            ),
        ],
    }
}

fn solved_expansion(
    initial: &World,
    trace: &axionomy::Trace<maze::Ontology>,
) -> Result<MazeExpansion, ServiceError> {
    let solved = initial
        .replayed(trace)
        .map_err(|error| problem_error("maze", error))?;
    capture_expansion(&solved).ok_or_else(|| problem_error("maze", "solution lost its room"))
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
    if strategy == "breadth-first" {
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
                    "breadth-first-frontier",
                    session.progress(),
                    expansion,
                );
                retain_observation(&mut observations, observation);
            }
        }
        if let Some(solution) = session.solution() {
            let expansion = solved_expansion(initial, solution.trace())?;
            let observation =
                saved_solution_observation("breadth-first-solution", session.progress(), expansion);
            retain_observation(&mut observations, observation);
        }
    } else {
        let capture = std::rc::Rc::clone(&expanded);
        let (phase, heuristic): (&str, fn(&World) -> u64) = if strategy == "dijkstra" {
            ("dijkstra-frontier", zero_heuristic)
        } else {
            ("a-star-frontier", maze::heuristic)
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
        if let Some(solution) = session.solution() {
            let expansion = solved_expansion(initial, solution.trace())?;
            let observation = saved_solution_observation(
                if strategy == "dijkstra" {
                    "dijkstra-solution"
                } else {
                    "a-star-solution"
                },
                session.progress(),
                expansion,
            );
            retain_observation(&mut observations, observation);
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
        phase: slug("pareto-frontier"),
        algorithm: slug("exact-pareto-search"),
        kind: SearchObservationKindView::Frontier,
        label: format!(
            "Exact frontier exhausted · {} terminal routes · {} non-dominated tradeoffs",
            progress.terminal_outcomes(),
            progress.pareto_outcomes()
        ),
        completed: progress.expanded() as u64,
        total: progress.expanded() as u64,
        subjects: vec![ViewId::new(slug("node/exit"), "Exit")],
        metrics: vec![
            visual_metric(
                slug("expanded"),
                "States expanded",
                progress.expanded(),
                None,
            ),
            visual_metric(
                slug("generated"),
                "States generated",
                progress.generated(),
                None,
            ),
            visual_metric(
                slug("terminal"),
                "Terminal routes",
                progress.terminal_outcomes(),
                None,
            ),
            visual_metric(
                slug("pareto"),
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
        slug(phase),
        state,
        expansion_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.ensure()
}

fn emit_solution(
    phase: &str,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
    progress: &mut ProgressSink<'_>,
) -> Result<(), ServiceError> {
    let _ = progress.graph_with_subjects(
        slug(phase),
        state,
        solution_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.ensure()
}

fn observe_selected_search(
    initial: &World,
    strategy: &str,
    progress: &mut ProgressSink<'_>,
) -> Result<(), ServiceError> {
    if strategy.starts_with("pareto-") {
        return Ok(());
    }
    let expanded = std::rc::Rc::new(std::cell::RefCell::new(None));
    if strategy == "breadth-first" {
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
                    "breadth-first-frontier",
                    session.progress(),
                    expansion,
                    progress,
                )?;
            }
        }
        if let Some(solution) = session.solution() {
            emit_solution(
                "breadth-first-solution",
                session.progress(),
                solved_expansion(initial, solution.trace())?,
                progress,
            )?;
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
                        "dijkstra-frontier"
                    } else {
                        "a-star-frontier"
                    },
                    session.progress(),
                    expansion,
                    progress,
                )?;
            }
        }
        if let Some(solution) = session.solution() {
            emit_solution(
                if strategy == "dijkstra" {
                    "dijkstra-solution"
                } else {
                    "a-star-solution"
                },
                session.progress(),
                solved_expansion(initial, solution.trace())?,
                progress,
            )?;
        }
    }
    Ok(())
}

fn frontier_trace(
    result: &maze::ParetoResult,
    energy_first: bool,
) -> Result<axionomy::Trace<maze::Ontology>, ServiceError> {
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
            key: slug("energy"),
            label: "Energy spent".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: maze::spent_energy(world).to_string(),
        },
        ObjectiveView {
            key: slug("time"),
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
                key: slug("energy"),
                label: "Energy spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: slug("time"),
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
    let node_key = |node| slug(format!("node/{node:?}"));
    let graph_nodes = nodes
        .iter()
        .copied()
        .map(|node| {
            let (x, y) = positions(node);
            let mut classes = if node == Node::Gate {
                vec![slug("facility")]
            } else {
                vec![slug("location")]
            };
            if focus == Some(node) {
                classes.push(slug("current"));
            }
            if node == Node::Exit {
                classes.push(slug("goal"));
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
                id: slug(format!("edge/{from:?}/{to:?}")),
                source: node_key(*from),
                target: node_key(*to),
                label: Some(if *needs_open_door {
                    format!("{energy} E · gate")
                } else {
                    format!("{energy} E")
                }),
                classes: if (*needs_open_door
                    && (world.balance(&AccountId::World, &Asset::Open).is_zero()
                        || (!world.balance(&AccountId::Agent, &Asset::Health).is_zero()
                            && world
                                .balance(&AccountId::World, &Asset::PowerVault)
                                .is_zero())))
                    || ((*from, *to) == (Node::Canal, Node::Bridge)
                        && world
                            .balance(&AccountId::World, &Asset::PowerTransit)
                            .is_zero()
                        && !world.balance(&AccountId::Agent, &Asset::Health).is_zero())
                {
                    vec![slug("locked")]
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
        let mut entity = link_balance::<maze::Ontology>(
            visual_entity(
                slug("agent/explorer"),
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
            &AccountId::Agent,
            &Asset::At(node),
        );
        entity.metrics = vec![
            asset_metric(slug("energy"), "Energy", &Asset::Energy, energy),
            asset_metric(slug("time"), "Time", &Asset::Time, time),
        ];
        entity
    });
    let key = if !world.balance(&AccountId::World, &Asset::Key).is_zero() {
        Some(link_balance::<maze::Ontology>(
            visual_entity(
                slug("maze/key/brass"),
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::GraphNode {
                    node: node_key(Node::KeyRoom),
                },
                SceneEntityRoleView::Attachment,
                SceneToneView::Warning,
                Some("available".into()),
            ),
            &AccountId::World,
            &Asset::Key,
        ))
    } else if !world.balance(&AccountId::Agent, &Asset::Key).is_zero() {
        Some(link_balance::<maze::Ontology>(
            visual_entity(
                slug("maze/key/brass"),
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::Entity {
                    entity: slug("agent/explorer"),
                },
                SceneEntityRoleView::Attachment,
                SceneToneView::Active,
                Some("carried".into()),
            ),
            &AccountId::Agent,
            &Asset::Key,
        ))
    } else {
        None
    };
    let enhanced = !world.balance(&AccountId::Agent, &Asset::Health).is_zero();
    let vault_power = !world
        .balance(&AccountId::World, &Asset::PowerVault)
        .is_zero();
    let unlocked = !world.balance(&AccountId::World, &Asset::Open).is_zero();
    let gate_open = unlocked && (!enhanced || vault_power);
    let gate_status = if gate_open {
        "open"
    } else if unlocked {
        "unlocked · needs power"
    } else {
        "locked"
    };
    let gate = link_balance::<maze::Ontology>(
        visual_entity(
            slug("maze/gate/state"),
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
            Some(gate_status.into()),
        ),
        &AccountId::World,
        if gate_open {
            &Asset::Open
        } else {
            &Asset::Locked
        },
    );
    let mut scene = Scene::graph(
        "The Vault District — find the key, open the gate, reach the exit",
        graph_nodes,
        graph_edges,
        focus.map(node_key),
    )
    .with_entities(agent.into_iter().chain(key).chain([gate]))
    .with_metrics([
        asset_metric(
            slug("energy-left"),
            "Explorer energy",
            &Asset::Energy,
            world.balance(&AccountId::Agent, &Asset::Energy),
        ),
        asset_metric(
            slug("time-left"),
            "Time remaining",
            &Asset::Time,
            world.balance(&AccountId::Agent, &Asset::Time),
        ),
        visual_metric(
            slug("key-state"),
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
        visual_metric(slug("gate-state"), "Vault gate", gate_status, None),
    ]);
    if enhanced {
        scene.metrics.extend([
            visual_metric(
                slug("power"),
                "Shared power",
                if vault_power {
                    "vault shutter"
                } else {
                    "canal crossing"
                },
                None,
            ),
            asset_metric(
                slug("injury"),
                "Injury delay per action",
                &Asset::Injury,
                world.balance(&AccountId::Agent, &Asset::Injury),
            ),
            asset_metric(
                slug("shield"),
                "Protection remaining",
                &Asset::Shield,
                world.balance(&AccountId::Agent, &Asset::Shield),
            ),
            asset_metric(
                slug("health"),
                "Health",
                &Asset::Health,
                world.balance(&AccountId::Agent, &Asset::Health),
            ),
        ]);
        for (key, label, at, glyph, state) in [
            (
                "power-library",
                "Power switch",
                Node::Library,
                SceneGlyphView::Energy,
                if vault_power {
                    "vault powered"
                } else {
                    "canal powered"
                },
            ),
            (
                "power-workshop",
                "Power switch",
                Node::Workshop,
                SceneGlyphView::Energy,
                if vault_power {
                    "vault powered"
                } else {
                    "canal powered"
                },
            ),
            (
                "medical",
                "Aid station",
                Node::Garden,
                SceneGlyphView::Repair,
                "treat injuries · 2 ticks",
            ),
            (
                "protection",
                "Protective equipment",
                Node::Start,
                SceneGlyphView::Shield,
                if world.balance(&AccountId::World, &Asset::Shield).is_zero() {
                    "collected"
                } else {
                    "2 charges · 1 tick"
                },
            ),
        ] {
            scene.entities.push(visual_entity(
                slug(key),
                label,
                glyph,
                SceneAnchorView::GraphNode { node: node_key(at) },
                SceneEntityRoleView::Attachment,
                SceneToneView::Neutral,
                Some(state.into()),
            ));
        }
    }
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

fn route_id(rate: &RateId) -> Option<ItemPath> {
    match rate {
        RateId::Move { from, to, .. } | RateId::ProtectedMove { from, to, .. } => {
            Some(slug(format!("edge/{from:?}/{to:?}")))
        }
        _ => None,
    }
}

fn decorate_route_evidence(document: &mut ViewDocument, trace: &axionomy::Trace<maze::Ontology>) {
    let planned = trace
        .exchanges()
        .iter()
        .filter_map(|exchange| route_id(exchange.rate()))
        .collect::<std::collections::BTreeSet<_>>();
    let mut traversed = std::collections::BTreeSet::new();
    decorate_scene_paths(document.initial.scene.as_mut(), &planned, &traversed, None);
    for (frame, exchange) in document.frames.iter_mut().zip(trace.exchanges()) {
        let current = route_id(exchange.rate());
        decorate_scene_paths(
            frame.after.scene.as_mut(),
            &planned,
            &traversed,
            current.as_ref(),
        );
        if let Some(current) = current {
            traversed.insert(current);
        }
    }
}

fn decorate_scene_paths(
    scene: Option<&mut Scene>,
    planned: &std::collections::BTreeSet<ItemPath>,
    traversed: &std::collections::BTreeSet<ItemPath>,
    current: Option<&ItemPath>,
) {
    let Some(scene) = scene else { return };
    scene.legend.retain(|entry| {
        entry.label != "Planned next"
            && entry.label != "Traversed route"
            && entry.label != "Current move"
    });
    let remaining = planned
        .iter()
        .any(|path| !traversed.contains(path) && current != Some(path));
    if remaining {
        scene.legend.push(SceneLegendView {
            label: "Planned next".into(),
            glyph: SceneGlyphView::Move,
            tone: SceneToneView::Warning,
        });
    }
    if !traversed.is_empty() {
        scene.legend.push(SceneLegendView {
            label: "Traversed route".into(),
            glyph: SceneGlyphView::Move,
            tone: SceneToneView::Active,
        });
    }
    if current.is_some() {
        scene.legend.push(SceneLegendView {
            label: "Current move".into(),
            glyph: SceneGlyphView::Move,
            tone: SceneToneView::Success,
        });
    }
    if let SceneSurfaceView::Graph { edges, .. } = &mut scene.surface {
        for edge in edges {
            edge.classes.retain(|class| {
                class.as_str() != "current"
                    && class.as_str() != "traversed"
                    && class.as_str() != "candidate"
            });
            if current == Some(&edge.id) {
                edge.classes.push(slug("current"));
            } else if traversed.contains(&edge.id) {
                edge.classes.push(slug("traversed"));
            } else if planned.contains(&edge.id) {
                edge.classes.push(slug("candidate"));
            }
        }
    }
    for path in &mut scene.paths {
        path.status = if current == Some(&path.id) {
            ScenePathStatusView::Current
        } else if traversed.contains(&path.id) {
            ScenePathStatusView::Traversed
        } else if planned.contains(&path.id) {
            ScenePathStatusView::Candidate
        } else if matches!(path.status, ScenePathStatusView::Blocked) {
            ScenePathStatusView::Blocked
        } else {
            ScenePathStatusView::Available
        };
    }
}

// How Studio names and writes this problem's values.

impl StudioFormat for maze::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy | Self::Distance(_) => {
                QuantityFormat::COUNT.unit("units")
            }
            Self::Time | Self::SpentTime | Self::Injury => QuantityFormat::COUNT.unit("ticks"),
            Self::Shield | Self::UsedShield => QuantityFormat::COUNT.unit("charges"),
            _ => QuantityFormat::COUNT,
        }
    }
}

fn maze_node(node: maze::Node) -> String {
    words(&node)
}

impl StudioLabel for maze::Node {
    fn studio_label(&self) -> String {
        maze_node(*self)
    }
}

impl StudioLabel for maze::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent => "Explorer".into(),
            Self::World => "Vault District".into(),
        }
    }
}

impl StudioLabel for maze::Asset {
    fn studio_label(&self) -> String {
        match *self {
            Self::PowerTransit => "Power routed to the canal crossing".into(),
            Self::PowerVault => "Power routed to the vault shutter".into(),
            Self::Health => "Health reserve".into(),
            Self::Injury => "Injury delay per action".into(),
            Self::Shield => "Protective equipment".into(),
            Self::UsedShield => "Spent protection".into(),
            Self::Medicine => "Available medicine".into(),
            Self::UsedMedicine => "Medicine used".into(),
            Self::At(node) => format!("Standing in {}", maze_node(node)),
            Self::Edge(from, to) => {
                format!("Passage {} → {}", maze_node(from), maze_node(to))
            }
            Self::Key => "Key".into(),
            Self::Locked => "Vault gate locked".into(),
            Self::Open => "Vault gate open".into(),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Time => "Time left".into(),
            Self::SpentTime => "Time spent".into(),
            Self::Target(node) => format!("Goal: reach {}", maze_node(node)),
            Self::Distance(node) => format!("Distance from {} to the exit", maze_node(node)),
            Self::Active => "Expedition in progress".into(),
            Self::Solved => "Explorer escaped".into(),
        }
    }
}

impl StudioLabel for maze::RateId {
    fn studio_label(&self) -> String {
        match *self {
            Self::DivertPower { at, to_vault } => format!(
                "Divert power at {} to {}",
                maze_node(at),
                if to_vault { "vault" } else { "canal" }
            ),
            Self::TakeProtection { at } => {
                format!("Collect protective equipment at {}", maze_node(at))
            }
            Self::Treat { at } => format!("Treat injuries at {}", maze_node(at)),
            Self::ProtectedMove { from, to, energy } => format!(
                "Protected crossing {} → {} for {energy} energy",
                maze_node(from),
                maze_node(to)
            ),
            Self::Move {
                from,
                to,
                energy,
                needs_open_door,
            } => format!(
                "Walk {} → {} for {} energy{}",
                maze_node(from),
                maze_node(to),
                energy,
                if needs_open_door {
                    " once the vault gate is open"
                } else {
                    ""
                }
            ),
            Self::TakeKey { at } => format!("Pick up the key in {}", maze_node(at)),
            Self::UnlockDoor { at } => format!("Unlock the gate from {}", maze_node(at)),
            Self::Finish => "Reach the exit".into(),
        }
    }
}

debug_labels!(maze::Role);
