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
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SetId {
    Ab,
    Cd,
    Ac,
    Bd,
    Ef,
    Gh,
    Eg,
    Fh,
    Ad,
    Bc,
    Eh,
    Fg,
    Ij,
    Kl,
    Ik,
    Jl,
    Il,
    Jk,
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

const MICRO_SETS: [SetId; 4] = [SetId::Ab, SetId::Cd, SetId::Ac, SetId::Bd];
const MICRO_ELEMENTS: [Element; 4] = [Element::A, Element::B, Element::C, Element::D];
const SHOWCASE_SETS: [SetId; 12] = [
    SetId::Ab,
    SetId::Cd,
    SetId::Ef,
    SetId::Gh,
    SetId::Ac,
    SetId::Bd,
    SetId::Eg,
    SetId::Fh,
    SetId::Ad,
    SetId::Bc,
    SetId::Eh,
    SetId::Fg,
];
const SHOWCASE_ELEMENTS: [Element; 8] = [
    Element::A,
    Element::B,
    Element::C,
    Element::D,
    Element::E,
    Element::F,
    Element::G,
    Element::H,
];
const STRESS_SETS: [SetId; 18] = [
    SetId::Ab,
    SetId::Cd,
    SetId::Ef,
    SetId::Gh,
    SetId::Ij,
    SetId::Kl,
    SetId::Ac,
    SetId::Bd,
    SetId::Eg,
    SetId::Fh,
    SetId::Ik,
    SetId::Jl,
    SetId::Ad,
    SetId::Bc,
    SetId::Eh,
    SetId::Fg,
    SetId::Il,
    SetId::Jk,
];
const STRESS_ELEMENTS: [Element; 12] = [
    Element::A,
    Element::B,
    Element::C,
    Element::D,
    Element::E,
    Element::F,
    Element::G,
    Element::H,
    Element::I,
    Element::J,
    Element::K,
    Element::L,
];

pub fn initial() -> World {
    build(&MICRO_ELEMENTS, &MICRO_SETS)
}

pub fn unsatisfiable() -> World {
    build(&MICRO_ELEMENTS, &[SetId::Ab, SetId::Ac])
}

/// An eight-element incidence matrix with several competing exact covers.
pub fn initial_showcase() -> World {
    build(&SHOWCASE_ELEMENTS, &SHOWCASE_SETS)
}

/// A twelve-element matrix whose three independent incidence blocks create
/// substantially more partial covers for generic search to distinguish.
pub fn initial_stress() -> World {
    build(&STRESS_ELEMENTS, &STRESS_SETS)
}

pub fn unsatisfiable_showcase() -> World {
    build(
        &SHOWCASE_ELEMENTS,
        &[
            SetId::Ab,
            SetId::Cd,
            SetId::Ef,
            SetId::Ac,
            SetId::Bd,
            SetId::Eg,
            SetId::Ad,
            SetId::Bc,
        ],
    )
}

pub fn unsatisfiable_stress() -> World {
    build(
        &STRESS_ELEMENTS,
        &[
            SetId::Ab,
            SetId::Cd,
            SetId::Ef,
            SetId::Gh,
            SetId::Ac,
            SetId::Bd,
            SetId::Eg,
            SetId::Fh,
            SetId::Ik,
            SetId::Il,
        ],
    )
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
    let remaining = elements(world).into_iter().collect();
    let available = sets(world)
        .into_iter()
        .filter(|set| {
            !world
                .balance(&AccountId::Problem, &Asset::Available(*set))
                .is_zero()
        })
        .collect();
    let selected = cover(world, remaining, available)?;
    let mut trace = Trace::new();
    let mut before = 0_u8;
    for set in selected {
        trace.push(action(RateId::Select { set, before }));
        before = before.checked_add(u8::try_from(members(world, set).len()).ok()?)?;
    }
    trace.push(action(RateId::Finish));

    let mut validation = world.clone();
    validation.replay(&trace).ok()?;
    validation.matches(&goal()).then_some(trace)
}

fn build(elements: &[Element], available: &[SetId]) -> World {
    let mut problem = Basket::new();
    for element in elements {
        problem.insert(Asset::Uncovered(*element), Quantity::new(1));
    }
    for set in available {
        problem.insert(Asset::Available(*set), Quantity::new(1));
    }
    problem.insert(Asset::Progress(0), Quantity::new(1));
    problem.insert(Asset::Active, Quantity::new(1));

    let mut builder = EconomyBuilder::new().account(AccountId::Problem, Account::from(problem));

    for &set in available {
        for before in (0..elements.len()).step_by(2).map(|value| value as u8) {
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

    let element_invariant = elements.iter().copied().fold(
        LinearInvariant::new("each universe element persists"),
        |invariant, element| {
            invariant
                .weight(Asset::Uncovered(element), 1)
                .weight(Asset::Covered(element), 1)
        },
    );
    let set_invariant = available.iter().copied().fold(
        LinearInvariant::new("set choice is single use"),
        |invariant, set| {
            invariant
                .weight(Asset::Available(set), 1)
                .weight(Asset::Selected(set), 1)
        },
    );
    let progress_invariant = (0..=elements.len()).step_by(2).fold(
        LinearInvariant::new("one progress token"),
        |invariant, progress| invariant.weight(Asset::Progress(progress as u8), 1),
    );

    let finish_requirements = elements.iter().copied().fold(
        Basket::from([(Asset::Progress(elements.len() as u8), Quantity::new(1))]),
        |mut basket, element| {
            basket.insert(Asset::Covered(element), Quantity::new(1));
            basket
        },
    );

    builder
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Problem, finish_requirements)
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
        SetId::Ef => [Element::E, Element::F],
        SetId::Gh => [Element::G, Element::H],
        SetId::Eg => [Element::E, Element::G],
        SetId::Fh => [Element::F, Element::H],
        SetId::Ad => [Element::A, Element::D],
        SetId::Bc => [Element::B, Element::C],
        SetId::Eh => [Element::E, Element::H],
        SetId::Fg => [Element::F, Element::G],
        SetId::Ij => [Element::I, Element::J],
        SetId::Kl => [Element::K, Element::L],
        SetId::Ik => [Element::I, Element::K],
        SetId::Jl => [Element::J, Element::L],
        SetId::Il => [Element::I, Element::L],
        SetId::Jk => [Element::J, Element::K],
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

pub fn elements(world: &World) -> Vec<Element> {
    STRESS_ELEMENTS
        .into_iter()
        .filter(|element| {
            !world
                .balance(&AccountId::Problem, &Asset::Uncovered(*element))
                .is_zero()
                || !world
                    .balance(&AccountId::Problem, &Asset::Covered(*element))
                    .is_zero()
        })
        .collect()
}

pub fn sets(world: &World) -> Vec<SetId> {
    let mut values = BTreeSet::new();
    for rate in world.rate_ids() {
        if let RateId::Select { set, .. } = rate {
            values.insert(*set);
        }
    }
    values.into_iter().collect()
}

pub fn members(world: &World, set: SetId) -> Vec<Element> {
    encoded_members(world, set).into_iter().collect()
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

    #[test]
    fn stress_profile_expands_the_incidence_problem_and_remains_solvable() {
        let showcase = initial_showcase();
        let stress = initial_stress();

        assert!(elements(&stress).len() > elements(&showcase).len());
        assert!(sets(&stress).len() > sets(&showcase).len());

        for trace in [
            solve_bfs(&stress)
                .expect("stress cover exists")
                .trace()
                .clone(),
            algorithm_x(&stress).expect("Algorithm X finds the stress cover"),
        ] {
            let replayed = stress
                .replayed(&trace)
                .expect("stress proposal must replay");
            assert!(replayed.matches(&goal()));
        }

        let unsatisfiable = unsatisfiable_stress();
        assert!(solve_bfs(&unsatisfiable).is_none());
        assert!(algorithm_x(&unsatisfiable).is_none());
    }

    #[test]
    fn every_available_set_combination_matches_a_direct_domain_oracle() {
        for mask in 0_u8..(1 << MICRO_SETS.len()) {
            let available = MICRO_SETS
                .into_iter()
                .enumerate()
                .filter_map(|(index, set)| (mask & (1 << index) != 0).then_some(set))
                .collect::<Vec<_>>();
            let expected = brute_force_has_exact_cover(&available);
            let world = build(&MICRO_ELEMENTS, &available);

            assert_eq!(solve_bfs(&world).is_some(), expected, "mask {mask:04b}");
            assert_eq!(algorithm_x(&world).is_some(), expected, "mask {mask:04b}");
        }
    }

    fn brute_force_has_exact_cover(available: &[SetId]) -> bool {
        (0_u8..(1 << available.len())).any(|selection| {
            MICRO_ELEMENTS.into_iter().all(|element| {
                available
                    .iter()
                    .enumerate()
                    .filter(|(index, set)| {
                        selection & (1 << index) != 0 && declared_members(**set).contains(&element)
                    })
                    .count()
                    == 1
            })
        })
    }
}
