//! The Connect Four game as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    connect_four,
    support::{Capability, Family, ProblemCopy, Size, StrategyCopy, problem, size},
};
use axionomy::{Exchange, Quantity, path};
use axionomy_study::{
    DocumentSpec, Problem, ProblemDescriptor, ProposalSpec, StudyError, StudyKey, StudyProgress,
    Work, document, problem_error, proposal, telemetry,
};
use axionomy_view::{
    Labeled, ObjectiveDirectionView, ObjectiveView, QuantityFormat, SearchObservationKindView,
    TelemetryKindView, ViewDocument,
};

/// Connect Four: its catalog entry and how its studies are computed.
pub struct ConnectFour {
    descriptor: ProblemDescriptor,
}

impl Default for ConnectFour {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("connect-four"),
                    title: "Connect Four",
                    summary: "Tactical Connect Four with forced blocks, double threats, positional search, and distinct opponents; gravity and terminal outcomes are core rules.",
                    instances: [
                        "A compact board with a short game and few rollouts",
                        "The full 7×6 board, all 69 lines, tactical self-play and two distinct opponents",
                        "The full board with four times the MCTS work per move",
                    ],
                },
                Family::AdversarialGame,
                path!("mcts-game"),
                &[
                    StrategyCopy {
                        key: path!("mcts-game"),
                        label: "Tactical MCTS self-play",
                        description: "Both players recognize wins, forced blocks and double threats before exploring positional continuations.",
                        algorithm: "tactical Monte Carlo tree search",
                    },
                    StrategyCopy {
                        key: path!("center-opponent"),
                        label: "Center-first opponent",
                        description: "Red uses tactical MCTS against a center-first Yellow baseline on the standard board.",
                        algorithm: "tactical MCTS versus positional baseline",
                    },
                    StrategyCopy {
                        key: path!("search-opponent"),
                        label: "Depth-five opponent",
                        description: "Red uses tactical MCTS against depth-five alpha-beta Yellow on the standard board.",
                        algorithm: "tactical MCTS versus alpha-beta",
                    },
                ],
                &[Capability::Mcts, Capability::MultiAccountExchange],
            ),
        }
    }
}

