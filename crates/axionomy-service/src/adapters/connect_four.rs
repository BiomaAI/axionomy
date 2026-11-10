use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Line, Player, World};
use axionomy_view::{
    GridCellView, ObjectiveDirectionView, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, TelemetryKindView,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    let standard = !matches!(profile, InstanceProfile::Micro);
    let initial = if standard {
        connect_four::initial_standard()
    } else {
        connect_four::initial()
    };
    let iterations = match profile {
        InstanceProfile::Micro => study.budget.clamp(8, 64),
        InstanceProfile::Showcase => (study.budget / 8).clamp(8, 32),
        InstanceProfile::Stress => (study.budget / 4).clamp(32, 128),
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
    progress.ensure()?;
    let trace = trace.ok_or_else(|| problem_error("connect-four", "MCTS game was interrupted"))?;
    let final_world = initial
        .replayed(&trace)
        .map_err(|error| problem_error("connect-four", error))?;
    let terminal = connect_four::terminal_values(&final_world)
        .ok_or_else(|| problem_error("connect-four", "game did not terminate"))?;
    let mut view = document(progress, DocumentSpec { problem: "connect-four", strategy: "mcts-game", title: "Connect Four · MCTS self-play", description: "Both players use tactical MCTS: immediate wins, forced blocks and double threats guide search. The economy declares the winner.", source_label: "Connect Four" }, &initial, &axionomy::Goal::new(), &trace, vec![
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
        for (strategy, title, opponent, named) in [
            (
                "center-opponent",
                "Connect Four · against a center-first opponent",
                connect_four::Opponent::CenterFirst,
                "Center-first opponent",
            ),
            (
                "search-opponent",
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
                        slug(strategy),
                        turn as u64,
                        85,
                        format!("{named} comparison · move {turn}"),
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
            documents.push(document(progress, DocumentSpec {
                problem: "connect-four", strategy, title,
                description: "Red uses tactical MCTS. Yellow uses the named independent policy on the same empty board. One game is an inspectable matchup, not a win-rate estimate.", source_label: "Connect Four",
            }, &initial, &axionomy::Goal::new(), &trace, vec![
                ObjectiveView { key: slug("red"), label: "Red terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[0]) },
                ObjectiveView { key: slug("yellow"), label: "Yellow terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[1]) },
            ], scene).map_err(|error| problem_error("connect-four", error))?);
        }
    }
    assemble(study, descriptor, documents)
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
                        path!("piece").then(&player).then(&column).then(&row),
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

// How Studio names and writes this problem's values. Columns and rows are
// counted from 1, as the scene counts them, with row 1 at the bottom.

counted!(connect_four::Asset);

fn player(player: Player) -> &'static str {
    match player {
        Player::Red => "Red",
        Player::Yellow => "Yellow",
    }
}

fn cell(column: u8, row: u8) -> String {
    format!("column {}, row {}", column + 1, row + 1)
}

/// Where a count of pieces lies: "in row 2", "on the main diagonal".
fn along(line: Line) -> String {
    match line {
        Line::Row(row) => format!("in row {}", row + 1),
        Line::Column(column) => format!("in column {}", column + 1),
        Line::MainDiagonal => "on the main diagonal".into(),
        Line::AntiDiagonal => "on the anti-diagonal".into(),
        Line::Segment(segment) => format!("in line {}", segment + 1),
    }
}

/// "a, b and c".
fn listed(items: &[String]) -> String {
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => items.concat(),
    }
}

impl StudioLabel for connect_four::AccountId {
    fn studio_label(&self) -> String {
        match *self {
            Self::Game => "Connect Four game".into(),
            Self::Column(column) => format!("Column {}", column + 1),
            Self::Cell { column, row } => format!("Cell at {}", cell(column, row)),
            Self::Result => "Game result".into(),
        }
    }
}

impl StudioLabel for connect_four::Asset {
    fn studio_label(&self) -> String {
        match *self {
            Self::SegmentVacancies(segment) => {
                format!("Unoccupied cells in line {}", segment + 1)
            }
            Self::SegmentPieces(who, segment) => {
                format!("{} pieces in line {}", player(who), segment + 1)
            }
            Self::GameIdentity => "Game identity".into(),
            Self::ColumnIdentity(column) => format!("This is column {}", column + 1),
            Self::CellIdentity { column, row } => {
                format!("This is the cell at {}", cell(column, row))
            }
            Self::ResultIdentity => "Result identity".into(),
            Self::Empty => "Empty cell".into(),
            Self::Piece(who) => format!("{} piece", player(who)),
            Self::NextRow(row) => format!("Next piece lands on row {}", row + 1),
            Self::ColumnFull => "Column full".into(),
            Self::Turn(who) => format!("{} to move", player(who)),
            Self::LineCount(who, line, count) => {
                format!("{} has {count} {}", player(who), along(line))
            }
            Self::Winner(who) => format!("{} wins", player(who)),
            Self::Draw => "Draw".into(),
            Self::BoardSize { width, height } => format!("Board is {width}×{height}"),
            Self::Pending(who) => format!("{}'s move awaits a win check", player(who)),
        }
    }
}

impl StudioLabel for connect_four::RateId {
    fn studio_label(&self) -> String {
        match *self {
            // A move rate exists for each count of the mover's pieces in the
            // lines through the landing cell; the label names the counts the
            // move makes.
            Self::Move {
                player: who,
                column,
                row,
                counts,
            } => {
                let made = [
                    Some(format!("{} in the row", counts.row() + 1)),
                    Some(format!("{} in the column", counts.column() + 1)),
                    counts
                        .main_diagonal()
                        .map(|count| format!("{} on the main diagonal", count + 1)),
                    counts
                        .anti_diagonal()
                        .map(|count| format!("{} on the anti-diagonal", count + 1)),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
                format!(
                    "{} drops into column {}, lands on row {}, making {}",
                    player(who),
                    column + 1,
                    row + 1,
                    listed(&made)
                )
            }
            Self::StandardMove {
                player: who,
                column,
                row,
            } => format!(
                "{} drops into column {}, lands on row {}",
                player(who),
                column + 1,
                row + 1
            ),
            Self::Draw(who) | Self::StandardDraw(who) => {
                format!("Declare a draw after {}'s move", player(who))
            }
            Self::ClaimWin {
                player: who,
                segment,
            } => format!("{} claims winning line {}", player(who), segment + 1),
            Self::Continue(who) => format!(
                "No win for {}; pass the turn to {}",
                player(who),
                player(who.other())
            ),
        }
    }
}

impl StudioLabel for connect_four::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::Game => "Game".into(),
            Self::Column => "Column played".into(),
            Self::Cell => "Landing cell".into(),
            Self::Result => "Result".into(),
            Self::Column0 => "Column 1".into(),
            Self::Column1 => "Column 2".into(),
            Self::Column2 => "Column 3".into(),
            Self::Column3 => "Column 4".into(),
            Self::Column4 => "Column 5".into(),
            Self::Column5 => "Column 6".into(),
            Self::Column6 => "Column 7".into(),
            Self::Winning0 => "Winning cell 1".into(),
            Self::Winning1 => "Winning cell 2".into(),
            Self::Winning2 => "Winning cell 3".into(),
            Self::Winning3 => "Winning cell 4".into(),
        }
    }
}
