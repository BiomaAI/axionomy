#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Quantity};

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
    crate::fixtures::initial(axionomy::path!("connect-four/micro"))
}

/// The standard 7×6 board.
pub fn initial_standard() -> World {
    crate::fixtures::initial(axionomy::path!("connect-four/showcase"))
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
