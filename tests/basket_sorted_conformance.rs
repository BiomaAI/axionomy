//! Sorted views preserve borrowed entries and follow the ontology's asset order.
use axionomy::{Basket, Quantity, QuantityScalar};
use serde::Deserialize;
use std::{cmp::Reverse, hash::Hash};

// Deliberately not Clone: sorting borrows asset identities.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Asset(u64);

#[derive(Deserialize)]
struct Case {
    entries: Vec<(u64, String)>,
    ascending: Vec<(u64, String)>,
    descending: Vec<(u64, String)>,
}

#[derive(Deserialize)]
struct Fixtures {
    #[serde(rename = "basketSorted")]
    baskets: Vec<Case>,
}

fn fixtures() -> Vec<Case> {
    let data: Fixtures =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    assert_eq!(data.baskets.len(), 326);
    data.baskets
}

fn check<A: Eq + Hash + Ord, N: QuantityScalar>(
    basket: &Basket<A, N>,
    key: impl Fn(&A) -> u64,
    expected: &[(u64, String)],
) {
    let original = basket
        .iter()
        .map(|(asset, quantity)| (key(asset), quantity.to_string()))
        .collect::<Vec<_>>();
    let sorted = basket.iter_sorted().collect::<Vec<_>>();
    let observed = sorted
        .iter()
        .map(|(asset, quantity)| (key(asset), quantity.to_string()))
        .collect::<Vec<_>>();
    assert_eq!(observed, expected);
    for (asset, quantity) in sorted {
        let (stored_asset, stored_quantity) = basket.iter().find(|(a, _)| *a == asset).unwrap();
        assert!(std::ptr::eq(asset, stored_asset));
        assert!(std::ptr::eq(quantity, stored_quantity));
    }
    assert_eq!(
        basket
            .iter()
            .map(|(asset, quantity)| (key(asset), quantity.to_string()))
            .collect::<Vec<_>>(),
        original
    );
}

#[test]
fn sorted_borrowed_views_match_lean_in_both_asset_orders() {
    for case in fixtures() {
        let ascending = Basket::try_from_entries(
            case.entries
                .iter()
                .map(|(asset, value)| (Asset(*asset), Quantity::new(value.parse().unwrap()))),
        )
        .unwrap();
        check(&ascending, |asset| asset.0, &case.ascending);
        let descending = Basket::try_from_entries(case.entries.iter().map(|(asset, value)| {
            (
                Reverse(Asset(*asset)),
                Quantity::new(value.parse().unwrap()),
            )
        }))
        .unwrap();
        check(&descending, |asset| asset.0.0, &case.descending);
    }
}

#[test]
fn sorting_does_not_require_scalar_traits() {
    struct UnorderedNonClone;
    let basket = Basket::<Asset, UnorderedNonClone>::new();
    assert!(basket.iter_sorted().next().is_none());
}

#[cfg(feature = "bigint")]
#[test]
fn non_copy_quantities_are_borrowed_without_replacement() {
    for case in fixtures() {
        let basket = Basket::try_from_entries(case.entries.iter().map(|(asset, value)| {
            (
                Asset(*asset),
                Quantity::try_from_scalar(value.parse::<num_bigint::BigUint>().unwrap()).unwrap(),
            )
        }))
        .unwrap();
        check(&basket, |asset| asset.0, &case.ascending);
    }
}
