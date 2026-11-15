#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, basket,
};

#[cfg(feature = "packages")]
pub mod package;

pub const WIDTH: u8 = 4;
pub const HEIGHT: u8 = 4;
pub const STANDARD_WIDTH: u8 = 7;
pub const STANDARD_HEIGHT: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Player {
    Red,
    Yellow,
}

impl Player {
    pub const fn other(self) -> Self {
        match self {
            Self::Red => Self::Yellow,
            Self::Yellow => Self::Red,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Line {
    Row(u8),
    Column(u8),
    MainDiagonal,
    AntiDiagonal,
    Segment(u8),
}

pub const LINES: [Line; 10] = [
    Line::Row(0),
    Line::Row(1),
    Line::Row(2),
    Line::Row(3),
    Line::Column(0),
    Line::Column(1),
    Line::Column(2),
    Line::Column(3),
    Line::MainDiagonal,
    Line::AntiDiagonal,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Game,
    Column(u8),
    Cell { column: u8, row: u8 },
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    GameIdentity,
    ColumnIdentity(u8),
    CellIdentity { column: u8, row: u8 },
    ResultIdentity,
    Empty,
    Piece(Player),
    NextRow(u8),
    ColumnFull,
    Turn(Player),
    LineCount(Player, Line, u8),
    Winner(Player),
    Draw,
    BoardSize { width: u8, height: u8 },
    Pending(Player),
    SegmentVacancies(u8),
    SegmentPieces(Player, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Game,
    Column,
    Cell,
    Result,
    Column0,
    Column1,
    Column2,
    Column3,
    Column4,
    Column5,
    Column6,
    Winning0,
    Winning1,
    Winning2,
    Winning3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub struct LineCounts {
    row: u8,
    column: u8,
    main_diagonal: Option<u8>,
    anti_diagonal: Option<u8>,
}

impl LineCounts {
    pub const fn row(self) -> u8 {
        self.row
    }

    pub const fn column(self) -> u8 {
        self.column
    }

    pub const fn main_diagonal(self) -> Option<u8> {
        self.main_diagonal
    }

    pub const fn anti_diagonal(self) -> Option<u8> {
        self.anti_diagonal
    }

    pub fn completes_line(self) -> bool {
        self.row == 3
            || self.column == 3
            || self.main_diagonal == Some(3)
            || self.anti_diagonal == Some(3)
    }

    fn entries(self, column: u8, row: u8) -> Vec<(Line, u8)> {
        let mut entries = vec![
            (Line::Row(row), self.row),
            (Line::Column(column), self.column),
        ];
        if let Some(count) = self.main_diagonal {
            entries.push((Line::MainDiagonal, count));
        }
        if let Some(count) = self.anti_diagonal {
            entries.push((Line::AntiDiagonal, count));
        }
        entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Move {
        player: Player,
        column: u8,
        row: u8,
        counts: LineCounts,
    },
    Draw(Player),
    StandardMove {
        player: Player,
        column: u8,
        row: u8,
    },
    ClaimWin {
        player: Player,
        segment: u8,
    },
    Continue(Player),
    StandardDraw(Player),
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `connect-four` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "connect-four";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial() -> World {
    let mut game_assets = basket([(Asset::GameIdentity, 1), (Asset::Turn(Player::Red), 1)]);
    for player in [Player::Red, Player::Yellow] {
        for line in LINES {
            game_assets.insert(Asset::LineCount(player, line, 0), Quantity::new(1));
        }
    }

    let mut builder = EconomyBuilder::new()
        .account(AccountId::Game, Account::from(game_assets))
        .account(
            AccountId::Result,
            Account::from(basket([(Asset::ResultIdentity, 1)])),
        );

    for column in 0..WIDTH {
        builder = builder.account(
            AccountId::Column(column),
            Account::from(basket([
                (Asset::ColumnIdentity(column), 1),
                (Asset::NextRow(0), 1),
            ])),
        );
        for row in 0..HEIGHT {
            builder = builder.account(
                AccountId::Cell { column, row },
                Account::from(basket([
                    (Asset::CellIdentity { column, row }, 1),
                    (Asset::Empty, 1),
                ])),
            );
            for player in [Player::Red, Player::Yellow] {
                for counts in count_combinations(column, row) {
                    let rate = move_rate(player, column, row, counts);
                    builder = builder.rate(
                        RateId::Move {
                            player,
                            column,
                            row,
                            counts,
                        },
                        rate,
                    );
                }
            }
        }
    }

    for player in [Player::Red, Player::Yellow] {
        builder = builder.rate(
            RateId::Draw(player),
            Rate::new()
                .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
                .consume(Role::Game, basket([(Asset::Turn(player), 1)]))
                .preserve(
                    Role::Column0,
                    basket([(Asset::ColumnIdentity(0), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(
                    Role::Column1,
                    basket([(Asset::ColumnIdentity(1), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(
                    Role::Column2,
                    basket([(Asset::ColumnIdentity(2), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(
                    Role::Column3,
                    basket([(Asset::ColumnIdentity(3), 1), (Asset::ColumnFull, 1)]),
                )
                .preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
                .produce(Role::Result, basket([(Asset::Draw, 1)]))
                .distinct(Role::Game, Role::Column0)
                .distinct(Role::Game, Role::Column1)
                .distinct(Role::Game, Role::Column2)
                .distinct(Role::Game, Role::Column3)
                .distinct(Role::Game, Role::Result)
                .distinct(Role::Column0, Role::Column1)
                .distinct(Role::Column0, Role::Column2)
                .distinct(Role::Column0, Role::Column3)
                .distinct(Role::Column0, Role::Result)
                .distinct(Role::Column1, Role::Column2)
                .distinct(Role::Column1, Role::Column3)
                .distinct(Role::Column1, Role::Result)
                .distinct(Role::Column2, Role::Column3)
                .distinct(Role::Column2, Role::Result)
                .distinct(Role::Column3, Role::Result),
        );
    }

    builder
        .invariant(
            LinearInvariant::new(path!("cell-occupancy"))
                .weight(Asset::Empty, 1)
                .weight(Asset::Piece(Player::Red), 1)
                .weight(Asset::Piece(Player::Yellow), 1),
        )
        .invariant((0..HEIGHT).fold(
            LinearInvariant::new(path!("column-progression")).weight(Asset::ColumnFull, 1),
            |invariant, row| invariant.weight(Asset::NextRow(row), 1),
        ))
        .invariant(
            LinearInvariant::new(path!("game-phase"))
                .weight(Asset::Turn(Player::Red), 1)
                .weight(Asset::Turn(Player::Yellow), 1)
                .weight(Asset::Winner(Player::Red), 1)
                .weight(Asset::Winner(Player::Yellow), 1)
                .weight(Asset::Draw, 1),
        )
        .invariant(
            [Player::Red, Player::Yellow]
                .into_iter()
                .flat_map(|player| LINES.into_iter().map(move |line| (player, line)))
                .fold(
                    LinearInvariant::new(path!("line-counter-tokens")),
                    |invariant, (player, line)| {
                        (0..=4).fold(invariant, |invariant, count| {
                            invariant.weight(Asset::LineCount(player, line, count), 1)
                        })
                    },
                ),
        )
        .build()
        .expect("connect four model is valid")
}

/// A standard 7×6 board using compact move rates and explicit four-cell win
/// certificates. Segment balances prevent continuing or drawing after a win
/// in the core economy, including exchanges supplied outside candidate generation.
pub fn initial_standard() -> World {
    let mut game = basket([
        (Asset::GameIdentity, 1),
        (Asset::Turn(Player::Red), 1),
        (
            Asset::BoardSize {
                width: STANDARD_WIDTH,
                height: STANDARD_HEIGHT,
            },
            1,
        ),
    ]);
    for segment in 0..winning_segments().len() {
        game.insert(Asset::SegmentVacancies(segment as u8), Quantity::new(4));
    }
    let mut builder = EconomyBuilder::new()
        .account(AccountId::Game, Account::from(game))
        .account(
            AccountId::Result,
            Account::from(basket([(Asset::ResultIdentity, 1)])),
        );
    for column in 0..STANDARD_WIDTH {
        builder = builder.account(
            AccountId::Column(column),
            Account::from(basket([
                (Asset::ColumnIdentity(column), 1),
                (Asset::NextRow(0), 1),
            ])),
        );
        for row in 0..STANDARD_HEIGHT {
            builder = builder.account(
                AccountId::Cell { column, row },
                Account::from(basket([
                    (Asset::CellIdentity { column, row }, 1),
                    (Asset::Empty, 1),
                ])),
            );
            for player in [Player::Red, Player::Yellow] {
                builder = builder.rate(
                    RateId::StandardMove {
                        player,
                        column,
                        row,
                    },
                    standard_move_rate(player, column, row),
                );
            }
        }
    }
    for player in [Player::Red, Player::Yellow] {
        builder = builder
            .rate(
                RateId::Continue(player),
                no_standard_win(
                    Rate::new()
                        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
                        .consume(Role::Game, basket([(Asset::Pending(player), 1)]))
                        .produce(Role::Game, basket([(Asset::Turn(player.other()), 1)])),
                    player,
                ),
            )
            .rate(RateId::StandardDraw(player), standard_draw_rate(player));
        for (segment, cells) in winning_segments().into_iter().enumerate() {
            builder = builder.rate(
                RateId::ClaimWin {
                    player,
                    segment: segment as u8,
                },
                claim_win_rate(player, cells),
            );
        }
    }
    let mut segment_invariant = LinearInvariant::new(path!("winning-line-occupancy"));
    for segment in 0..winning_segments().len() {
        segment_invariant = segment_invariant
            .weight(Asset::SegmentVacancies(segment as u8), 1)
            .weight(Asset::SegmentPieces(Player::Red, segment as u8), 1)
            .weight(Asset::SegmentPieces(Player::Yellow, segment as u8), 1);
    }
    builder = builder.invariant(segment_invariant);
    builder
        .invariant(
            LinearInvariant::new(path!("cell-occupancy"))
                .weight(Asset::Empty, 1)
                .weight(Asset::Piece(Player::Red), 1)
                .weight(Asset::Piece(Player::Yellow), 1),
        )
        .invariant((0..STANDARD_HEIGHT).fold(
            LinearInvariant::new(path!("column-progression")).weight(Asset::ColumnFull, 1),
            |invariant, row| invariant.weight(Asset::NextRow(row), 1),
        ))
        .invariant(
            LinearInvariant::new(path!("game-phase"))
                .weight(Asset::Turn(Player::Red), 1)
                .weight(Asset::Turn(Player::Yellow), 1)
                .weight(Asset::Pending(Player::Red), 1)
                .weight(Asset::Pending(Player::Yellow), 1)
                .weight(Asset::Winner(Player::Red), 1)
                .weight(Asset::Winner(Player::Yellow), 1)
                .weight(Asset::Draw, 1),
        )
        .build()
        .expect("standard connect four model is valid")
}

fn move_rate(player: Player, column: u8, row: u8, counts: LineCounts) -> Rate<Ontology> {
    let mut consumed = basket([(Asset::Turn(player), 1)]);
    let mut produced = Basket::new();
    for (line, count) in counts.entries(column, row) {
        consumed.insert(Asset::LineCount(player, line, count), Quantity::new(1));
        produced.insert(Asset::LineCount(player, line, count + 1), Quantity::new(1));
    }

    let winning = counts.completes_line();
    if !winning {
        produced.insert(Asset::Turn(player.other()), Quantity::new(1));
    }

    let rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, consumed)
        .produce(Role::Game, produced)
        .preserve(Role::Column, basket([(Asset::ColumnIdentity(column), 1)]))
        .consume(Role::Column, basket([(Asset::NextRow(row), 1)]))
        .produce(
            Role::Column,
            if row + 1 == HEIGHT {
                basket([(Asset::ColumnFull, 1)])
            } else {
                basket([(Asset::NextRow(row + 1), 1)])
            },
        )
        .preserve(
            Role::Cell,
            basket([(Asset::CellIdentity { column, row }, 1)]),
        )
        .consume(Role::Cell, basket([(Asset::Empty, 1)]))
        .produce(Role::Cell, basket([(Asset::Piece(player), 1)]))
        .distinct(Role::Game, Role::Column)
        .distinct(Role::Game, Role::Cell)
        .distinct(Role::Column, Role::Cell);

    if winning {
        rate.preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
            .produce(Role::Result, basket([(Asset::Winner(player), 1)]))
            .distinct(Role::Game, Role::Result)
            .distinct(Role::Column, Role::Result)
            .distinct(Role::Cell, Role::Result)
    } else {
        rate
    }
}

fn standard_move_rate(player: Player, column: u8, row: u8) -> Rate<Ontology> {
    let mut rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, basket([(Asset::Turn(player), 1)]))
        .produce(Role::Game, basket([(Asset::Pending(player), 1)]))
        .preserve(Role::Column, basket([(Asset::ColumnIdentity(column), 1)]))
        .consume(Role::Column, basket([(Asset::NextRow(row), 1)]))
        .produce(
            Role::Column,
            if row + 1 == STANDARD_HEIGHT {
                basket([(Asset::ColumnFull, 1)])
            } else {
                basket([(Asset::NextRow(row + 1), 1)])
            },
        )
        .preserve(
            Role::Cell,
            basket([(Asset::CellIdentity { column, row }, 1)]),
        )
        .consume(Role::Cell, basket([(Asset::Empty, 1)]))
        .produce(Role::Cell, basket([(Asset::Piece(player), 1)]))
        .distinct(Role::Game, Role::Column)
        .distinct(Role::Game, Role::Cell)
        .distinct(Role::Column, Role::Cell);
    for (segment, cells) in winning_segments().into_iter().enumerate() {
        if cells.contains(&(column, row)) {
            rate = rate
                .consume(
                    Role::Game,
                    basket([(Asset::SegmentVacancies(segment as u8), 1)]),
                )
                .produce(
                    Role::Game,
                    basket([(Asset::SegmentPieces(player, segment as u8), 1)]),
                );
        }
    }
    rate
}

fn no_standard_win(mut rate: Rate<Ontology>, player: Player) -> Rate<Ontology> {
    for segment in 0..winning_segments().len() {
        rate = rate.condition(
            path!("segment-is-not-a-win").join(&path::write(&segment)),
            Q::balance(Role::Game, Asset::SegmentPieces(player, segment as u8)),
            C::LessThanOrEqual,
            Q::constant(3),
        );
    }
    rate
}

fn claim_win_rate(player: Player, cells: [(u8, u8); 4]) -> Rate<Ontology> {
    let roles = [
        Role::Winning0,
        Role::Winning1,
        Role::Winning2,
        Role::Winning3,
    ];
    let mut rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, basket([(Asset::Pending(player), 1)]))
        .preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
        .produce(Role::Result, basket([(Asset::Winner(player), 1)]))
        .distinct(Role::Game, Role::Result);
    for (role, (column, row)) in roles.into_iter().zip(cells) {
        rate = rate
            .preserve(
                role,
                basket([
                    (Asset::CellIdentity { column, row }, 1),
                    (Asset::Piece(player), 1),
                ]),
            )
            .distinct(Role::Game, role)
            .distinct(Role::Result, role);
    }
    for (index, left) in roles.iter().enumerate() {
        for right in &roles[index + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    rate
}

fn standard_draw_rate(player: Player) -> Rate<Ontology> {
    let roles = [
        Role::Column0,
        Role::Column1,
        Role::Column2,
        Role::Column3,
        Role::Column4,
        Role::Column5,
        Role::Column6,
    ];
    let mut rate = Rate::new()
        .preserve(Role::Game, basket([(Asset::GameIdentity, 1)]))
        .consume(Role::Game, basket([(Asset::Pending(player), 1)]))
        .preserve(Role::Result, basket([(Asset::ResultIdentity, 1)]))
        .produce(Role::Result, basket([(Asset::Draw, 1)]))
        .distinct(Role::Game, Role::Result);
    for (column, role) in roles.into_iter().enumerate() {
        rate = rate
            .preserve(
                role,
                basket([
                    (Asset::ColumnIdentity(column as u8), 1),
                    (Asset::ColumnFull, 1),
                ]),
            )
            .distinct(Role::Game, role)
            .distinct(Role::Result, role);
    }
    for (index, left) in roles.iter().enumerate() {
        for right in &roles[index + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    no_standard_win(rate, player)
}

fn winning_segments() -> Vec<[(u8, u8); 4]> {
    let mut segments = Vec::with_capacity(69);
    for row in 0..STANDARD_HEIGHT {
        for column in 0..=STANDARD_WIDTH - 4 {
            segments.push([
                (column, row),
                (column + 1, row),
                (column + 2, row),
                (column + 3, row),
            ]);
        }
    }
    for column in 0..STANDARD_WIDTH {
        for row in 0..=STANDARD_HEIGHT - 4 {
            segments.push([
                (column, row),
                (column, row + 1),
                (column, row + 2),
                (column, row + 3),
            ]);
        }
    }
    for column in 0..=STANDARD_WIDTH - 4 {
        for row in 0..=STANDARD_HEIGHT - 4 {
            segments.push([
                (column, row),
                (column + 1, row + 1),
                (column + 2, row + 2),
                (column + 3, row + 3),
            ]);
            segments.push([
                (column, row + 3),
                (column + 1, row + 2),
                (column + 2, row + 1),
                (column + 3, row),
            ]);
        }
    }
    segments
}

/// The exchange of one unit of `rate`, each of its roles bound to the
/// account the rate names: the game, the column and cell a drop lands in,
/// the cells of a claimed win or the columns of a draw, and the result.
pub fn exchange(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    match rate {
        RateId::Move {
            column,
            row,
            counts,
            ..
        } => {
            let exchange = exchange
                .bind(Role::Game, AccountId::Game)
                .bind(Role::Column, AccountId::Column(column))
                .bind(Role::Cell, AccountId::Cell { column, row });
            if counts.completes_line() {
                exchange.bind(Role::Result, AccountId::Result)
            } else {
                exchange
            }
        }
        RateId::Draw(_) => exchange
            .bind(Role::Game, AccountId::Game)
            .bind(Role::Column0, AccountId::Column(0))
            .bind(Role::Column1, AccountId::Column(1))
            .bind(Role::Column2, AccountId::Column(2))
            .bind(Role::Column3, AccountId::Column(3))
            .bind(Role::Result, AccountId::Result),
        RateId::StandardMove { column, row, .. } => exchange
            .bind(Role::Game, AccountId::Game)
            .bind(Role::Column, AccountId::Column(column))
            .bind(Role::Cell, AccountId::Cell { column, row }),
        RateId::ClaimWin { segment, .. } => {
            let cells = winning_segments()[usize::from(segment)];
            [
                Role::Winning0,
                Role::Winning1,
                Role::Winning2,
                Role::Winning3,
            ]
            .into_iter()
            .zip(cells)
            .fold(
                exchange
                    .bind(Role::Game, AccountId::Game)
                    .bind(Role::Result, AccountId::Result),
                |exchange, (role, (column, row))| {
                    exchange.bind(role, AccountId::Cell { column, row })
                },
            )
        }
        RateId::Continue(_) => exchange.bind(Role::Game, AccountId::Game),
        RateId::StandardDraw(_) => [
            Role::Column0,
            Role::Column1,
            Role::Column2,
            Role::Column3,
            Role::Column4,
            Role::Column5,
            Role::Column6,
        ]
        .into_iter()
        .enumerate()
        .fold(
            exchange
                .bind(Role::Game, AccountId::Game)
                .bind(Role::Result, AccountId::Result),
            |exchange, (column, role)| exchange.bind(role, AccountId::Column(column as u8)),
        ),
    }
}

fn count_combinations(column: u8, row: u8) -> Vec<LineCounts> {
    let diagonal = if column == row {
        (0..4).map(Some).collect::<Vec<_>>()
    } else {
        vec![None]
    };
    let anti_diagonal = if column + row + 1 == WIDTH {
        (0..4).map(Some).collect::<Vec<_>>()
    } else {
        vec![None]
    };
    let mut combinations = Vec::new();
    for row_count in 0..4 {
        for column_count in 0..4 {
            for main_diagonal in &diagonal {
                for anti_diagonal in &anti_diagonal {
                    combinations.push(LineCounts {
                        row: row_count,
                        column: column_count,
                        main_diagonal: *main_diagonal,
                        anti_diagonal: *anti_diagonal,
                    });
                }
            }
        }
    }
    combinations
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a player already counts on the lines through a cell: its row,
    /// its column, and the diagonals it lies on.
    const fn counts(
        row: u8,
        column: u8,
        main_diagonal: Option<u8>,
        anti_diagonal: Option<u8>,
    ) -> LineCounts {
        LineCounts {
            row,
            column,
            main_diagonal,
            anti_diagonal,
        }
    }

    /// `player`'s drop on the compact board into `column`, landing in `row`.
    fn drop(player: Player, column: u8, row: u8, counts: LineCounts) -> Action {
        exchange(RateId::Move {
            player,
            column,
            row,
            counts,
        })
    }

    /// `player`'s drop on the standard board into `column`, landing in `row`.
    fn standard_drop(player: Player, column: u8, row: u8) -> Action {
        exchange(RateId::StandardMove {
            player,
            column,
            row,
        })
    }

    #[test]
    fn gravity_and_line_counts_are_core_validated() {
        let mut world = initial();
        world
            .apply(drop(Player::Red, 0, 0, counts(0, 0, Some(0), None)))
            .expect("first piece lands");

        assert_eq!(
            world.balance(
                &AccountId::Cell { column: 0, row: 0 },
                &Asset::Piece(Player::Red),
            ),
            Quantity::new(1)
        );
        assert_eq!(
            world.balance(&AccountId::Column(0), &Asset::NextRow(1)),
            Quantity::new(1)
        );
        assert_eq!(
            world.balance(
                &AccountId::Game,
                &Asset::LineCount(Player::Red, Line::Row(0), 1),
            ),
            Quantity::new(1)
        );
        // A piece cannot land above an empty cell.
        assert!(!world.is_applicable(&drop(Player::Yellow, 1, 1, counts(0, 0, Some(0), None))));
    }

    #[test]
    fn encoded_coordinates_reject_misbinding_an_otherwise_empty_cell() {
        let world = initial();
        let misplaced = Exchange::new(
            RateId::Move {
                player: Player::Red,
                column: 0,
                row: 0,
                counts: counts(0, 0, Some(0), None),
            },
            Quantity::new(1),
        )
        .bind(Role::Game, AccountId::Game)
        .bind(Role::Column, AccountId::Column(3))
        .bind(Role::Cell, AccountId::Cell { column: 3, row: 3 });

        assert!(!world.is_applicable(&misplaced));
    }

    #[test]
    fn completing_a_line_produces_the_encoded_winner() {
        let mut world = initial();
        // Red and Yellow alternate in columns 0, 0, 1, 1, 2 and 2; Red's
        // drop into column 3 completes the bottom row.
        for exchange in [
            drop(Player::Red, 0, 0, counts(0, 0, Some(0), None)),
            drop(Player::Yellow, 0, 1, counts(0, 0, None, None)),
            drop(Player::Red, 1, 0, counts(1, 0, None, None)),
            drop(Player::Yellow, 1, 1, counts(1, 0, Some(0), None)),
            drop(Player::Red, 2, 0, counts(2, 0, None, None)),
            drop(Player::Yellow, 2, 1, counts(2, 0, None, Some(0))),
        ] {
            world.apply(exchange).unwrap();
        }
        // A drop consumes the counts its player holds: Red already has
        // three in the bottom row, not two.
        assert!(!world.is_applicable(&drop(Player::Red, 3, 0, counts(2, 0, None, Some(0)))));
        world
            .apply(drop(Player::Red, 3, 0, counts(3, 0, None, Some(0))))
            .expect("winning move applies");
        assert_eq!(
            world.balance(&AccountId::Result, &Asset::Winner(Player::Red)),
            Quantity::new(1)
        );
        assert!(
            world
                .balance(&AccountId::Game, &Asset::Turn(Player::Yellow))
                .is_zero()
        );
    }

    #[test]
    fn standard_board_uses_compact_rates_and_core_validated_win_certificates() {
        let mut world = initial_standard();
        assert_eq!(
            world.balance(
                &AccountId::Game,
                &Asset::BoardSize {
                    width: 7,
                    height: 6
                }
            ),
            Quantity::new(1)
        );
        assert!(world.rate_ids().count() < 300);

        // Red and Yellow alternate in columns 0, 0, 1, 1, 2 and 2, each drop
        // adjudicated before the turn passes.
        for (player, column, row) in [
            (Player::Red, 0, 0),
            (Player::Yellow, 0, 1),
            (Player::Red, 1, 0),
            (Player::Yellow, 1, 1),
            (Player::Red, 2, 0),
            (Player::Yellow, 2, 1),
        ] {
            world
                .apply(standard_drop(player, column, row))
                .expect("piece placement applies");
            world
                .apply(exchange(RateId::Continue(player)))
                .expect("no win, so the turn passes");
        }
        // Red's drop into column 3 completes the bottom row: the turn cannot
        // pass, and the win's certificate applies.
        world.apply(standard_drop(Player::Red, 3, 0)).unwrap();
        let before = world.state_key();
        assert!(
            world
                .apply(exchange(RateId::Continue(Player::Red)))
                .is_err()
        );
        assert_eq!(before, world.state_key());
        world
            .apply(exchange(RateId::ClaimWin {
                player: Player::Red,
                segment: 0,
            }))
            .expect("the win certificate applies");
        assert_eq!(
            world.balance(&AccountId::Result, &Asset::Winner(Player::Red)),
            Quantity::new(1)
        );
        assert!(!world.is_applicable(&standard_drop(Player::Yellow, 4, 0)));
    }
}
