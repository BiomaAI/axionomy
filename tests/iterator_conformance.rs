//! Iterator consumers against Lean specifications, including non-fused inputs.
use axionomy::{Economy, EconomyBuilder, Exchange, Quantity, Trace};
use serde::Deserialize;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "support/scripted_iterator.rs"]
mod scripted_iterator;

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        struct $name(u64);
    };
}
identifier!(Asset);
identifier!(Holder);
identifier!(Role);
identifier!(RuleId);
type World = Economy<Holder, Asset, RuleId, Role>;
type Proposal = Exchange<RuleId, Role, Holder>;

fn proposal(id: u64) -> Proposal {
    Exchange::new(RuleId(id), Quantity::new(1))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    initial: Vec<u64>,
    script: Vec<Option<u64>>,
    expected_trace: Vec<u64>,
    expected_visible: Vec<u64>,
    calls: usize,
}

#[derive(Deserialize)]
struct Fixtures {
    iterators: Vec<Case>,
}

#[test]
fn trace_extension_and_views_match_lean_for_scripted_iterators() {
    let fixtures: Fixtures =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    assert_eq!(fixtures.iterators.len(), 24);
    let model: World = EconomyBuilder::new().build().unwrap();
    for case in fixtures.iterators {
        let mut trace = Trace::new();
        for id in case.initial {
            trace.push(proposal(id));
        }
        let (source, starts, calls) =
            scripted_iterator::source(case.script.iter().map(|value| value.map(proposal)));
        trace.extend(source);
        assert_eq!(
            trace.into_exchanges(),
            case.expected_trace
                .into_iter()
                .map(proposal)
                .collect::<Vec<_>>()
        );
        assert_eq!(starts.get(), 1);
        assert_eq!(calls.get(), case.calls);

        let (source, starts, calls) =
            scripted_iterator::source(case.script.into_iter().map(|value| value.map(Holder)));
        let view = model.view(source);
        assert_eq!(
            view.observation_key().visible_accounts(),
            case.expected_visible
                .into_iter()
                .map(Holder)
                .collect::<Vec<_>>()
        );
        assert_eq!(starts.get(), 1);
        assert_eq!(calls.get(), case.calls);
    }
}

#[test]
fn iterator_panics_propagate_after_each_successful_prefix() {
    let model: World = EconomyBuilder::new().build().unwrap();
    for length in 0..4 {
        let calls = Cell::new(0);
        let input = || {
            std::iter::from_fn(|| {
                let index = calls.get();
                calls.set(index + 1);
                assert!(index < length, "iterator failure");
                Some(index as u64)
            })
        };
        let mut trace = Trace::<RuleId, Role, Holder>::new();
        assert!(catch_unwind(AssertUnwindSafe(|| trace.extend(input().map(proposal)))).is_err());
        assert_eq!(calls.get(), length + 1);
        calls.set(0);
        assert!(catch_unwind(AssertUnwindSafe(|| model.view(input().map(Holder)))).is_err());
        assert_eq!(calls.get(), length + 1);
        calls.set(0);
        assert!(
            catch_unwind(AssertUnwindSafe(|| model.applicable(input().map(proposal)))).is_err()
        );
        assert_eq!(calls.get(), length + 1);
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
fn into_iterator_panics_propagate() {
    let model: World = EconomyBuilder::new().build().unwrap();
    let mut trace = Trace::<RuleId, Role, Holder>::new();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || trace.extend(FailingSource(std::marker::PhantomData))
        ))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(
            || model.view(FailingSource::<Holder>(std::marker::PhantomData))
        ))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| model.applicable(FailingSource::<
            Proposal,
        >(
            std::marker::PhantomData
        ))))
        .is_err()
    );
}
