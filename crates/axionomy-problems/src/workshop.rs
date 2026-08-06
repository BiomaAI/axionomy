//! A stoichiometric workshop with conserved material and labor accounting.

use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    basket,
};
use axionomy_search::{
    SearchSolution, best_first, bfs,
    pareto::{self, Objective, ObjectiveVector, ParetoError, ParetoSearchResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Workshop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Wood,
    Labor,
    SpentLabor,
    Tool,
    Chair,
    Waste,
    Time,
    SpentTime,
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Shop,
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
pub type ParetoResult = ParetoSearchResult<RateId, Role, AccountId, u64, ObjectiveKey, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveKey {
    Waste,
    Time,
}

pub fn initial() -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Workshop,
            Account::from(basket([
                (Asset::Wood, 6),
                (Asset::Labor, 4),
                (Asset::Tool, 1),
                (Asset::Time, 3),
                (Asset::Active, 1),
            ])),
        )
        .rate(
            RateId::BasicChair,
            Rate::new()
                .preserve(Role::Shop, basket([(Asset::Active, 1)]))
                .consume(
                    Role::Shop,
                    basket([(Asset::Wood, 2), (Asset::Labor, 1), (Asset::Time, 1)]),
                )
                .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
                .produce(
                    Role::Shop,
                    basket([
                        (Asset::Chair, 1),
                        (Asset::Waste, 1),
                        (Asset::SpentLabor, 1),
                        (Asset::SpentTime, 1),
                    ]),
                ),
        )
        .rate(
            RateId::EfficientBatch,
            Rate::new()
                .preserve(Role::Shop, basket([(Asset::Active, 1)]))
                .consume(
                    Role::Shop,
                    basket([(Asset::Wood, 3), (Asset::Labor, 2), (Asset::Time, 3)]),
                )
                .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
                .produce(
                    Role::Shop,
                    basket([
                        (Asset::Chair, 2),
                        (Asset::Waste, 1),
                        (Asset::SpentLabor, 2),
                        (Asset::SpentTime, 3),
                    ]),
                ),
        )
        // This deliberately malformed domain proposal is installed to prove
        // that declared invariants, rather than solver discipline, are final.
        .rate(
            RateId::CounterfeitChair,
            Rate::new()
                .preserve(Role::Shop, basket([(Asset::Active, 1)]))
                .consume(Role::Shop, basket([(Asset::Wood, 1), (Asset::Time, 1)]))
                .produce(
                    Role::Shop,
                    basket([(Asset::Chair, 2), (Asset::SpentTime, 1)]),
                ),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Shop, basket([(Asset::Chair, 2)]))
                .consume(Role::Shop, basket([(Asset::Active, 1)]))
                .produce(Role::Shop, basket([(Asset::Solved, 1)])),
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
        .invariant(
            LinearInvariant::new("time accounting")
                .weight(Asset::Time, 1)
                .weight(Asset::SpentTime, 1),
        )
        .invariant(
            LinearInvariant::new("workshop lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("workshop model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Workshop, basket([(Asset::Solved, 1)]))
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

/// Exhaustively exposes the valid waste/time recipe tradeoffs.
pub fn pareto_front(world: &World) -> Result<ParetoResult, ParetoError> {
    pareto::search(world, &goal(), candidates, objectives)
}

pub fn objectives(world: &World) -> ObjectiveVector<ObjectiveKey, u64> {
    ObjectiveVector::try_new([
        Objective::minimize(ObjectiveKey::Waste, waste(world)),
        Objective::minimize(ObjectiveKey::Time, spent_time(world)),
    ])
    .expect("workshop objective schema is static and unique")
}

pub fn waste(world: &World) -> u64 {
    world.balance(&AccountId::Workshop, &Asset::Waste).get()
}

pub fn spent_time(world: &World) -> u64 {
    world.balance(&AccountId::Workshop, &Asset::SpentTime).get()
}

pub fn action(rate: RateId) -> Action {
    Exchange::new(rate, Quantity::new(1)).bind(Role::Shop, AccountId::Workshop)
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
        assert!(candidates(&replay).is_empty());
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

    #[test]
    fn pareto_front_retains_fast_and_low_waste_recipes() {
        let initial = initial();
        let result = pareto_front(&initial).unwrap();
        let mut outcomes = Vec::new();

        for entry in result.front().entries() {
            let replayed = initial.replayed(entry.payload()).unwrap();
            assert!(replayed.matches(&goal()));
            assert_eq!(&objectives(&replayed), entry.objectives());
            outcomes.push((waste(&replayed), spent_time(&replayed)));
        }

        outcomes.sort_unstable();
        assert_eq!(outcomes, [(1, 3), (2, 2)]);
    }
}
