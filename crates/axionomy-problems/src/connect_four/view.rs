//! How the Connect Four game is presented: its labels, units, scene and leaderboards.

use crate::connect_four::{self, AccountId, Asset, Line, Player, World};
use axionomy::path;
use axionomy_view::{
    GridCellView, Keyed, Label, Scene, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, account_key, counted, link_balance, visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
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

impl Label for connect_four::AccountId {
    fn label(&self) -> String {
        match *self {
            Self::Game => "Connect Four game".into(),
            Self::Column(column) => format!("Column {}", column + 1),
            Self::Cell { column, row } => format!("Cell at {}", cell(column, row)),
            Self::Result => "Game result".into(),
        }
    }
}

impl Label for connect_four::Asset {
    fn label(&self) -> String {
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

impl Label for connect_four::RateId {
    fn label(&self) -> String {
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

impl Label for connect_four::Role {
    fn label(&self) -> String {
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
