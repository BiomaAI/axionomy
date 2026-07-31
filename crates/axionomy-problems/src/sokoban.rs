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
    Player,
    Crate,
    Empty,
    GoalCell,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    From,
    Middle,
    To,
    Goal,
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
    build(0, 1, 3)
}

pub fn deadlocked() -> World {
    build(0, 4, 3)
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

fn build(player: u8, crate_at: u8, goal_cell: u8) -> World {
    let mut builder = EconomyBuilder::new().account(AccountId::Success, Account::default());
    for cell in 0..5 {
        let occupant = if cell == player {
            Asset::Player
        } else if cell == crate_at {
            Asset::Crate
        } else {
            Asset::Empty
        };
        let mut assets = Basket::from([(occupant, Quantity::new(1))]);
        if cell == goal_cell {
            assets.insert(Asset::GoalCell, Quantity::new(1));
        }
        builder = builder.account(AccountId::Cell(cell), Account::from(assets));
    }

    for from in 0..5 {
        for to in neighbors(from) {
            builder = builder.rate(
                RateId::Move { from, to },
                Rate::new()
                    .consume(Role::From, basket([(Asset::Player, 1)]))
                    .consume(Role::To, basket([(Asset::Empty, 1)]))
                    .produce(Role::From, basket([(Asset::Empty, 1)]))
                    .produce(Role::To, basket([(Asset::Player, 1)]))
                    .distinct(Role::From, Role::To),
            );
        }
    }
    for (behind, middle, to) in [
        (0, 1, 2),
        (1, 2, 3),
        (2, 3, 4),
        (4, 3, 2),
        (3, 2, 1),
        (2, 1, 0),
    ] {
        builder = builder.rate(
            RateId::Push {
                behind,
                crate_at: middle,
                to,
            },
            Rate::new()
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
    for cell in 0..5 {
        builder = builder.rate(
            RateId::Finish { cell },
            Rate::new()
                .preserve(
                    Role::Middle,
                    basket([(Asset::Crate, 1), (Asset::GoalCell, 1)]),
                )
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::Middle, Role::Goal),
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
        .build()
}

fn neighbors(cell: u8) -> impl Iterator<Item = u8> {
    [
        cell.checked_sub(1),
        cell.checked_add(1).filter(|next| *next < 5),
    ]
    .into_iter()
    .flatten()
}

fn action(rate: RateId) -> Action {
    match rate {
        RateId::Move { from, to } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::From, AccountId::Cell(from))
            .bind(Role::To, AccountId::Cell(to)),
        RateId::Push {
            behind,
            crate_at,
            to,
        } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::From, AccountId::Cell(behind))
            .bind(Role::Middle, AccountId::Cell(crate_at))
            .bind(Role::To, AccountId::Cell(to)),
        RateId::Finish { cell } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::Middle, AccountId::Cell(cell))
            .bind(Role::Goal, AccountId::Success),
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
}
