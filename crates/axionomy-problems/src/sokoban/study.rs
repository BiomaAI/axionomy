//! The Sokoban warehouse as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    sokoban::{self, AccountId, Asset, RateId, Solution, World},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::path;
use axionomy_search::{
    AStarSession, BfsSession, GraphSearchProgress,
    session::{Continue, WorkBudget},
};
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProblemPath, ProposalSpec, ServiceError, StudyKey, StudyProgress, Work, document,
    problem_error, proposal, strategy, telemetry,
};
use axionomy_view::{SearchObservationKindView, TelemetryKindView, ViewDocument};

/// Sokoban: its catalog entry and how its studies are computed.
pub struct Sokoban {
    descriptor: ProblemDescriptor,
}

impl Default for Sokoban {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("sokoban"),
                    title: "Sokoban",
                    summary: "Push crates onto their goal squares. Every push rewrites three cells at once, and some positions can never be recovered.",
                    instances: [
                        "A bounded one-crate teaching puzzle",
                        "Two crates, a pressure plate, a gated chamber, and a remote latch",
                        "Three crates and two coupled door circuits across three chambers",
                    ],
                },
                ProblemFamily::Pathfinding,
                path!("a-star"),
                &[
                    StrategyCopy {
                        key: path!("a-star"),
                        label: "Goal-directed A*",
                        description: "A* prioritises states using an admissible crate-to-goal distance bound.",
                        algorithm: "A*",
                    },
                    StrategyCopy {
                        key: path!("breadth-first"),
                        label: "Fewest exchanges",
                        description: "Breadth-first search explores every shorter replay before accepting a solution.",
                        algorithm: "breadth-first search",
                    },
                ],
                &[
                    Capability::DeterministicSearch,
                    Capability::MultiAccountExchange,
                    Capability::FeasibilityAssessment,
                ],
            ),
        }
    }
}

impl Problem for Sokoban {
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

/// Sokoban's strategies, as the catalog lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
enum Strategy {
    AStar,
    BreadthFirst,
}

impl Strategy {
    /// The algorithm in words, as its telemetry is titled.
    fn title(self) -> &'static str {
        match self {
            Self::AStar => "A* with crate-to-goal lower bound",
            Self::BreadthFirst => "breadth-first search",
        }
    }

