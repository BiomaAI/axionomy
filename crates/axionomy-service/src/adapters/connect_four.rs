use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Line, Player, World};
use axionomy_view::{
    GridCellView, ObjectiveDirectionView, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, TelemetryKindView,
};

/// Connect Four: its catalog entry and how its studies are computed.
pub(crate) struct ConnectFour {
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
                ProblemFamily::AdversarialGame,
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
    ) -> Result<Vec<ViewDocument>, ServiceError> {
        build(key, progress)
    }
}

pub(super) fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
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
            &Work::new(
                path!("mcts-game"),
                path!("mcts-game"),
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
                    path!("center-opponent"),
                    path!("center-opponent"),
                    SearchObservationKindView::Phase,
                ),
                "Connect Four · against a center-first opponent",
                connect_four::Opponent::CenterFirst,
                "Center-first opponent",
            ),
            (
                path!("search-opponent"),
                Work::new(
                    path!("search-opponent"),
                    path!("search-opponent"),
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
                classes: vec![path!("connect-slot")],
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
                path!("turn"),
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
                path!("wins"),
                "Winning columns",
                columns(&evidence.winning_columns),
                None,
            ),
            visual_metric(
                path!("opponent-wins"),
                "Opponent threats",
                columns(&evidence.opponent_winning_columns),
                None,
            ),
            visual_metric(
                path!("forks"),
                "Double-threat moves",
                columns(&evidence.fork_columns),
                None,
            ),
            visual_metric(
                path!("safe"),
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