impl Problem for ConnectFour {
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
    let profile = size(study)?;
    let standard = !matches!(profile, Size::Micro);
    let initial = if standard {
        connect_four::initial_standard()
    } else {
        connect_four::initial()
    };
    let iterations = match profile {
        Size::Micro => study.budget.clamp(8, 64),
        Size::Showcase => (study.budget / 8).clamp(8, 32),
        Size::Stress => (study.budget / 4).clamp(32, 128),
    } as usize;
    let mut observe = |state: connect_four::GameProgress| {
        let completed = state
            .moves_completed()
            .saturating_mul(state.iterations_per_move())
            .saturating_add(state.iterations_completed());
        let total = state
            .maximum_moves()
            .saturating_mul(state.iterations_per_move());
        progress.emit(
            &Work::new(
                Labeled::new(path!("mcts-game"), "MCTS"),
                Labeled::new(path!("mcts-game"), "MCTS self-play"),
                SearchObservationKindView::Tree,
            ),
            completed as u64,
            total as u64,
            format!(
                "Move {}/{} · {}/{} MCTS iterations · {} nodes",
                state.moves_completed() + 1,
                state.maximum_moves(),
                state.iterations_completed(),
                state.iterations_per_move(),
                state.nodes()
            ),
        )
    };
    let trace = if standard {
        connect_four::play_standard_game_with_progress(
            iterations,
            study.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    } else {
        connect_four::play_game_with_progress(
            iterations,
            study.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    };
    progress.checkpoint()?;
    let trace = trace.ok_or_else(|| problem_error(&study.problem, "MCTS game was interrupted"))?;
    let final_world = initial
        .replayed(&trace)
        .map_err(|error| problem_error(&study.problem, error))?;
    let terminal = connect_four::terminal_values(&final_world)
        .ok_or_else(|| problem_error(&study.problem, "game did not terminate"))?;
    let mut view = document(progress, DocumentSpec { problem: &study.problem, document: path!("mcts-game"), title: "Connect Four · MCTS self-play", description: "Both players use tactical MCTS: immediate wins, forced blocks and double threats guide search. The economy declares the winner.", source_label: "Connect Four" }, &initial, &axionomy::Goal::new(), &trace, terminal_objectives(&terminal), scene).map_err(|error| problem_error(&study.problem, error))?;
    view.telemetry.push(telemetry(
        "vector-valued MCTS",
        false,
        [
            (
                TelemetryKindView::Iteration,
                (iterations * trace.exchanges().len()) as u64,
                "maximum tree iterations".into(),
            ),
            (
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "played moves".into(),
            ),
        ],
    ));
    if let Some(first) = trace.exchanges().first() {
        let malformed = Exchange::new(*first.rate(), first.units());
        view.proposals.push(proposal(ProposalSpec { id: path!("move-without-board"), label: "Move without board bindings", description: "A drop must name the game, column, landing cell, and result. Without them, gravity has nothing to act on." }, &initial, &malformed));
    }
    let mut documents = vec![view];
    {
        for (strategy, phase, title, opponent, named) in [
            (
                path!("center-opponent"),
                Work::new(
                    Labeled::new(
                        path!("center-opponent"),
                        "Game against the center-first opponent",
                    ),
                    Labeled::new(path!("center-opponent"), "Center-first opponent"),
                    SearchObservationKindView::Phase,
                ),
                "Connect Four · against a center-first opponent",
                connect_four::Opponent::CenterFirst,
                "Center-first opponent",
            ),
            (
                path!("search-opponent"),
                Work::new(
                    Labeled::new(
                        path!("search-opponent"),
                        "Game against the depth-five opponent",
                    ),
                    Labeled::new(path!("search-opponent"), "Depth-five search opponent"),
                    SearchObservationKindView::Phase,
                ),
                "Connect Four · against a depth-five opponent",
                connect_four::Opponent::AlphaBeta,
                "Depth-five opponent",
            ),
        ] {
            let trace = connect_four::play_opponent_game_from_with_progress(
                initial.clone(),
                iterations,
                study.seed,
                opponent,
                |turn| {
                    progress.emit(
                        &phase,
                        turn as u64,
                        85,
                        format!("{named} comparison · move {turn}"),
                    )
                },
            );
            progress.checkpoint()?;
            let trace = trace
                .ok_or_else(|| problem_error(&study.problem, "opponent comparison interrupted"))?;
            let final_world = initial
                .replayed(&trace)
                .map_err(|error| problem_error(&study.problem, error))?;
            let terminal = connect_four::terminal_values(&final_world)
                .ok_or_else(|| problem_error(&study.problem, "comparison did not terminate"))?;
            documents.push(document(progress, DocumentSpec {
                problem: &study.problem, document: strategy, title,
                description: "Red uses tactical MCTS. Yellow uses the named independent policy on the same empty board. One game is an inspectable matchup, not a win-rate estimate.", source_label: "Connect Four",
            }, &initial, &axionomy::Goal::new(), &trace, terminal_objectives(&terminal), scene).map_err(|error| problem_error(&study.problem, error))?);
        }
    }
    Ok(documents)
}

/// Each player's terminal value, exact: a win is `1.0`, a draw `0.5` and a
/// loss `0.0`, counted in tenths.
fn terminal_objectives(terminal: &[f64]) -> Vec<ObjectiveView> {
    let tenths = |value: f64| Quantity::new((value * 10.0).round() as u64);
    let format = QuantityFormat::decimal(1).digits(1);
    vec![
        ObjectiveView::new(
            path!("red"),
            "Red terminal value",
            ObjectiveDirectionView::Maximize,
            tenths(terminal[0]),
        )
        .with_format(format.clone()),
        ObjectiveView::new(
            path!("yellow"),
            "Yellow terminal value",
            ObjectiveDirectionView::Maximize,
            tenths(terminal[1]),
        )
        .with_format(format),
    ]
}
