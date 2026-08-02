//! Exact cover solved both by generic graph search and an Algorithm X proposer.

use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    Trace, basket,
};
use axionomy_search::{SearchSolution, bfs};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Element {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SetId {
    Ab,
    Cd,
    Ac,
    Bd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Uncovered(Element),
    Covered(Element),
    Available(SetId),
    Selected(SetId),
    Progress(u8),
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Select { set: SetId, before: u8 },
    Finish,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;
pub type Proposal = Trace<RateId, Role, AccountId>;

const SETS: [SetId; 4] = [SetId::Ab, SetId::Cd, SetId::Ac, SetId::Bd];
const ELEMENTS: [Element; 4] = [Element::A, Element::B, Element::C, Element::D];

pub fn initial() -> World {
    build(&SETS)
}

pub fn unsatisfiable() -> World {
    build(&[SetId::Ab, SetId::Ac])
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Problem, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}

pub fn solve_bfs(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

/// Runs Algorithm X over memberships read back from the economy's rates.
///
/// It is deliberately only a proposer: the resulting trace still has to pass
/// `Economy::replay`.
pub fn algorithm_x(world: &World) -> Option<Proposal> {
    let remaining = ELEMENTS.into_iter().collect();
    let available = SETS
        .into_iter()
        .filter(|set| {
            !world
                .balance(&AccountId::Problem, &Asset::Available(*set))
                .is_zero()
        })
        .collect();
    let selected = cover(world, remaining, available)?;
    let mut trace = Trace::new();
    for (index, set) in selected.into_iter().enumerate() {
        let before = u8::try_from(index * 2).ok()?;
        trace.push(action(RateId::Select { set, before }));
    }
    trace.push(action(RateId::Finish));

    let mut validation = world.clone();
    validation.replay(&trace).ok()?;
    validation.matches(&goal()).then_some(trace)
}

fn build(available: &[SetId]) -> World {
    let mut problem = Basket::new();
    for element in ELEMENTS {
        problem.insert(Asset::Uncovered(element), Quantity::new(1));
    }
    for set in available {
        problem.insert(Asset::Available(*set), Quantity::new(1));
    }
    problem.insert(Asset::Progress(0), Quantity::new(1));
    problem.insert(Asset::Active, Quantity::new(1));

    let mut builder = EconomyBuilder::new().account(AccountId::Problem, Account::from(problem));

    for set in SETS {
        for before in [0, 2] {
            let mut consume = Basket::from([
                (Asset::Available(set), Quantity::new(1)),
                (Asset::Progress(before), Quantity::new(1)),
            ]);
            let mut produce = Basket::from([
                (Asset::Selected(set), Quantity::new(1)),
                (Asset::Progress(before + 2), Quantity::new(1)),
            ]);
            for element in declared_members(set) {
                consume.insert(Asset::Uncovered(element), Quantity::new(1));
                produce.insert(Asset::Covered(element), Quantity::new(1));
            }
            builder = builder.rate(
                RateId::Select { set, before },
                Rate::new()
                    .preserve(Role::Problem, basket([(Asset::Active, 1)]))
                    .consume(Role::Problem, consume)
                    .produce(Role::Problem, produce),
            );
        }
    }

    let element_invariant = ELEMENTS.into_iter().fold(
        LinearInvariant::new("each universe element persists"),
        |invariant, element| {
            invariant
                .weight(Asset::Uncovered(element), 1)
                .weight(Asset::Covered(element), 1)
        },
    );
    let set_invariant = SETS.into_iter().fold(
        LinearInvariant::new("set choice is single use"),
        |invariant, set| {
            invariant
                .weight(Asset::Available(set), 1)
                .weight(Asset::Selected(set), 1)
        },
    );
    let progress_invariant = [0, 2, 4].into_iter().fold(
        LinearInvariant::new("one progress token"),
        |invariant, progress| invariant.weight(Asset::Progress(progress), 1),
    );

    builder
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(
                    Role::Problem,
                    basket([
                        (Asset::Progress(4), 1),
                        (Asset::Covered(Element::A), 1),
                        (Asset::Covered(Element::B), 1),
                        (Asset::Covered(Element::C), 1),
                        (Asset::Covered(Element::D), 1),
                    ]),
                )
                .consume(Role::Problem, basket([(Asset::Active, 1)]))
                .produce(Role::Problem, basket([(Asset::Solved, 1)])),
        )
        .invariant(element_invariant)
        .invariant(set_invariant)
        .invariant(progress_invariant)
        .invariant(
            LinearInvariant::new("exact-cover lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("exact-cover model is valid")
}

fn declared_members(set: SetId) -> [Element; 2] {
    match set {
        SetId::Ab => [Element::A, Element::B],
        SetId::Cd => [Element::C, Element::D],
        SetId::Ac => [Element::A, Element::C],
        SetId::Bd => [Element::B, Element::D],
    }
}

fn encoded_members(world: &World, set: SetId) -> BTreeSet<Element> {
    world
        .rate(&RateId::Select { set, before: 0 })
        .and_then(|rate| rate.consumed(&Role::Problem))
        .into_iter()
        .flat_map(Basket::iter)
        .filter_map(|(asset, _)| match asset {
            Asset::Uncovered(element) => Some(*element),
            _ => None,
        })
        .collect()
}

fn cover(
    world: &World,
    remaining: BTreeSet<Element>,
    available: BTreeSet<SetId>,
) -> Option<Vec<SetId>> {
    if remaining.is_empty() {
        return Some(Vec::new());
    }
    let pivot = remaining
        .iter()
        .min_by_key(|element| {
            available
                .iter()
                .filter(|set| encoded_members(world, **set).contains(element))
                .count()
        })
        .copied()?;

    for set in available.iter().copied() {
        let members = encoded_members(world, set);
        if !members.contains(&pivot) || !members.is_subset(&remaining) {
            continue;
        }
        let next_remaining = remaining.difference(&members).copied().collect();
        let next_available = available
            .iter()
            .copied()
            .filter(|candidate| encoded_members(world, *candidate).is_disjoint(&members))
            .collect();
        if let Some(mut suffix) = cover(world, next_remaining, next_available) {
            suffix.insert(0, set);
            return Some(suffix);
        }
    }
    None
}

fn action(rate: RateId) -> Action {
    Exchange::new(rate, Quantity::new(1)).bind(Role::Problem, AccountId::Problem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_x_and_bfs_emit_core_validated_solutions() {
        let world = initial();
        let bfs = solve_bfs(&world).expect("an exact cover exists");
        let algorithm_x = algorithm_x(&world).expect("Algorithm X finds a cover");
        assert_eq!(bfs.trace().exchanges().len(), algorithm_x.exchanges().len());

        for trace in [bfs.trace(), &algorithm_x] {
            let mut replay = initial();
            replay.replay(trace).expect("proposal must replay");
            assert!(replay.matches(&goal()));
            assert!(candidates(&replay).is_empty());
        }
    }

    #[test]
    fn both_solvers_agree_on_infeasibility() {
        let world = unsatisfiable();
        assert!(solve_bfs(&world).is_none());
        assert!(algorithm_x(&world).is_none());
    }
}
