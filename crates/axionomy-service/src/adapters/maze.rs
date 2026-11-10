use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::maze::{self, AccountId, Asset, Node, ObjectiveKey, RateId, Role, World};
use axionomy_search::{
    AStarSession, BfsSession,
    pareto::Objective,
    session::{Continue, WorkBudget},
};
use axionomy_view::{
    ClassPath, FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, SceneAnchorView, SceneGlyphView,
    SceneLegendView, ScenePathStatusView, SceneSurfaceView, SceneToneView, TelemetryKindView,
    ViewId,
};

/// The maze's strategies, as the catalog lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
enum Strategy {
    BreadthFirst,
    Dijkstra,
    AStar,
    ParetoEnergy,
    ParetoTime,
}

impl Strategy {
    /// The graph search that finds this strategy's route; none for a point
    /// on the exact Pareto frontier.
    fn search(self) -> Option<Search> {
        match self {
            Self::BreadthFirst => Some(Search::BreadthFirst),
            Self::Dijkstra => Some(Search::Dijkstra),
            Self::AStar => Some(Search::AStar),
            Self::ParetoEnergy | Self::ParetoTime => None,
        }
    }
}

/// A graph search over the maze's rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Search {
    BreadthFirst,
    Dijkstra,
    AStar,
}

impl Search {
    fn algorithm(self) -> AlgorithmPath {
        match self {
            Self::BreadthFirst => path!("breadth-first"),
            Self::Dijkstra => path!("dijkstra"),
            Self::AStar => path!("a-star"),
        }
    }

    /// The search expanding its frontier.
    fn frontier(self) -> Work {
        let phase = match self {
            Self::BreadthFirst => path!("breadth-first-frontier"),
            Self::Dijkstra => path!("dijkstra-frontier"),
            Self::AStar => path!("a-star-frontier"),
        };
        Work::new(phase, self.algorithm(), SearchObservationKindView::Frontier)
    }

    /// The search accepting its solution.
    fn solution(self) -> Work {
        let phase = match self {
            Self::BreadthFirst => path!("breadth-first-solution"),
            Self::Dijkstra => path!("dijkstra-solution"),
            Self::AStar => path!("a-star-solution"),
        };
        Work::new(
            phase,
            self.algorithm(),
            SearchObservationKindView::Incumbent,
        )
    }

    /// The estimate of a best-first search: none for Dijkstra, the encoded
    /// distance for A*.
    fn heuristic(self) -> fn(&World) -> u64 {
        match self {
            Self::AStar => maze::heuristic,
            Self::BreadthFirst | Self::Dijkstra => zero_heuristic,
        }
    }
}

