//! A stoichiometric workshop with conserved material and labor accounting.

use crate::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    SearchSolution, basket, best_first, bfs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Workshop,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Wood,
    Labor,
    SpentLabor,
    Tool,
    Chair,
    Waste,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Shop,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    BasicChair,
    EfficientBatch,
    CounterfeitChair,
    Finish,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;
pub type Failure = ApplyError<RateId, Role, AccountId, Asset>;

pub fn initial() -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Workshop,
            Account::from(basket([
                (Asset::Wood, 6),
                (Asset::Labor, 4),
                (Asset::Tool, 1),
            ])),
        )
        .account(AccountId::Success, Account::default())
        .rate(
            RateId::BasicChair,
            Rate::new()
                .consume(Role::Shop, basket([(Asset::Wood, 2), (Asset::Labor, 1)]))
                .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
                .produce(
                    Role::Shop,
                    basket([(Asset::Chair, 1), (Asset::Waste, 1), (Asset::SpentLabor, 1)]),
                ),
        )
        .rate(
            RateId::EfficientBatch,
            Rate::new()
                .consume(Role::Shop, basket([(Asset::Wood, 3), (Asset::Labor, 2)]))
                .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
                .produce(
                    Role::Shop,
                    basket([(Asset::Chair, 2), (Asset::Waste, 1), (Asset::SpentLabor, 2)]),
                ),
        )
        // This deliberately malformed domain proposal is installed to prove
        // that declared invariants, rather than solver discipline, are final.
        .rate(
            RateId::CounterfeitChair,
            Rate::new()
                .consume(Role::Shop, basket([(Asset::Wood, 1)]))
                .produce(Role::Shop, basket([(Asset::Chair, 2)])),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Shop, basket([(Asset::Chair, 2)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::Shop, Role::Goal),
        )
        .invariant(
            LinearInvariant::new("material mass")
                .weight(Asset::Wood, 1)
                .weight(Asset::Chair, 1)
                .weight(Asset::Waste, 1),
        )
        .invariant(
            LinearInvariant::new("labor accounting")
                .weight(Asset::Labor, 1)
                .weight(Asset::SpentLabor, 1),
        )
        .build()
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}

pub fn solve_bfs(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

pub fn minimize_waste(world: &World) -> Option<Solution> {
    best_first(world, &goal(), candidates, waste, |_| 0)
}

pub fn waste(world: &World) -> u64 {
    world.balance(&AccountId::Workshop, &Asset::Waste).get()
}

pub fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::Shop, AccountId::Workshop);
    if rate == RateId::Finish {
        exchange.bind(Role::Goal, AccountId::Success)
    } else {
        exchange
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_uses_the_more_efficient_recipe() {
        let world = initial();
        let bfs = solve_bfs(&world).expect("workshop can make two chairs");
        let optimized = minimize_waste(&world).expect("workshop can make two chairs");
        assert_eq!(optimized.cost(), 1);
        assert_eq!(bfs.trace().exchanges().len(), 2);

        let mut replay = initial();
        replay
            .replay(optimized.trace())
            .expect("optimized proposal must replay");
        assert!(replay.matches(&goal()));
    }

    #[test]
    fn invariant_rejects_a_stoichiometrically_invalid_rate_atomically() {
        let mut world = initial();
        let before = world.state_key();
        let error = world
            .apply(action(RateId::CounterfeitChair))
            .expect_err("mass creation must be rejected");
        assert!(matches!(
            error,
            ApplyError::InvariantViolation {
                invariant,
                before: 6,
                after: 7,
            } if invariant == "material mass"
        ));
        assert_eq!(world.state_key(), before);
    }
}
