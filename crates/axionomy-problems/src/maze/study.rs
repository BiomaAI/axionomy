//! The key-door maze as a study: its catalog entry, strategies and documents.

use super::view::{decorate_route_evidence, scene};
use crate::{
    maze::{self, AccountId, Asset, Node, ObjectiveKey, RateId, Role, World},
    support::{Capability, Family, ProblemCopy, Size, StrategyCopy, problem, size},
};
use axionomy::{Exchange, Quantity, path};
use axionomy_search::{
    AStarSession, BfsSession,
    session::{Continue, WorkBudget},
};
use axionomy_study::{
    DocumentSpec, Problem, ProblemDescriptor, ProblemPath, ProposalSpec, StudyError, StudyKey,
    StudyProgress, Work, document, frontier_trace, graph_metrics, problem_error, proposal,
    retain_observation, strategy, telemetry,
};
use axionomy_view::{
    Keyed, Label, Labeled, LabeledAlgorithm, ObjectiveAxisView, ObjectiveDirectionView,
    ObjectiveView, ParetoFrontView, SearchObservationKindView, SearchObservationView,
    TelemetryKindView, ViewDocument, ViewId, exact_front, exact_values, visual_metric,
};
use std::collections::VecDeque;

/// Key-door maze: its catalog entry and how its studies are computed.
pub struct Maze {
    descriptor: ProblemDescriptor,
}

impl Default for Maze {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("maze"),
                    title: "Key-door maze",
                    summary: "Reach the exit under limited energy and time. The key-and-gate route saves energy, direct routes save time, and the exact frontier preserves four tradeoffs that one score would hide.",
                    instances: [
                        "The smallest maze with a key, a locked gate, and a real route choice",
                        "15 rooms and 25 passages with shared power, protection, persistent injury delays and recovery",
                        "24 rooms and 42 passages with more hazards and a second recovery route",
                    ],
                },
                Family::Pathfinding,
                path!("a-star"),
                &[
                    StrategyCopy {
                        key: path!("breadth-first"),
                        label: "Fewest moves",
                        description: "Breadth-first search: the shortest sequence, whatever it costs.",
                        algorithm: "breadth-first search",
                    },
                    StrategyCopy {
                        key: path!("dijkstra"),
                        label: "Least energy · uninformed",
                        description: "Dijkstra proves the lowest-energy route without a heuristic.",
                        algorithm: "Dijkstra",
                    },
                    StrategyCopy {
                        key: path!("a-star"),
                        label: "Least energy",
                        description: "A*, guided by an admissible distance estimate stored in the economy itself.",
                        algorithm: "A*",
                    },
                    StrategyCopy {
                        key: path!("pareto-energy"),
                        label: "Pareto: least energy",
                        description: "The lowest-energy point on the exact Pareto frontier, where no route is better on both energy and time.",
                        algorithm: "exact Pareto search",
                    },
                    StrategyCopy {
                        key: path!("pareto-time"),
                        label: "Pareto: least time",
                        description: "The fastest point on that exact frontier.",
                        algorithm: "exact Pareto search",
                    },
                ],
                &[
                    Capability::DeterministicSearch,
                    Capability::WeightedSearch,
                    Capability::ExactPareto,
                    Capability::FeasibilityAssessment,
                ],
            ),
        }
    }
}

impl Problem for Maze {
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
    fn algorithm(self) -> LabeledAlgorithm {
        match self {
            Self::BreadthFirst => Labeled::new(path!("breadth-first"), "Breadth-first search"),
            Self::Dijkstra => Labeled::new(path!("dijkstra"), "Dijkstra"),
            Self::AStar => Labeled::new(path!("a-star"), "A*"),
        }
    }