/// The exact Pareto search exhausting its frontier.
fn pareto_frontier() -> Work {
    Work::new(
        path!("pareto-frontier"),
        path!("exact-pareto-search"),
        SearchObservationKindView::Frontier,
    )
}

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let problem = &study.problem;
    let initial = match study.instance {
        InstanceProfile::Micro => maze::initial(),
        InstanceProfile::Showcase => maze::initial_showcase(),
        InstanceProfile::Stress => maze::initial_stress(),
    };
    let selected: Strategy = strategy(study)?;
    if let Some(search) = selected.search() {
        observe_selected_search(&initial, search, progress, problem)?;
    }
    let bfs = maze::solve_bfs(&initial).ok_or_else(|| problem_error(problem, "no BFS route"))?;
    let dijkstra = maze::solve_dijkstra(&initial)
        .ok_or_else(|| problem_error(problem, "no Dijkstra route"))?;
    let astar = maze::solve_astar(&initial).ok_or_else(|| problem_error(problem, "no A* route"))?;
    let pareto = maze::pareto_front(&initial).map_err(|error| problem_error(problem, error))?;
    if selected.search().is_none() {
        let state = pareto.progress();
        let _ = progress.emit(
            &pareto_frontier(),
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
    let energy_trace = frontier_trace(&pareto, true, problem)?;
    let time_trace = frontier_trace(&pareto, false, problem)?;

    let traces = [
        (
            Strategy::BreadthFirst,
            "Maze · fewest moves",
            "Breadth-first search minimizes exchange count. In larger mazes, untreated injuries can make a short sequence take longer than a prepared route.",
            bfs.trace().clone(),
            "breadth-first search",
            bfs.expanded(),
        ),
        (
            Strategy::Dijkstra,
            "Maze · least energy without guidance",
            "Dijkstra proves the cheapest key-and-gate route while exploring without a heuristic.",
            dijkstra.trace().clone(),
            "Dijkstra",
            dijkstra.expanded(),
        ),
        (
            Strategy::AStar,
            "Maze · least energy",
            "A* uses the encoded distance estimate to find the efficient key-and-gate route with less search.",
            astar.trace().clone(),
            "A*",
            astar.expanded(),
        ),
        (
            Strategy::ParetoEnergy,
            "Maze Pareto · least energy",
            "The cheapest route on the frontier; every step was replayed to confirm it.",
            energy_trace,
            "exact Pareto search",
            pareto.progress().expanded(),
        ),
        (
            Strategy::ParetoTime,
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
            .map_err(|error| problem_error(problem, error))?;
        let mut document = document(
            progress,
            DocumentSpec {
                problem,
                document: path::write(&strategy),
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
        .map_err(|error| problem_error(problem, error))?;
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
        document.thinking = match strategy.search() {
            Some(search) => saved_graph_observations(&initial, search, problem)?,
            None => saved_pareto_observations(&pareto),
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
            .ok_or_else(|| problem_error(problem, "missing locked passage"))?;
        let locked = Exchange::new(locked_rate, Quantity::new(1))
            .bind(Role::Actor, AccountId::Agent)
            .bind(Role::Environment, AccountId::World);
        let malformed = Exchange::new(RateId::TakeKey { at: Node::KeyRoom }, Quantity::new(1))
            .bind(Role::Actor, AccountId::Agent);
        document.proposals = vec![
            proposal(
                ProposalSpec {
                    id: path!("locked-door"),
                    label: "Cross locked door",
                    description: "Well-formed, but the Explorer is not at the gate and the gate is still locked.",
                },
                &initial,
                &locked,
            ),
            proposal(
                ProposalSpec {
                    id: path!("missing-environment"),
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
        path!("node").then(&expansion.node),
        expansion.node.studio_label(),
    )
}

fn saved_graph_observation(
    work: &Work,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
) -> SearchObservationView {
    SearchObservationView {
        sequence: state.expanded() as u64,
        phase: work.phase.clone(),
        algorithm: work.algorithm.clone(),
        kind: work.kind,
        label: expansion_label(expansion),
        completed: state.expanded() as u64,
        total: 0,
        subjects: vec![expansion_subject(expansion)],
        metrics: vec![
            visual_metric(path!("expanded"), "States expanded", state.expanded(), None),
            visual_metric(
                path!("generated"),
                "States generated",
                state.generated(),
                None,
            ),
            visual_metric(path!("frontier"), "Frontier", state.frontier(), None),
            visual_metric(path!("visited"), "States visited", state.visited(), None),
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
    work: &Work,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
) -> SearchObservationView {
    SearchObservationView {
        sequence: state.expanded() as u64 + 1,
        phase: work.phase.clone(),
        algorithm: work.algorithm.clone(),
        kind: work.kind,
        label: solution_label(expansion),
        completed: state.expanded() as u64,
        total: state.expanded() as u64,
        subjects: vec![expansion_subject(expansion)],
        metrics: vec![
            visual_metric(path!("expanded"), "States expanded", state.expanded(), None),
            visual_metric(
                path!("generated"),
                "States generated",
                state.generated(),
                None,
            ),
            visual_metric(path!("visited"), "States visited", state.visited(), None),
            visual_metric(
                path!("solution-cost"),
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
    problem: &ProblemPath,
) -> Result<MazeExpansion, ServiceError> {
    let solved = initial
        .replayed(trace)
        .map_err(|error| problem_error(problem, error))?;
    capture_expansion(&solved).ok_or_else(|| problem_error(problem, "solution lost its room"))
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
    search: Search,
    problem: &ProblemPath,
) -> Result<Vec<SearchObservationView>, ServiceError> {
    let expanded = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut observations = Vec::new();
    let frontier = search.frontier();
    if search == Search::BreadthFirst {
        let capture = std::rc::Rc::clone(&expanded);
        let mut session = BfsSession::new(initial, maze::goal(), move |world| {
            *capture.borrow_mut() = capture_expansion(world);
            maze::candidates(world)
        });
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                let observation = saved_graph_observation(&frontier, session.progress(), expansion);
                retain_observation(&mut observations, observation);
            }
        }
        if let Some(solution) = session.solution() {
            let expansion = solved_expansion(initial, solution.trace(), problem)?;
            let observation =
                saved_solution_observation(&search.solution(), session.progress(), expansion);
            retain_observation(&mut observations, observation);
        }
    } else {
        let capture = std::rc::Rc::clone(&expanded);
        let mut session = AStarSession::new(
            initial,
            maze::goal(),
            move |world| {
                *capture.borrow_mut() = capture_expansion(world);
                maze::candidates(world)
            },
            maze::move_energy,
            search.heuristic(),
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                let observation = saved_graph_observation(&frontier, session.progress(), expansion);
                retain_observation(&mut observations, observation);
            }
        }
        if let Some(solution) = session.solution() {
            let expansion = solved_expansion(initial, solution.trace(), problem)?;
            let observation =
                saved_solution_observation(&search.solution(), session.progress(), expansion);
            retain_observation(&mut observations, observation);
        }
    }
    if observations.is_empty() {
        return Err(problem_error(
            problem,
            "search produced no observable expansions",
        ));
    }
    Ok(observations)
}

fn saved_pareto_observations(result: &maze::ParetoResult) -> Vec<SearchObservationView> {
    let progress = result.progress();
    let work = pareto_frontier();
    vec![SearchObservationView {
        sequence: 0,
        phase: work.phase,
        algorithm: work.algorithm,
        kind: work.kind,
        label: format!(
            "Exact frontier exhausted · {} terminal routes · {} non-dominated tradeoffs",
            progress.terminal_outcomes(),
            progress.pareto_outcomes()
        ),
        completed: progress.expanded() as u64,
        total: progress.expanded() as u64,
        subjects: vec![ViewId::new(path!("node").then(&Node::Exit), "Exit")],
        metrics: vec![
            visual_metric(
                path!("expanded"),
                "States expanded",
                progress.expanded(),
                None,
            ),
            visual_metric(
                path!("generated"),
                "States generated",
                progress.generated(),
                None,
            ),
            visual_metric(
                path!("terminal"),
                "Terminal routes",
                progress.terminal_outcomes(),
                None,
            ),
            visual_metric(
                path!("pareto"),
                "Pareto outcomes",
                progress.pareto_outcomes(),
                None,
            ),
        ],
    }]
}

fn emit_expansion(
    work: &Work,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
    progress: &mut ProgressSink<'_>,
) -> Result<(), ServiceError> {
    let _ = progress.graph_with_subjects(
        work,
        state,
        expansion_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.ensure()
}

fn emit_solution(
    work: &Work,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
    progress: &mut ProgressSink<'_>,
) -> Result<(), ServiceError> {
    let _ = progress.graph_with_subjects(
        work,
        state,
        solution_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.ensure()
}

fn observe_selected_search(
    initial: &World,
    search: Search,
    progress: &mut ProgressSink<'_>,
    problem: &ProblemPath,
) -> Result<(), ServiceError> {
    let expanded = std::rc::Rc::new(std::cell::RefCell::new(None));
    let frontier = search.frontier();
    if search == Search::BreadthFirst {
        let capture = std::rc::Rc::clone(&expanded);
        let mut session = BfsSession::new(initial, maze::goal(), move |world| {
            *capture.borrow_mut() = capture_expansion(world);
            maze::candidates(world)
        });
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                emit_expansion(&frontier, session.progress(), expansion, progress)?;
            }
        }
        if let Some(solution) = session.solution() {
            emit_solution(
                &search.solution(),
                session.progress(),
                solved_expansion(initial, solution.trace(), problem)?,
                progress,
            )?;
        }
    } else {
        let capture = std::rc::Rc::clone(&expanded);
        let mut session = AStarSession::new(
            initial,
            maze::goal(),
            move |world| {
                *capture.borrow_mut() = capture_expansion(world);
                maze::candidates(world)
            },
            maze::move_energy,
            search.heuristic(),
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer);
            if let Some(expansion) = expanded.borrow_mut().take() {
                emit_expansion(&frontier, session.progress(), expansion, progress)?;
            }
        }
        if let Some(solution) = session.solution() {
            emit_solution(
                &search.solution(),
                session.progress(),
                solved_expansion(initial, solution.trace(), problem)?,
                progress,
            )?;
        }
    }
    Ok(())
}

fn frontier_trace(
    result: &maze::ParetoResult,
    energy_first: bool,
    problem: &ProblemPath,
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
        .ok_or_else(|| problem_error(problem, "empty Pareto frontier"))
}

fn objective_value(values: &[Objective<ObjectiveKey, u64>], key: ObjectiveKey) -> u64 {
    values
        .iter()
        .find(|objective| objective.key() == &key)
        .map_or(0, |objective| *objective.value())
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView::new(
            path!("energy"),
            "Energy spent",
            ObjectiveDirectionView::Minimize,
            maze::spent_energy(world),
        ),
        ObjectiveView::new(
            path!("time"),
            "Time spent",
            ObjectiveDirectionView::Minimize,
            maze::spent_time(world),
        ),
    ]
}

fn front_view(result: &maze::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|objective| objective.value)
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Energy vs. time — every non-dominated route".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: path!("energy"),
                label: "Energy spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: path!("time"),
                label: "Time spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let exact = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|objective| Quantity::from(*objective.value()))
                    .collect::<Vec<_>>();
                let values = exact.iter().map(ToString::to_string).collect::<Vec<_>>();
                ParetoPointView {
                    label: format!("{} energy · {} time", values[0], values[1]),
                    selected: exact == selected,
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
    let node_key = |node| path!("node").then(&node);
    let graph_nodes = nodes
        .iter()
        .copied()
        .map(|node| {
            let (x, y) = positions(node);
            let mut classes = if node == Node::Gate {
                vec![path!("facility")]
            } else {
                vec![path!("location")]
            };
            if focus == Some(node) {
                classes.push(path!("current"));
            }
            if node == Node::Exit {
                classes.push(path!("goal"));
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
                id: path!("edge").then(from).then(to),
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
                    vec![path!("locked")]
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
                path!("agent/explorer"),
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
            asset_metric(path!("energy"), "Energy", &Asset::Energy, energy),
            asset_metric(path!("time"), "Time", &Asset::Time, time),
        ];
        entity
    });
    let key = if !world.balance(&AccountId::World, &Asset::Key).is_zero() {
        Some(link_balance::<maze::Ontology>(
            visual_entity(
                path!("maze/key/brass"),
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
                path!("maze/key/brass"),
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::Entity {
                    entity: path!("agent/explorer"),
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
            path!("maze/gate/state"),
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
            path!("energy-left"),
            "Explorer energy",
            &Asset::Energy,
            world.balance(&AccountId::Agent, &Asset::Energy),
        ),
        asset_metric(
            path!("time-left"),
            "Time remaining",
            &Asset::Time,
            world.balance(&AccountId::Agent, &Asset::Time),
        ),
        visual_metric(
            path!("key-state"),
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
        visual_metric(path!("gate-state"), "Vault gate", gate_status, None),
    ]);
    if enhanced {
        scene.metrics.extend([
            visual_metric(
                path!("power"),
                "Shared power",
                if vault_power {
                    "vault shutter"
                } else {
                    "canal crossing"
                },
                None,
            ),
            asset_metric(
                path!("injury"),
                "Injury delay per action",
                &Asset::Injury,
                world.balance(&AccountId::Agent, &Asset::Injury),
            ),
            asset_metric(
                path!("shield"),
                "Protection remaining",
                &Asset::Shield,
                world.balance(&AccountId::Agent, &Asset::Shield),
            ),
            asset_metric(
                path!("health"),
                "Health",
                &Asset::Health,
                world.balance(&AccountId::Agent, &Asset::Health),
            ),
        ]);
        for (key, label, at, glyph, state) in [
            (
                path!("power-library"),
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
                path!("power-workshop"),
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
                path!("medical"),
                "Aid station",
                Node::Garden,
                SceneGlyphView::Repair,
                "treat injuries · 2 ticks",
            ),
            (
                path!("protection"),
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
                key,
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
            Some(path!("edge").then(from).then(to))
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
        let route: [ClassPath; 3] = [path!("current"), path!("traversed"), path!("candidate")];
        let [current_class, traversed_class, candidate_class] = &route;
        for edge in edges {
            edge.classes.retain(|class| !route.contains(class));
            if current == Some(&edge.id) {
                edge.classes.push(current_class.clone());
            } else if traversed.contains(&edge.id) {
                edge.classes.push(traversed_class.clone());
            } else if planned.contains(&edge.id) {
                edge.classes.push(candidate_class.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A strategy the catalog offers is one this adapter reads, so a study
    /// of it never fails as unknown.
    #[test]
    fn every_catalog_strategy_is_read() {
        let problem = catalog()
            .into_iter()
            .find(|problem| problem.key == path!("maze"))
            .expect("the catalog lists the maze");
        for strategy in &problem.strategies {
            assert!(
                path::read::<_, Strategy>(&strategy.key).is_ok(),
                "{}",
                strategy.key
            );
        }
    }
}
