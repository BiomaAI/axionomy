#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, LinearInvariant, Param, Pattern, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Slot, basket,
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

/// The way a line of four cells runs: along a row, up a column, or along a
/// diagonal rising or falling to the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Direction {
    Across,
    Up,
    Rising,
    Falling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Game,
    Column(u8),
    Cell { column: u8, row: u8 },
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Empty,
    Piece(Player),
    /// The pieces a column holds: the row its next piece lands on.
    Height,
    Turn(Player),
    /// The pieces a player holds in a line of four cells running
    /// `Direction`, named by its rightmost column and its top row. A name
    /// whose cells leave the board names a line no drop fills.
    Line(Player, Direction, u8, u8),
    Winner(Player),
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Game,
    Column,
    Cell,
    Result,
    /// A column a draw finds full, counting from the left.
    Full(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    /// `player` drops a piece into `column`, landing on `row`, completing
    /// no line of four: the turn passes.
    Drop { player: Player, column: u8, row: u8 },
    /// `player`'s drop into `column`, landing on `row`, completes a line of
    /// four: the player wins.
    Win { player: Player, column: u8, row: u8 },
    /// Every column is full and no line of four is complete at `player`'s
    /// turn: the game is drawn.
    Draw(Player),
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

/// The compact 4×4 board.
pub fn initial() -> World {
    board(WIDTH, HEIGHT)
}

/// The standard 7×6 board.
pub fn initial_standard() -> World {
    board(STANDARD_WIDTH, STANDARD_HEIGHT)
}

/// The lines of four through a cell, sixteen of them, each named by its
/// rightmost column and top row: the `k`th column to the cell's right is
/// `right[k]`, and the `k`th row above it `up[k]`, the cell's own at zero.
fn lines_through(
    player: &Slot<Player>,
    right: &[Param<u8>; 4],
    up: &[Param<u8>; 4],
) -> Vec<Pattern<Asset>> {
    let line = |direction, column: &Param<u8>, row: &Param<u8>| -> Pattern<Asset> {
        AssetPattern::Line(player.clone(), Slot::Is(direction), column.var(), row.var()).into()
    };
    (0..4)
        .flat_map(|k| {
            [
                line(Direction::Across, &right[k], &up[0]),
                line(Direction::Up, &right[0], &up[k]),
                line(Direction::Rising, &right[k], &up[k]),
                line(Direction::Falling, &right[k], &up[3 - k]),
            ]
        })
        .collect()
}

/// `player`'s drop into `{column}`, landing on `{row}` of a board `width`
/// by `height`, each role fixed to its account: the column's height, which
/// must be the row, grows by one; the cell's emptiness becomes the piece;
/// the player's turn is spent; and each line of four through the cell, its
/// columns and rows derived from the cell's, counts the piece. `completes`
/// says whether one of those lines already holds three of the player's
/// pieces, so this drop completes it, or none does.
fn drop(
    player: Slot<Player>,
    (column, row): (&Param<u8>, &Param<u8>),
    (width, height): (u8, u8),
    completes: bool,
) -> Rate<Ontology> {
    let right = [
        column.clone(),
        Param::new(path!("right-1")),
        Param::new(path!("right-2")),
        Param::new(path!("right-3")),
    ];
    let up = [
        row.clone(),
        Param::new(path!("up-1")),
        Param::new(path!("up-2")),
        Param::new(path!("up-3")),
    ];
    let mut rate = Rate::new()
        .variable(column.range(0, width - 1))
        .variable(row.range(0, height - 1));
    for k in 1..4 {
        let offset = Q::constant(k as u64);
        rate = rate
            .derive(right[k].name().clone(), column.quantity() + offset.clone())
            .derive(up[k].name().clone(), row.quantity() + offset);
    }
    let lines = lines_through(&player, &right, &up);
    let most = lines
        .iter()
        .map(|line| Q::balance_at(Role::Game, line.clone()))
        .reduce(Q::maximum)
        .expect("sixteen lines");
    let one = |asset: Pattern<Asset>| (asset, Quantity::new(1));
    let (name, comparison, bound) = if completes {
        (path!("completes-a-line-of-four"), C::GreaterThanOrEqual, 3)
    } else {
        (path!("completes-no-line-of-four"), C::LessThanOrEqual, 2)
    };
    rate.account(Role::Game, Pattern::literal(AccountId::Game))
        .account(Role::Column, AccountIdPattern::Column(column.var()).into())
        .account(
            Role::Cell,
            AccountIdPattern::Cell {
                column: column.var(),
                row: row.var(),
            }
            .into(),
        )
        .condition(
            path!("lands-on-the-column-height"),
            Q::balance(Role::Column, Asset::Height),
            C::Equal,
            row.quantity(),
        )
        .condition(name, most, comparison, Q::constant(bound))
        .consume_at(Role::Game, [one(AssetPattern::Turn(player.clone()).into())])
        .produce_at(Role::Game, lines.into_iter().map(one))
        .produce(Role::Column, basket([(Asset::Height, 1)]))
        .consume(Role::Cell, basket([(Asset::Empty, 1)]))
        .produce_at(Role::Cell, [one(AssetPattern::Piece(player).into())])
}

/// A board `width` columns by `height` rows, empty, Red to drop first. A
/// drop is a rate over every column and row, the column's height a balance
/// a condition checks the row against; one completing a line of four wins,
/// and a full board with no line complete is drawn.
fn board(width: u8, height: u8) -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Game,
            Account::from(basket([(Asset::Turn(Player::Red), 1)])),
        )
        .account(AccountId::Result, Account::from(basket([])));
    for column in 0..width {
        builder = builder.account(AccountId::Column(column), Account::from(basket([])));
        for row in 0..height {
            builder = builder.account(
                AccountId::Cell { column, row },
                Account::from(basket([(Asset::Empty, 1)])),
            );
        }
    }

    let (column, row) = (Param::<u8>::new(path!("column")), Param::new(path!("row")));
    let player = Param::<Player>::new(path!("player"));
    let size = (width, height);
    for mover in [Player::Red, Player::Yellow] {
        builder = builder.rate_at(
            RateIdPattern::Drop {
                player: Slot::Is(mover),
                column: column.var(),
                row: row.var(),
            }
            .into(),
            drop(Slot::Is(mover), (&column, &row), size, false)
                .produce(Role::Game, basket([(Asset::Turn(mover.other()), 1)])),
        );
    }
    builder = builder.rate_at(
        RateIdPattern::Win {
            player: player.var(),
            column: column.var(),
            row: row.var(),
        }
        .into(),
        drop(player.var(), (&column, &row), size, true)
            .variable(player.every())
            .account(Role::Result, Pattern::literal(AccountId::Result))
            .produce_at(
                Role::Result,
                [(AssetPattern::Winner(player.var()).into(), Quantity::new(1))],
            ),
    );
    let mut draw = Rate::new()
        .variable(player.every())
        .account(Role::Game, Pattern::literal(AccountId::Game))
        .account(Role::Result, Pattern::literal(AccountId::Result))
        .consume_at(
            Role::Game,
            [(AssetPattern::Turn(player.var()).into(), Quantity::new(1))],
        )
        .produce(Role::Result, basket([(Asset::Draw, 1)]));
    for column in 0..width {
        draw = draw
            .account(
                Role::Full(column),
                Pattern::literal(AccountId::Column(column)),
            )
            .condition(
                path!("the-column-is-full").join(&path::write(&column)),
                Q::balance(Role::Full(column), Asset::Height),
                C::Equal,
                Q::constant(u64::from(height)),
            );
    }
    builder = builder.rate_at(RateIdPattern::Draw(player.var()).into(), draw);

    let players = [Player::Red, Player::Yellow];
    // Every piece counts in sixteen lines of four, some of them off the
    // board's edge.
    let mut lines = LinearInvariant::new(path!("line-counts"));
    for player in players {
        lines = lines.weight(Asset::Piece(player), -16);
        for direction in [
            Direction::Across,
            Direction::Up,
            Direction::Rising,
            Direction::Falling,
        ] {
            for column in 0..width + 3 {
                for row in 0..height + 3 {
                    lines = lines.weight(Asset::Line(player, direction, column, row), 1);
                }
            }
        }
    }
    builder
        .invariant(
            LinearInvariant::new(path!("cell-occupancy"))
                .weight(Asset::Empty, 1)
                .weight(Asset::Piece(Player::Red), 1)
                .weight(Asset::Piece(Player::Yellow), 1),
        )
        .invariant(
            LinearInvariant::new(path!("column-heights"))
                .weight(Asset::Height, 1)
                .weight(Asset::Piece(Player::Red), -1)
                .weight(Asset::Piece(Player::Yellow), -1),
        )
        .invariant(
            LinearInvariant::new(path!("game-phase"))
                .weight(Asset::Turn(Player::Red), 1)
                .weight(Asset::Turn(Player::Yellow), 1)
                .weight(Asset::Winner(Player::Red), 1)
                .weight(Asset::Winner(Player::Yellow), 1)
                .weight(Asset::Draw, 1),
        )
        .invariant(lines)
        .build()
        .expect("connect four model is valid")
}

