//! Production collection boundaries against independently generated Lean cases.
use axionomy::{AssetAmount, Basket, BasketError, Quantity};
use serde::Deserialize;
use std::{
    any::Any,
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "support/scripted_iterator.rs"]
mod scripted_iterator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
struct Asset(u64);
type Entry = (Asset, Quantity);
type Amounts = Basket<Asset>;

#[derive(Deserialize)]
struct Case {
    entries: Vec<(Asset, String)>,
    accepted: Option<Vec<(Asset, String)>>,
    duplicate: Option<Asset>,
    pulls: usize,
}

#[derive(Deserialize)]
struct Fixtures {
    #[serde(rename = "basketCollections")]
    baskets: Vec<Case>,
}

fn fixtures() -> Vec<Case> {
    let document: Fixtures =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    assert_eq!(document.baskets.len(), 85);
    document.baskets
}

fn entries(case: &Case) -> Vec<Entry> {
    case.entries
        .iter()
        .map(|(asset, value)| (*asset, value.parse::<Quantity>().unwrap()))
        .collect()
}

fn assert_result(result: Result<Amounts, BasketError<Asset>>, case: &Case) {
    match result {
        Ok(basket) => {
            assert!(case.duplicate.is_none());
            let observed = basket
                .iter()
                .map(|(a, q)| (*a, q.to_string()))
                .collect::<Vec<_>>();
            assert_eq!(Some(&observed), case.accepted.as_ref());
            assert_eq!(basket.len(), observed.len());
            assert_eq!(basket.is_empty(), observed.is_empty());
            for asset in [Asset(0), Asset(1), Asset(9)] {
                assert_eq!(
                    basket.get(&asset).map(ToString::to_string),
                    observed
                        .iter()
                        .find(|(a, _)| *a == asset)
                        .map(|(_, q)| q.clone())
                );
            }
        }
        Err(BasketError::DuplicateAsset { asset }) => assert_eq!(Some(asset), case.duplicate),
    }
}

fn panic_text(error: &(dyn Any + Send)) -> &str {
    error
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| error.downcast_ref::<String>().map(String::as_str))
        .unwrap()
}

fn assert_collection(result: Result<Amounts, Box<dyn Any + Send>>, case: &Case) {
    if case.duplicate.is_some() {
        assert_eq!(
            panic_text(result.unwrap_err().as_ref()),
            "basket entries must have unique assets"
        );
    } else {
        assert_result(Ok(result.unwrap()), case);
    }
}

#[test]
fn constructors_conversions_and_queries_match_lean() {
    assert!(Amounts::new().is_empty());
    assert!(Amounts::default().is_empty());
    for case in fixtures() {
        let values = entries(&case);
        let schedule = || {
            values
                .iter()
                .copied()
                .map(Some)
                .chain([None, Some((Asset(9), Quantity::new(1)))])
        };
        let (source, starts, pulls) = scripted_iterator::source(schedule());
        assert_result(Amounts::try_from_entries(source), &case);
        assert_eq!(starts.get(), 1);
        assert_eq!(pulls.get(), case.pulls);

        let (source, starts, pulls) =
            scripted_iterator::source(schedule().map(|v| v.map(AssetAmount::from)));
        assert_result(Amounts::try_from_amounts(source), &case);
        assert_eq!(starts.get(), 1);
        assert_eq!(pulls.get(), case.pulls);

        let (source, starts, pulls) = scripted_iterator::source(schedule());
        assert_collection(
            catch_unwind(AssertUnwindSafe(|| Amounts::from_iter(source))),
            &case,
        );
        assert_eq!(starts.get(), 1);
        assert_eq!(pulls.get(), case.pulls);

        let converted = catch_unwind(AssertUnwindSafe(|| match values.as_slice() {
            [] => Amounts::from([]),
            [a] => Amounts::from([*a]),
            [a, b] => Amounts::from([*a, *b]),
            [a, b, c] => Amounts::from([*a, *b, *c]),
            _ => unreachable!("generated arrays have lengths zero through three"),
        }));
        assert_collection(converted, &case);
    }
}

#[test]
fn first_duplicate_wins_over_an_unused_panicking_tail() {
    for case in fixtures() {
        let values = entries(&case);
        let pulls = Cell::new(0);
        let input = || {
            std::iter::from_fn(|| {
                let index = pulls.get();
                pulls.set(index + 1);
                Some(*values.get(index).expect("iterator failure"))
            })
        };
        let result = catch_unwind(AssertUnwindSafe(|| Amounts::try_from_entries(input())));
        if case.duplicate.is_some() {
            assert_result(result.unwrap(), &case);
        } else {
            assert_eq!(panic_text(result.unwrap_err().as_ref()), "iterator failure");
        }
        assert_eq!(pulls.get(), case.pulls);

        pulls.set(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            Amounts::try_from_amounts(input().map(AssetAmount::from))
        }));
        if case.duplicate.is_some() {
            assert_result(result.unwrap(), &case);
        } else {
            assert_eq!(panic_text(result.unwrap_err().as_ref()), "iterator failure");
        }
        assert_eq!(pulls.get(), case.pulls);

        pulls.set(0);
        let failure = catch_unwind(AssertUnwindSafe(|| Amounts::from_iter(input()))).unwrap_err();
        assert_eq!(
            panic_text(failure.as_ref()),
            if case.duplicate.is_some() {
                "basket entries must have unique assets"
            } else {
                "iterator failure"
            }
        );
        assert_eq!(pulls.get(), case.pulls);
    }
}

struct FailingSource<T>(std::marker::PhantomData<T>);
impl<T> IntoIterator for FailingSource<T> {
    type Item = T;
    type IntoIter = std::iter::Empty<T>;
    fn into_iter(self) -> Self::IntoIter {
        panic!("into_iter failure")
    }
}

#[test]
fn collection_input_conversion_failures_propagate() {
    let entries = || FailingSource::<Entry>(std::marker::PhantomData);
    assert_eq!(
        panic_text(
            catch_unwind(|| Amounts::try_from_entries(entries()))
                .unwrap_err()
                .as_ref()
        ),
        "into_iter failure"
    );
    assert_eq!(
        panic_text(
            catch_unwind(|| Amounts::from_iter(entries()))
                .unwrap_err()
                .as_ref()
        ),
        "into_iter failure"
    );
    let amounts = FailingSource::<AssetAmount<Asset>>(std::marker::PhantomData);
    assert_eq!(
        panic_text(
            catch_unwind(|| Amounts::try_from_amounts(amounts))
                .unwrap_err()
                .as_ref()
        ),
        "into_iter failure"
    );
}
