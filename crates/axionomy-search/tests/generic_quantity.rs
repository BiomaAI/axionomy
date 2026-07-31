#![cfg(feature = "bigint")]

use axionomy::{Account, Basket, Economy, EconomyBuilder, Exchange, Goal, Quantity, Rate};
use axionomy_search::bfs;
use num_bigint::BigUint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Ready,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Solve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Agent,
}

type World = Economy<AccountId, Asset, RateId, Role, BigUint>;
type Action = Exchange<RateId, Role, AccountId, BigUint>;

fn quantity(value: u64) -> Quantity<BigUint> {
    Quantity::try_from_scalar(BigUint::from(value)).unwrap()
}

#[test]
fn implicit_search_accepts_non_copy_economic_quantity_backends() {
    let world: World = EconomyBuilder::new()
        .account(
            AccountId::Agent,
            Account::from(Basket::from([(Asset::Ready, quantity(1))])),
        )
        .rate(
            RateId::Solve,
            Rate::new()
                .consume(Role::Agent, Basket::from([(Asset::Ready, quantity(1))]))
                .produce(Role::Agent, Basket::from([(Asset::Solved, quantity(1))])),
        )
        .build()
        .unwrap();
    let goal = Goal::new().require(
        AccountId::Agent,
        Basket::from([(Asset::Solved, quantity(1))]),
    );
    let candidates = |world: &World| {
        world.applicable([
            Action::new(RateId::Solve, quantity(1)).bind(Role::Agent, AccountId::Agent)
        ])
    };

    let solution = bfs(&world, &goal, candidates).expect("one valid successor solves the goal");
    assert_eq!(solution.cost(), 1);
    assert!(world.replayed(solution.trace()).unwrap().matches(&goal));
}