    /// The search expanding its frontier.
    fn frontier(self) -> Work {
        let (phase, algorithm) = match self {
            Self::AStar => (path!("a-star-frontier"), path!("a-star")),
            Self::BreadthFirst => (path!("breadth-first-frontier"), path!("breadth-first")),
        };
        Work::new(phase, algorithm, SearchObservationKindView::Frontier)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let profile = study.instance;
    let initial = match profile {
        InstanceProfile::Micro => sokoban::initial(),
        InstanceProfile::Showcase => sokoban::initial_showcase(),
        InstanceProfile::Stress => sokoban::initial_stress(),
    };
    let strategy: Strategy = strategy(study)?;
    let chunk = usize::try_from(study.budget.clamp(8, 256)).unwrap_or(64);
    let (solution, search) = search(&initial, strategy, chunk, progress, &study.problem)?;
    let mut solved = document(
        progress,
        DocumentSpec {
            problem: &study.problem,
            document: path::write(&strategy),
            title: "Sokoban · warehouse solved",
            description: "Crates hold pressure plates to open doors. Cross into the next chamber, latch the door, recover the staged crate, and reach the far exit after completing every goal. Each move and mechanism is an atomic economic exchange.",
            source_label: "Sokoban",
        },
        &initial,
        &sokoban::goal(),
        solution.trace(),
        Vec::new(),
        scene,
    )
    .map_err(|error| problem_error(&study.problem, error))?;
    solved.telemetry.push(telemetry(
        strategy.title(),
        true,
        [
            (
                TelemetryKindView::Expanded,
                search.expanded() as u64,
                "verified economic states expanded".into(),
            ),
            (
                TelemetryKindView::Generated,
                search.generated() as u64,
                "distinct successors generated".into(),
            ),
            (
                TelemetryKindView::Message,
                solution.trace().exchanges().len() as u64,
                "replayable exchanges in solution".into(),
            ),
        ],
    ));

    if let Some(rate) = initial.rate_ids().find(|rate| {
        matches!(rate, RateId::Push { crate_at, crate_id, .. }
            if initial.balance(&AccountId::Cell(*crate_at), &Asset::Crate(*crate_id)).is_zero())
    }) {
        let missing_crate = sokoban::exchange(&initial, *rate);
        solved.proposals.push(proposal(ProposalSpec {
                id: path!("push-without-crate"),
                label: "Push an empty floor tile",
                description: "The geometry names three real floor cells, but the middle account does not own the requested crate identity.",
            },
            &initial,
            &missing_crate,
        ));
    }

    let mut documents = vec![solved];
    if let Some(deadlock_trace) = sokoban::losing_trace(&initial) {
        let deadlocked = initial
            .replayed(&deadlock_trace)
            .map_err(|error| problem_error(&study.problem, error))?;
        let mut deadlock = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: path!("deadlock"),
                title: "Sokoban · legal plan, losing outcome",
                description: "Starting from the same warehouse, replay a sequence that is valid under every rule yet strands a stable crate in a square from which no goal is reachable. Valid is not the same as wise.",
                source_label: "Sokoban",
            },
            &initial,
            &sokoban::goal(),
            &deadlock_trace,
            Vec::new(),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        let trapped = sokoban::crate_cells(&deadlocked)
            .into_iter()
            .filter(|(_, cell)| sokoban::is_dead_square(&deadlocked, *cell))
            .count();
        deadlock.telemetry.push(telemetry(
            "reverse-push reachability proof",
            true,
            [(
                TelemetryKindView::Message,
                trapped as u64,
                "crates trapped on non-goal dead squares after the replay".into(),
            )],
        ));
        documents.push(deadlock);
    }

    Ok(documents)
}

fn search(
    initial: &World,
    strategy: Strategy,
    chunk: usize,
    progress: &mut StudyProgress<'_>,
    problem: &ProblemPath,
) -> Result<(Solution, GraphSearchProgress), ServiceError> {
    let frontier = strategy.frontier();
    if strategy == Strategy::AStar {
        let mut session = AStarSession::new(
            initial,
            sokoban::goal(),
            sokoban::search_candidates,
            |_, _, _| 1,
            sokoban::crate_goal_distance,
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(chunk), &mut observer);
            let state = session.progress();
            let _ = progress.graph(
                &frontier,
                state,
                format!(
                    "{} expanded · {} frontier · {} distinct states",
                    state.expanded(),
                    state.frontier(),
                    state.visited()
                ),
            );
            progress.checkpoint()?;
        }
        let state = session.progress();
        let solution = session
            .into_solution()
            .ok_or_else(|| problem_error(problem, "puzzle has no solution"))?;
        Ok((solution, state))
    } else {
        let mut session = BfsSession::new(initial, sokoban::goal(), sokoban::search_candidates);
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(chunk), &mut observer);
            let state = session.progress();
            let _ = progress.graph(
                &frontier,
                state,
                format!(
                    "{} expanded · {} frontier · {} distinct states",
                    state.expanded(),
                    state.frontier(),
                    state.visited()
                ),
            );
            progress.checkpoint()?;
        }
        let state = session.progress();
        let solution = session
            .into_solution()
            .ok_or_else(|| problem_error(problem, "puzzle has no solution"))?;
        Ok((solution, state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strategy the catalog offers is one this adapter reads, so a study
    /// of it never fails as unknown.
    #[test]
    fn every_catalog_strategy_is_read() {
        let problem = Sokoban::default();
        for strategy in &problem.descriptor().strategies {
            assert!(
                path::read::<_, Strategy>(&strategy.key).is_ok(),
                "{}",
                strategy.key
            );
        }
    }
}
