use axionomy::{Account, Economy, EconomyBuilder, Exchange, Goal, Quantity, Rate, basket};
use axionomy_search::session::{Continue, SearchStatus, WorkBudget};
use axionomy_search::{AStarSession, BfsSession, GraphSearchProgress, astar, bfs};
use std::ops::ControlFlow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Actor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Step(u8),
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Advance(u8),
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Actor,
}

type World = Economy<AccountId, Asset, RateId, Role>;
type Action = Exchange<RateId, Role, AccountId>;

fn world() -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Actor,
            Account::from(basket([(Asset::Step(0), 1)])),
        )
        .rate(
            RateId::Advance(0),
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Step(0), 1)]))
                .produce(Role::Actor, basket([(Asset::Step(1), 1)])),
        )
        .rate(
            RateId::Advance(1),
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Step(1), 1)]))
                .produce(Role::Actor, basket([(Asset::Step(2), 1)])),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Step(2), 1)]))
                .produce(Role::Actor, basket([(Asset::Solved, 1)])),
        )
        .build()
        .unwrap()
}

fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Actor, basket([(Asset::Solved, 1)]))
}

fn candidates(world: &World) -> Vec<Action> {
    world
        .rate_ids()
        .copied()
        .map(|rate| Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Actor))
        .collect()
}

fn remaining(world: &World) -> u64 {
    if !world.balance(&AccountId::Actor, &Asset::Step(0)).is_zero() {
        3
    } else if !world.balance(&AccountId::Actor, &Asset::Step(1)).is_zero() {
        2
    } else if !world.balance(&AccountId::Actor, &Asset::Step(2)).is_zero() {
        1
    } else {
        0
    }
}

#[test]
fn chunking_does_not_change_breadth_first_results() {
    let initial = world();
    let expected = bfs(&initial, &goal(), candidates).unwrap();
    let mut session = BfsSession::new(&initial, goal(), candidates);
    let mut observer = Continue;

    assert_eq!(
        session.advance(WorkBudget::new(1), &mut observer).status(),
        SearchStatus::Running
    );
    assert_eq!(
        session.advance(WorkBudget::new(1), &mut observer).status(),
        SearchStatus::Running
    );
    assert_eq!(
        session.advance(WorkBudget::new(1), &mut observer).status(),
        SearchStatus::Solved
    );

    let solution = session.into_solution().unwrap();
    assert_eq!(solution.cost(), expected.cost());
    assert_eq!(solution.trace(), expected.trace());
    assert!(initial.replayed(solution.trace()).unwrap().matches(&goal()));
}

#[test]
fn interruption_is_resumable_and_does_not_mutate_the_source() {
    let initial = world();
    let before = initial.state_key();
    let mut session = BfsSession::new(&initial, goal(), candidates);
    let mut calls = 0;
    let report = session.advance(WorkBudget::new(10), &mut |_: &GraphSearchProgress| {
        calls += 1;
        if calls == 2 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });

    assert_eq!(report.status(), SearchStatus::Interrupted);
    assert_eq!(report.work_completed(), 1);
    assert_eq!(initial.state_key(), before);

    let mut observer = Continue;
    let report = session.advance(WorkBudget::new(10), &mut observer);
    assert_eq!(report.status(), SearchStatus::Solved);
    assert!(
        initial
            .replayed(session.solution().unwrap().trace())
            .unwrap()
            .matches(&goal())
    );
}

#[test]
fn chunking_does_not_change_a_star_results() {
    let initial = world();
    let expected = astar(&initial, &goal(), candidates, |_, _, _| 1, remaining).unwrap();
    let mut session = AStarSession::new(&initial, goal(), candidates, |_, _, _| 1, remaining);
    let mut observer = Continue;
    while !session.status().is_terminal() {
        session.advance(WorkBudget::new(1), &mut observer);
    }

    let solution = session.into_solution().unwrap();
    assert_eq!(solution.cost(), expected.cost());
    assert_eq!(solution.trace(), expected.trace());
    assert!(initial.replayed(solution.trace()).unwrap().matches(&goal()));
}