    /// The search expanding its frontier.
    fn frontier(self) -> Work {
        let phase = match self {
            Self::BreadthFirst => {
                Labeled::new(path!("breadth-first-frontier"), "Breadth-first frontier")
            }
            Self::Dijkstra => Labeled::new(path!("dijkstra-frontier"), "Dijkstra frontier"),
            Self::AStar => Labeled::new(path!("a-star-frontier"), "A* frontier"),
        };
        Work::new(phase, self.algorithm(), SearchObservationKindView::Frontier)
    }

    /// The search accepting its solution.
    fn solution(self) -> Work {
        let phase = match self {
            Self::BreadthFirst => {
                Labeled::new(path!("breadth-first-solution"), "Breadth-first solution")
            }
            Self::Dijkstra => Labeled::new(path!("dijkstra-solution"), "Dijkstra solution"),
            Self::AStar => Labeled::new(path!("a-star-solution"), "A* solution"),
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
        Labeled::new(path!("pareto-frontier"), "Exact Pareto frontier"),
        Labeled::new(path!("exact-pareto-search"), "Exact Pareto search"),
        SearchObservationKindView::Frontier,
    )
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let problem = &study.problem;
    let initial = match size(study)? {
        Size::Micro => maze::initial(),
        Size::Showcase => maze::initial_showcase(),
        Size::Stress => maze::initial_stress(),
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
        progress.checkpoint()?;
    }
    let energy_trace = frontier_trace(
        &pareto,
        (ObjectiveKey::Energy, ObjectiveDirectionView::Minimize),
        (ObjectiveKey::Time, ObjectiveDirectionView::Minimize),
        problem,
    )?;
    let time_trace = frontier_trace(
        &pareto,
        (ObjectiveKey::Time, ObjectiveDirectionView::Minimize),
        (ObjectiveKey::Energy, ObjectiveDirectionView::Minimize),
        problem,
    )?;

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
    Ok(documents)
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
        expansion.node.label(),
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
    ViewId::new(path!("node").then(&expansion.node), expansion.node.label())
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
        metrics: graph_metrics(state),
    }
}

fn solution_label(expansion: MazeExpansion) -> String {
    format!(
        "Solution accepted at {} · E left {} · T left {}",
        expansion.node.label(),
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
) -> Result<MazeExpansion, StudyError> {
    let solved = initial
        .replayed(trace)
        .map_err(|error| problem_error(problem, error))?;
    capture_expansion(&solved).ok_or_else(|| problem_error(problem, "solution lost its room"))
}

fn saved_graph_observations(
    initial: &World,
    search: Search,
    problem: &ProblemPath,
) -> Result<Vec<SearchObservationView>, StudyError> {
    let expanded = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut observations = VecDeque::new();
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
    Ok(observations.into())
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
    progress: &mut StudyProgress<'_>,
) -> Result<(), StudyError> {
    let _ = progress.graph_with_subjects(
        work,
        state,
        expansion_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.checkpoint()
}

fn emit_solution(
    work: &Work,
    state: axionomy_search::GraphSearchProgress,
    expansion: MazeExpansion,
    progress: &mut StudyProgress<'_>,
) -> Result<(), StudyError> {
    let _ = progress.graph_with_subjects(
        work,
        state,
        solution_label(expansion),
        [expansion_subject(expansion)],
    );
    progress.checkpoint()
}

fn observe_selected_search(
    initial: &World,
    search: Search,
    progress: &mut StudyProgress<'_>,
    problem: &ProblemPath,
) -> Result<(), StudyError> {
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
    exact_front(
        "Energy vs. time — every non-dominated route",
        vec![
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
        selected,
        result.front().entries().iter().map(exact_values),
        |values| format!("{} energy · {} time", values[0], values[1]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strategy the catalog offers is one this adapter reads, so a study
    /// of it never fails as unknown.
    #[test]
    fn every_catalog_strategy_is_read() {
        let problem = Maze::default();
        for strategy in &problem.descriptor().strategies {
            assert!(
                path::read::<_, Strategy>(&strategy.key).is_ok(),
                "{}",
                strategy.key
            );
        }
    }
}
