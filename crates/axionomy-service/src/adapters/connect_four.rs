use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Player, World};
use axionomy_view::{
    GridCellView, ObjectiveDirectionView, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, TelemetryKindView,
};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = run.instance;
    let standard = !matches!(profile, InstanceProfile::Micro);
    let initial = if standard {
        connect_four::initial_standard()
    } else {
        connect_four::initial()
    };
    let iterations = match profile {
        InstanceProfile::Micro => run.budget.clamp(8, 64),
        InstanceProfile::Showcase => (run.budget / 8).clamp(8, 32),
        InstanceProfile::Stress => (run.budget / 4).clamp(32, 128),
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
            slug("mcts-game"),
            completed as u64,
            total as u64,
            format!(
                "action {}/{} · {}/{} MCTS iterations · {} nodes",
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
            run.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    } else {
        connect_four::play_game_with_progress(
            iterations,
            run.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    };
    progress.ensure()?;
    let trace = trace.ok_or_else(|| problem_error("connect-four", "MCTS game was interrupted"))?;
    let final_world = initial
        .replayed(&trace)
        .map_err(|error| problem_error("connect-four", error))?;
    let terminal = connect_four::terminal_values(&final_world)
        .ok_or_else(|| problem_error("connect-four", "game did not terminate"))?;
    let mut view = document(DocumentSpec { problem: "connect-four", strategy: "mcts-game", title: "Connect Four · MCTS self-play", description: "Both players use tactical MCTS: immediate wins, forced blocks and double threats guide search. The economy declares the winner.", source_label: "Connect Four" }, &initial, &axionomy::Goal::new(), &trace, vec![
        ObjectiveView { key: slug("red"), label: "Red terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[0]) },
        ObjectiveView { key: slug("yellow"), label: "Yellow terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[1]) },
    ], scene).map_err(|error| problem_error("connect-four", error))?;
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
        view.proposals.push(proposal(ProposalSpec { id: "move-without-board", label: "Move without board bindings", description: "A drop must name the game, column, landing cell, and result. Without them, gravity has nothing to act on." }, &initial, &malformed));
    }
    let mut documents = vec![view];
    {
        for (strategy, title, opponent) in [
            (
                "center-opponent",
                "Connect Four · against a center-first opponent",
                connect_four::Opponent::CenterFirst,
            ),
            (
                "search-opponent",
                "Connect Four · against a depth-five opponent",
                connect_four::Opponent::AlphaBeta,
            ),
        ] {
            let trace = connect_four::play_opponent_game_from_with_progress(
                initial.clone(),
                iterations,
                run.seed,
                opponent,
                |turn| {
                    progress.emit(
                        slug(strategy),
                        turn as u64,
                        85,
                        format!("{opponent:?} comparison · action {turn}"),
                    )
                },
            );
            progress.ensure()?;
            let trace = trace
                .ok_or_else(|| problem_error("connect-four", "opponent comparison interrupted"))?;
            let final_world = initial
                .replayed(&trace)
                .map_err(|error| problem_error("connect-four", error))?;
            let terminal = connect_four::terminal_values(&final_world)
                .ok_or_else(|| problem_error("connect-four", "comparison did not terminate"))?;
            documents.push(document(DocumentSpec {
                problem: "connect-four", strategy, title,
                description: "Red uses tactical MCTS. Yellow uses the named independent policy on the same empty board. One game is an inspectable matchup, not a win-rate estimate.", source_label: "Connect Four",
            }, &initial, &axionomy::Goal::new(), &trace, vec![
                ObjectiveView { key: slug("red"), label: "Red terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[0]) },
                ObjectiveView { key: slug("yellow"), label: "Yellow terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[1]) },
            ], scene).map_err(|error| problem_error("connect-four", error))?);
        }
    }
    artifact(run, descriptor, documents)
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let evidence = connect_four::tactical_evidence(world);
    let columns = |values: &[u8]| {
        if values.is_empty() {
            "none".to_string()
        } else {
            values
                .iter()
                .map(|column| (column + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let mut cells = Vec::new();
    let mut pieces = Vec::new();
    let (width, height) = connect_four::board_dimensions(world);
    for column in 0..width {
        for row in 0..height {
            let account = AccountId::Cell { column, row };
            let player = if !world
                .balance(&account, &Asset::Piece(Player::Red))
                .is_zero()
            {
                Some(Player::Red)
            } else if !world
                .balance(&account, &Asset::Piece(Player::Yellow))
                .is_zero()
            {
                Some(Player::Yellow)
            } else {
                None
            };
            let y = height - row - 1;
            cells.push(GridCellView {
                x: u32::from(column),
                y: u32::from(y),
                label: player
                    .map_or_else(|| "Open slot".into(), |player| format!("{player:?} piece")),
                classes: vec![slug("connect-slot")],
                account: Some(account_key::<connect_four::Ontology>(&account)),
            });
            if let Some(player) = player {
                pieces.push(link_balance::<connect_four::Ontology>(
                    visual_entity(
                        slug(format!("piece/{player:?}/{column}/{row}")),
                        format!("{player:?}"),
                        SceneGlyphView::Token,
                        SceneAnchorView::GridCell {
                            x: u32::from(column),
                            y: u32::from(y),
                        },
                        SceneEntityRoleView::Occupant,
                        match player {
                            Player::Red => SceneToneView::Danger,
                            Player::Yellow => SceneToneView::Warning,
                        },
                        Some(format!("{player:?}").to_ascii_lowercase()),
                    ),
                    &account,
                    &Asset::Piece(player),
                ));
            }
        }
    }
    Some(
        Scene::grid(
            "Board and pieces",
            u32::from(width),
            u32::from(height),
            cells,
        )
        .with_entities(pieces)
        .with_metrics([
            visual_metric(
                slug("turn"),
                "Next player",
                evidence.player.map_or_else(
                    || {
                        if connect_four::terminal_values(world).is_some() {
                            "finished".into()
                        } else {
                            "adjudicating".into()
                        }
                    },
                    |player| format!("{player:?}"),
                ),
                None,
            ),
            visual_metric(
                slug("wins"),
                "Winning columns",
                columns(&evidence.winning_columns),
                None,
            ),
            visual_metric(
                slug("opponent-wins"),
                "Opponent threats",
                columns(&evidence.opponent_winning_columns),
                None,
            ),
            visual_metric(
                slug("forks"),
                "Double-threat moves",
                columns(&evidence.fork_columns),
                None,
            ),
            visual_metric(
                slug("safe"),
                "Safe next columns",
                columns(&evidence.safe_columns),
                None,
            ),
        ]),
    )
}

// How Studio names and writes this problem's values.

counted!(connect_four::Asset);

impl StudioLabel for connect_four::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Game => "Connect Four game".into(),
            Self::Column(column) => format!("Column {column}"),
            Self::Cell { column, row } => format!("Cell ({column},{row})"),
            Self::Result => "Game result".into(),
        }
    }
}

impl StudioLabel for connect_four::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::SegmentVacancies(segment) => format!("Unoccupied cells in line {segment}"),
            Self::SegmentPieces(player, segment) => format!("{player:?} pieces in line {segment}"),
            Self::GameIdentity => "Game identity".into(),
            Self::ColumnIdentity(column) => format!("This is column {column}"),
            Self::CellIdentity { column, row } => format!("This is cell ({column},{row})"),
            Self::ResultIdentity => "Result identity".into(),
            Self::Empty => "Empty cell".into(),
            Self::Piece(player) => format!("{player:?} piece"),
            Self::NextRow(row) => format!("Next piece lands on row {row}"),
            Self::ColumnFull => "Column full".into(),
            Self::Turn(player) => format!("{player:?} to move"),
            Self::LineCount(player, line, count) => {
                format!("{player:?} has {count} in {line:?}")
            }
            Self::Winner(player) => format!("{player:?} wins"),
            Self::Draw => "Draw".into(),
            Self::BoardSize { width, height } => format!("Board is {width}×{height}"),
            Self::Pending(player) => format!("Check {player:?} move result"),
        }
    }
}

impl StudioLabel for connect_four::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Move {
                player,
                column,
                row,
                ..
            }
            | Self::StandardMove {
                player,
                column,
                row,
            } => format!("{player:?} drops into column {column}, lands on row {row}"),
            Self::Draw(player) | Self::StandardDraw(player) => {
                format!("Declare a draw after {player:?}'s move")
            }
            Self::ClaimWin { player, segment } => {
                format!("{player:?} claims winning line {segment}")
            }
            Self::Continue(player) => format!("No win; pass the turn after {player:?}"),
        }
    }
}

debug_labels!(connect_four::Role);