/// The exchange of one unit of `rate`, each of its roles bound to the
/// account the rate fixes: the game, the column and cell a drop lands in,
/// the result a win or a draw records, and the columns a draw finds full,
/// on a board `width` columns wide.
pub fn exchange(rate: RateId, width: u8) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::Game, AccountId::Game);
    match rate {
        RateId::Drop { column, row, .. } => exchange
            .bind(Role::Column, AccountId::Column(column))
            .bind(Role::Cell, AccountId::Cell { column, row }),
        RateId::Win { column, row, .. } => exchange
            .bind(Role::Column, AccountId::Column(column))
            .bind(Role::Cell, AccountId::Cell { column, row })
            .bind(Role::Result, AccountId::Result),
        RateId::Draw(_) => (0..width).fold(
            exchange.bind(Role::Result, AccountId::Result),
            |exchange, column| exchange.bind(Role::Full(column), AccountId::Column(column)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `player`'s drop into `column`, landing in `row`, completing no line.
    fn drop(player: Player, column: u8, row: u8) -> Action {
        exchange(
            RateId::Drop {
                player,
                column,
                row,
            },
            WIDTH,
        )
    }

    /// `player`'s drop into `column`, landing in `row`, completing a line.
    fn win(player: Player, column: u8, row: u8, width: u8) -> Action {
        exchange(
            RateId::Win {
                player,
                column,
                row,
            },
            width,
        )
    }

    #[test]
    fn gravity_and_line_counts_are_core_validated() {
        let mut world = initial();
        world
            .apply(drop(Player::Red, 0, 0))
            .expect("first piece lands");

        assert_eq!(
            world.balance(
                &AccountId::Cell { column: 0, row: 0 },
                &Asset::Piece(Player::Red),
            ),
            Quantity::new(1)
        );
        assert_eq!(
            world.balance(&AccountId::Column(0), &Asset::Height),
            Quantity::new(1)
        );
        // The bottom row's line ending three columns right of the corner
        // counts the piece.
        assert_eq!(
            world.balance(
                &AccountId::Game,
                &Asset::Line(Player::Red, Direction::Across, 3, 0),
            ),
            Quantity::new(1)
        );
        // A piece cannot land above an empty cell, nor below a full one.
        assert!(!world.is_applicable(&drop(Player::Yellow, 1, 1)));
        assert!(!world.is_applicable(&drop(Player::Yellow, 0, 0)));
        assert!(world.is_applicable(&drop(Player::Yellow, 0, 1)));
    }

    #[test]
    fn fixed_accounts_reject_misbinding_an_otherwise_empty_cell() {
        let world = initial();
        let misplaced = Exchange::new(
            RateId::Drop {
                player: Player::Red,
                column: 0,
                row: 0,
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
        for (mut world, width) in [(initial(), WIDTH), (initial_standard(), STANDARD_WIDTH)] {
            // Red and Yellow alternate in columns 0, 0, 1, 1, 2 and 2; Red's
            // drop into column 3 completes the bottom row.
            for (player, column, row) in [
                (Player::Red, 0, 0),
                (Player::Yellow, 0, 1),
                (Player::Red, 1, 0),
                (Player::Yellow, 1, 1),
                (Player::Red, 2, 0),
                (Player::Yellow, 2, 1),
            ] {
                world
                    .apply(exchange(
                        RateId::Drop {
                            player,
                            column,
                            row,
                        },
                        width,
                    ))
                    .unwrap();
            }
            // The drop completing the row cannot pass the turn; it wins.
            let before = world.state_key();
            assert!(
                world
                    .apply(exchange(
                        RateId::Drop {
                            player: Player::Red,
                            column: 3,
                            row: 0,
                        },
                        width,
                    ))
                    .is_err()
            );
            assert_eq!(before, world.state_key());
            // Nor can a drop elsewhere claim the win.
            assert!(!world.is_applicable(&win(Player::Red, 3, 1, width)));
            world
                .apply(win(Player::Red, 3, 0, width))
                .expect("the winning drop applies");
            assert_eq!(
                world.balance(&AccountId::Result, &Asset::Winner(Player::Red)),
                Quantity::new(1)
            );
            assert!(
                world
                    .balance(&AccountId::Game, &Asset::Turn(Player::Yellow))
                    .is_zero()
            );
            assert!(!world.is_applicable(&exchange(
                RateId::Drop {
                    player: Player::Yellow,
                    column: 3,
                    row: 1,
                },
                width,
            )));
        }
    }

    #[test]
    fn a_full_board_without_a_line_is_drawn() {
        let mut world = initial();
        // Columns filled in pairs, Red first, so no four line up:
        // rows alternate R R Y Y across each pair of columns.
        let order = [
            (0, 0),
            (1, 0),
            (0, 1),
            (1, 1),
            (1, 2),
            (0, 2),
            (1, 3),
            (0, 3),
            (2, 0),
            (3, 0),
            (2, 1),
            (3, 1),
            (3, 2),
            (2, 2),
            (3, 3),
            (2, 3),
        ];
        let mut player = Player::Red;
        let draw = exchange(RateId::Draw(Player::Red), WIDTH);
        for (column, row) in order {
            assert!(!world.is_applicable(&draw));
            world.apply(drop(player, column, row)).unwrap();
            player = player.other();
        }
        world.apply(draw).expect("the full board is drawn");
        assert_eq!(
            world.balance(&AccountId::Result, &Asset::Draw),
            Quantity::new(1)
        );
    }

    #[test]
    fn the_compact_board_is_a_handful_of_rates() {
        for world in [initial(), initial_standard()] {
            assert_eq!(world.rate_ids().count(), 4);
        }
    }
}
