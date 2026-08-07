//! A tiny Sokoban board encoded entirely as cell accounts and rewrite rates.

use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    basket,
};
use axionomy_search::{SearchSolution, bfs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Cell(u8),
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    CellIdentity(u8),
    Player,
    Crate,
    Empty,
    GoalCell,
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    From,
    Middle,
    To,
    Puzzle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Move { from: u8, to: u8 },
    Push { behind: u8, crate_at: u8, to: u8 },
    Finish { cell: u8 },
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;

pub fn initial() -> World {
    build_board(5, 1, 0, 1, 3)
}

pub fn deadlocked() -> World {
    build_board(5, 1, 0, 4, 3)
}

/// An open two-dimensional board that requires walking around the crate to
/// change push direction; many legal moves do not advance the objective.
pub fn initial_showcase() -> World {
    build_board(7, 5, cell(7, 0, 0), cell(7, 2, 2), cell(7, 5, 3))
}

pub fn deadlocked_showcase() -> World {
    build_board(7, 5, cell(7, 2, 2), cell(7, 0, 0), cell(7, 5, 3))
}

pub fn initial_stress() -> World {
    build_board(8, 6, cell(8, 0, 0), cell(8, 3, 2), cell(8, 6, 4))
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut rate_ids: Vec<_> = world.rate_ids().copied().collect();
    rate_ids.sort();
    world.applicable(rate_ids.into_iter().map(action))
}

pub fn solve(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

fn build_board(width: u8, height: u8, player: u8, crate_at: u8, goal_cell: u8) -> World {
    let cell_count = width.checked_mul(height).expect("small encoded board");
    let mut builder = EconomyBuilder::new().account(
        AccountId::Success,
        Account::from(basket([(Asset::Active, 1)])),
    );
    for cell in 0..cell_count {
        let occupant = if cell == player {
            Asset::Player
        } else if cell == crate_at {
            Asset::Crate
        } else {
            Asset::Empty
        };
        let mut assets = Basket::from([
            (Asset::CellIdentity(cell), Quantity::new(1)),
            (occupant, Quantity::new(1)),
        ]);
        if cell == goal_cell {
            assets.insert(Asset::GoalCell, Quantity::new(1));
        }
        builder = builder.account(AccountId::Cell(cell), Account::from(assets));
    }

    for from in 0..cell_count {
        for to in neighbors(from, width, height) {
            builder = builder.rate(
                RateId::Move { from, to },
                Rate::new()
                    .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                    .preserve(Role::From, basket([(Asset::CellIdentity(from), 1)]))
                    .preserve(Role::To, basket([(Asset::CellIdentity(to), 1)]))
                    .consume(Role::From, basket([(Asset::Player, 1)]))
                    .consume(Role::To, basket([(Asset::Empty, 1)]))
                    .produce(Role::From, basket([(Asset::Empty, 1)]))
                    .produce(Role::To, basket([(Asset::Player, 1)]))
                    .distinct(Role::From, Role::To),
            );
        }
    }
    for (behind, middle, to) in push_lines(width, height) {
        builder = builder.rate(
            RateId::Push {
                behind,
                crate_at: middle,
                to,
            },
            Rate::new()
                .preserve(Role::Puzzle, basket([(Asset::Active, 1)]))
                .preserve(Role::From, basket([(Asset::CellIdentity(behind), 1)]))
                .preserve(Role::Middle, basket([(Asset::CellIdentity(middle), 1)]))
                .preserve(Role::To, basket([(Asset::CellIdentity(to), 1)]))
                .consume(Role::From, basket([(Asset::Player, 1)]))
                .consume(Role::Middle, basket([(Asset::Crate, 1)]))
                .consume(Role::To, basket([(Asset::Empty, 1)]))
                .produce(Role::From, basket([(Asset::Empty, 1)]))
                .produce(Role::Middle, basket([(Asset::Player, 1)]))
                .produce(Role::To, basket([(Asset::Crate, 1)]))
                .distinct(Role::From, Role::Middle)
                .distinct(Role::Middle, Role::To)
                .distinct(Role::From, Role::To),
        );
    }
    for cell in 0..cell_count {
        builder = builder.rate(
            RateId::Finish { cell },
            Rate::new()
                .preserve(
                    Role::Middle,
                    basket([
                        (Asset::CellIdentity(cell), 1),
                        (Asset::Crate, 1),
                        (Asset::GoalCell, 1),
                    ]),
                )
                .consume(Role::Puzzle, basket([(Asset::Active, 1)]))
                .produce(Role::Puzzle, basket([(Asset::Solved, 1)]))
                .distinct(Role::Middle, Role::Puzzle),
        );
    }

    builder
        .invariant(LinearInvariant::new("one player").weight(Asset::Player, 1))
        .invariant(LinearInvariant::new("one crate").weight(Asset::Crate, 1))
        .invariant(
            LinearInvariant::new("cell occupancy")
                .weight(Asset::Player, 1)
                .weight(Asset::Crate, 1)
                .weight(Asset::Empty, 1),
        )
        .invariant(
            LinearInvariant::new("puzzle lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("sokoban model is valid")
}

fn neighbors(index: u8, width: u8, height: u8) -> Vec<u8> {
    let x = index % width;
    let y = index / width;
    [
        x.checked_sub(1).map(|next| cell(width, next, y)),
        (x + 1 < width).then(|| cell(width, x + 1, y)),
        y.checked_sub(1).map(|next| cell(width, x, next)),
        (y + 1 < height).then(|| cell(width, x, y + 1)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn push_lines(width: u8, height: u8) -> Vec<(u8, u8, u8)> {
    let mut lines = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if x + 2 < width {
                lines.push((
                    cell(width, x, y),
                    cell(width, x + 1, y),
                    cell(width, x + 2, y),
                ));
                lines.push((
                    cell(width, x + 2, y),
                    cell(width, x + 1, y),
                    cell(width, x, y),
                ));
            }
            if y + 2 < height {
                lines.push((
                    cell(width, x, y),
                    cell(width, x, y + 1),
                    cell(width, x, y + 2),
                ));
                lines.push((
                    cell(width, x, y + 2),
                    cell(width, x, y + 1),
                    cell(width, x, y),
                ));
            }
        }
    }
    lines
}

const fn cell(width: u8, x: u8, y: u8) -> u8 {
    y * width + x
}

pub fn dimensions(world: &World) -> (u8, u8) {
    if !world
        .balance(&AccountId::Cell(47), &Asset::CellIdentity(47))
        .is_zero()
    {
        (8, 6)
    } else if !world
        .balance(&AccountId::Cell(34), &Asset::CellIdentity(34))
        .is_zero()
    {
        (7, 5)
    } else {
        (5, 1)
    }
}

fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::Puzzle, AccountId::Success);
    match rate {
        RateId::Move { from, to } => exchange
            .bind(Role::From, AccountId::Cell(from))
            .bind(Role::To, AccountId::Cell(to)),
        RateId::Push {
            behind,
            crate_at,
            to,
        } => exchange
            .bind(Role::From, AccountId::Cell(behind))
            .bind(Role::Middle, AccountId::Cell(crate_at))
            .bind(Role::To, AccountId::Cell(to)),
        RateId::Finish { cell } => exchange.bind(Role::Middle, AccountId::Cell(cell)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_by_atomic_three_account_pushes() {
        let solution = solve(&initial()).expect("puzzle is solvable");
        assert_eq!(solution.trace().exchanges().len(), 3);
        let mut replay = initial();
        replay.replay(solution.trace()).expect("trace must replay");
        assert!(replay.matches(&goal()));
        for cell in 0..5 {
            let occupancy = [Asset::Player, Asset::Crate, Asset::Empty]
                .into_iter()
                .map(|asset| replay.balance(&AccountId::Cell(cell), &asset).get())
                .sum::<u64>();
            assert_eq!(occupancy, 1);
        }
    }

    #[test]
    fn reports_deadlock_without_mutating_the_world() {
        let world = deadlocked();
        let before = world.state_key();
        assert!(solve(&world).is_none());
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn encoded_cell_identities_reject_teleports() {
        let world = initial();
        let teleport = Exchange::new(RateId::Move { from: 0, to: 1 }, Quantity::new(1))
            .bind(Role::From, AccountId::Cell(0))
            .bind(Role::To, AccountId::Cell(4));

        assert!(!world.is_applicable(&teleport));
    }

    #[test]
    fn solved_puzzle_is_quiescent() {
        let solution = solve(&initial()).expect("puzzle is solvable");
        let solved = initial()
            .replayed(solution.trace())
            .expect("solution must replay");

        assert!(candidates(&solved).is_empty());
    }
}
