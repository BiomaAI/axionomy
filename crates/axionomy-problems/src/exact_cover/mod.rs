#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    basket,
};

#[cfg(feature = "packages")]
pub mod package;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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
    Ace,
    Bfg,
    Dh,
    Adg,
    Beh,
    Cf,
    Abef,
    Cdgh,
    Ae,
    Fgh,
    Aei,
    Bfj,
    Cgk,
    Dhl,
    Abij,
    Cdk,
    Efl,
    Ai,
    Aj,
    Ak,
    Al,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Uncovered(Element),
    Covered(Element),
    Available(SetId),
    Selected(SetId),
    Progress(u8),
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Select { set: SetId, before: u8 },
    Finish,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `exact-cover` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "exact-cover";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

const MICRO_SETS: [SetId; 4] = [SetId::Ab, SetId::Cd, SetId::Ac, SetId::Bd];
const MICRO_ELEMENTS: [Element; 4] = [Element::A, Element::B, Element::C, Element::D];
const SHOWCASE_SETS: [SetId; 14] = [
    SetId::Ab,
    SetId::Cd,
    SetId::Ef,
    SetId::Gh,
    SetId::Ace,
    SetId::Bfg,
    SetId::Dh,
    SetId::Adg,
    SetId::Beh,
    SetId::Cf,
    SetId::Abef,
    SetId::Cdgh,
    SetId::Ae,
    SetId::Fgh,
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
const STRESS_SETS: [SetId; 25] = [
    SetId::Ab,
    SetId::Cd,
    SetId::Ef,
    SetId::Gh,
    SetId::Ij,
    SetId::Kl,
    SetId::Ace,
    SetId::Bfg,
    SetId::Dh,
    SetId::Adg,
    SetId::Beh,
    SetId::Cf,
    SetId::Abef,
    SetId::Cdgh,
    SetId::Ae,
    SetId::Fgh,
    SetId::Aei,
    SetId::Bfj,
    SetId::Cgk,
    SetId::Dhl,
    SetId::Abij,
    SetId::Cdk,
    SetId::Efl,
    SetId::Ai,
    SetId::Al,
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

/// One connected incidence matrix with pairs, triples, and quadruples.
/// Legal selections can dead-end, and complete covers have different sizes.
pub fn initial_showcase() -> World {
    build(&SHOWCASE_ELEMENTS, &SHOWCASE_SETS)
}

/// Cross-region triples and quadruples couple all twelve elements.
pub fn initial_stress() -> World {
    build(&STRESS_ELEMENTS, &STRESS_SETS)
}

pub fn unsatisfiable_showcase() -> World {
    // Every element occurs, but covering B, C, D, and E would reuse A.
    build(
        &SHOWCASE_ELEMENTS,
        &[SetId::Ab, SetId::Ac, SetId::Ad, SetId::Ae, SetId::Fgh],
    )
}

pub fn unsatisfiable_stress() -> World {
    build(
        &STRESS_ELEMENTS,
        &[
            SetId::Ab,
            SetId::Ac,
            SetId::Ad,
            SetId::Ae,
            SetId::Fgh,
            SetId::Ai,
            SetId::Aj,
            SetId::Ak,
            SetId::Al,
        ],
    )
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Problem, basket([(Asset::Solved, 1)]))
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
        let width = declared_members(set).len();
        for before in (0..=elements.len() - width).map(|value| value as u8) {
            let mut consume = Basket::from([
                (Asset::Available(set), Quantity::new(1)),
                (Asset::Progress(before), Quantity::new(1)),
            ]);
            let mut produce = Basket::from([
                (Asset::Selected(set), Quantity::new(1)),
                (Asset::Progress(before + width as u8), Quantity::new(1)),
            ]);
            for &element in declared_members(set) {
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
        LinearInvariant::new(path!("each-universe-element-persists")),
        |invariant, element| {
            invariant
                .weight(Asset::Uncovered(element), 1)
                .weight(Asset::Covered(element), 1)
        },
    );
    let set_invariant = available.iter().copied().fold(
        LinearInvariant::new(path!("set-choice-is-single-use")),
        |invariant, set| {
            invariant
                .weight(Asset::Available(set), 1)
                .weight(Asset::Selected(set), 1)
        },
    );
    let progress_invariant = (0..=elements.len()).fold(
        LinearInvariant::new(path!("one-progress-token")),
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
            LinearInvariant::new(path!("exact-cover-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("exact-cover model is valid")
}

fn declared_members(set: SetId) -> &'static [Element] {
    match set {
        SetId::Ab => &[Element::A, Element::B],
        SetId::Cd => &[Element::C, Element::D],
        SetId::Ac => &[Element::A, Element::C],
        SetId::Bd => &[Element::B, Element::D],
        SetId::Ef => &[Element::E, Element::F],
        SetId::Gh => &[Element::G, Element::H],
        SetId::Eg => &[Element::E, Element::G],
        SetId::Fh => &[Element::F, Element::H],
        SetId::Ad => &[Element::A, Element::D],
        SetId::Bc => &[Element::B, Element::C],
        SetId::Eh => &[Element::E, Element::H],
        SetId::Fg => &[Element::F, Element::G],
        SetId::Ij => &[Element::I, Element::J],
        SetId::Kl => &[Element::K, Element::L],
        SetId::Ik => &[Element::I, Element::K],
        SetId::Jl => &[Element::J, Element::L],
        SetId::Il => &[Element::I, Element::L],
        SetId::Jk => &[Element::J, Element::K],
        SetId::Ace => &[Element::A, Element::C, Element::E],
        SetId::Bfg => &[Element::B, Element::F, Element::G],
        SetId::Dh => &[Element::D, Element::H],
        SetId::Adg => &[Element::A, Element::D, Element::G],
        SetId::Beh => &[Element::B, Element::E, Element::H],
        SetId::Cf => &[Element::C, Element::F],
        SetId::Abef => &[Element::A, Element::B, Element::E, Element::F],
        SetId::Cdgh => &[Element::C, Element::D, Element::G, Element::H],
        SetId::Ae => &[Element::A, Element::E],
        SetId::Fgh => &[Element::F, Element::G, Element::H],
        SetId::Aei => &[Element::A, Element::E, Element::I],
        SetId::Bfj => &[Element::B, Element::F, Element::J],
        SetId::Cgk => &[Element::C, Element::G, Element::K],
        SetId::Dhl => &[Element::D, Element::H, Element::L],
        SetId::Abij => &[Element::A, Element::B, Element::I, Element::J],
        SetId::Cdk => &[Element::C, Element::D, Element::K],
        SetId::Efl => &[Element::E, Element::F, Element::L],
        SetId::Ai => &[Element::A, Element::I],
        SetId::Aj => &[Element::A, Element::J],
        SetId::Ak => &[Element::A, Element::K],
        SetId::Al => &[Element::A, Element::L],
    }
}

/// The exchange selecting a set, or finishing, on the one problem account.
pub fn action(rate: RateId) -> Action {
    Exchange::new(rate, Quantity::new(1)).bind(Role::Problem, AccountId::Problem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The elements `world`'s problem holds, covered or not.
    fn elements(world: &World) -> Vec<Element> {
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

    /// The sets `world` can select.
    fn sets(world: &World) -> Vec<SetId> {
        let mut values = BTreeSet::new();
        for rate in world.rate_ids() {
            if let RateId::Select { set, .. } = rate {
                values.insert(*set);
            }
        }
        values.into_iter().collect()
    }

    /// The elements selecting `set` covers, read from what its rate consumes.
    fn members(world: &World, set: SetId) -> BTreeSet<Element> {
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

    /// Every subset of `sets`, each in the order `sets` lists them.
    fn subsets(sets: &[SetId]) -> impl Iterator<Item = Vec<SetId>> + '_ {
        (0_u32..1 << sets.len()).map(move |mask| {
            sets.iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, set)| *set)
                .collect()
        })
    }

    /// Whether `world` accepts selecting `chosen` in turn, each selection
    /// naming the progress before it, and then finishing.
    fn accepts(world: &World, chosen: &[SetId]) -> bool {
        let mut world = world.clone();
        let mut before = 0;
        for &set in chosen {
            if world.apply(action(RateId::Select { set, before })).is_err() {
                return false;
            }
            before += declared_members(set).len() as u8;
        }
        world.apply(action(RateId::Finish)).is_ok() && world.matches(&goal())
    }

    /// The oracle, independent of the economy: whether `chosen`'s declared
    /// members cover each element of `universe` exactly once.
    fn is_exact_cover(universe: &[Element], chosen: &[SetId]) -> bool {
        universe.iter().all(|element| {
            chosen
                .iter()
                .filter(|set| declared_members(**set).contains(element))
                .count()
                == 1
        })
    }

    #[test]
    fn a_cover_selects_disjoint_sets_then_finishes_and_is_quiescent() {
        let world = initial();
        let mut overlapping = world.clone();
        overlapping
            .apply(action(RateId::Select {
                set: SetId::Ab,
                before: 0,
            }))
            .unwrap();
        // AC needs A uncovered, which AB consumed.
        assert!(!overlapping.is_applicable(&action(RateId::Select {
            set: SetId::Ac,
            before: 2,
        })));

        let solved = world
            .replayed(
                &[
                    RateId::Select {
                        set: SetId::Ab,
                        before: 0,
                    },
                    RateId::Select {
                        set: SetId::Cd,
                        before: 2,
                    },
                    RateId::Finish,
                ]
                .into_iter()
                .map(action)
                .collect(),
            )
            .expect("AB and CD cover the universe");
        assert!(solved.matches(&goal()));
        assert!(
            solved
                .rate_ids()
                .all(|rate| !solved.is_applicable(&action(*rate)))
        );
    }

    #[test]
    fn every_available_set_combination_accepts_exactly_the_covers_a_direct_oracle_finds() {
        for mask in 0_u8..(1 << MICRO_SETS.len()) {
            let available = MICRO_SETS
                .into_iter()
                .enumerate()
                .filter_map(|(index, set)| (mask & (1 << index) != 0).then_some(set))
                .collect::<Vec<_>>();
            let world = build(&MICRO_ELEMENTS, &available);
            for chosen in subsets(&available) {
                assert_eq!(
                    accepts(&world, &chosen),
                    is_exact_cover(&MICRO_ELEMENTS, &chosen),
                    "mask {mask:04b}: {chosen:?}"
                );
            }
        }
        // Only AB and AC: no cover.
        let world = unsatisfiable();
        assert!(subsets(&sets(&world)).all(|chosen| !accepts(&world, &chosen)));
    }

    #[test]
    fn stress_profile_expands_the_incidence_problem_and_remains_solvable() {
        let showcase = initial_showcase();
        let stress = initial_stress();

        assert!(elements(&stress).len() > elements(&showcase).len());
        assert!(sets(&stress).len() > sets(&showcase).len());
        assert!(accepts(
            &stress,
            &[SetId::Abef, SetId::Cdgh, SetId::Ij, SetId::Kl]
        ));
    }

    #[test]
    fn irregular_matrix_has_global_coupling_different_covers_and_legal_dead_ends() {
        let initial = initial_showcase();
        let available = sets(&initial);
        let widths: BTreeSet<_> = available
            .iter()
            .map(|set| members(&initial, *set).len())
            .collect();
        assert_eq!(widths, BTreeSet::from([2, 3, 4]));
        // An independent subset oracle names every cover; the economy accepts
        // exactly those.
        let universe = elements(&initial);
        let mut covers = Vec::new();
        for chosen in subsets(&available) {
            let cover = is_exact_cover(&universe, &chosen);
            assert_eq!(accepts(&initial, &chosen), cover, "{chosen:?}");
            if cover {
                covers.push(chosen);
            }
        }
        let cover_sizes: BTreeSet<_> = covers.iter().map(Vec::len).collect();
        assert_eq!(cover_sizes, BTreeSet::from([2, 3, 4]));
        let mut connected = BTreeSet::from([Element::A]);
        loop {
            let before = connected.len();
            for set in &available {
                let members = members(&initial, *set);
                if !members.is_disjoint(&connected) {
                    connected.extend(members);
                }
            }
            if connected.len() == before {
                break;
            }
        }
        assert_eq!(connected.len(), universe.len());
        // A legal first selection that no cover contains blocks completion.
        let dead_ends = available
            .iter()
            .filter(|set| {
                initial.is_applicable(&action(RateId::Select {
                    set: **set,
                    before: 0,
                }))
            })
            .filter(|set| !covers.iter().any(|cover| cover.contains(set)))
            .count();
        assert!(
            dead_ends > 0,
            "some locally legal choices must block a cover"
        );
    }

    #[test]
    fn every_element_can_occur_even_when_no_global_cover_exists() {
        for world in [unsatisfiable_showcase(), unsatisfiable_stress()] {
            let available = sets(&world);
            assert!(elements(&world).iter().all(|element| {
                available
                    .iter()
                    .any(|set| members(&world, *set).contains(element))
            }));
            assert!(subsets(&available).all(|chosen| !accepts(&world, &chosen)));
        }
    }
}
