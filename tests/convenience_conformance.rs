//! Convenience panics and string callbacks against Lean-generated contracts.
use axionomy::{
    Basket, Exchange, LinearInvariant, Quantity, QuantityComparison, QuantityExpression, Rate,
    basket,
};
use serde_json::{Value, json};
use std::{any::Any, cell::Cell, panic::AssertUnwindSafe};

#[derive(Debug, PartialEq, Eq, Hash)]
struct LiteralAsset(u64); // No Clone, Copy or Ord bound is needed by basket().

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        struct $name(u64);
    };
}
identifier!(Asset);
identifier!(Role);
identifier!(Holder);
identifier!(RuleId);

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../verification/conformance.json")).unwrap()
}
fn items(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap()
}
fn number(value: &Value) -> u64 {
    value.as_u64().unwrap()
}
fn quantity(value: &Value) -> Quantity {
    Quantity::new(text(value).parse().unwrap())
}
fn amounts(value: &Value) -> Basket<Asset> {
    Basket::try_from_entries(
        items(value)
            .iter()
            .map(|e| (Asset(number(&e[0])), quantity(&e[1]))),
    )
    .unwrap()
}
fn contents(value: &Basket<Asset>) -> Value {
    json!(
        value
            .iter()
            .map(|(a, q)| (a.0, q.to_string()))
            .collect::<Vec<_>>()
    )
}
fn panic_text(error: &(dyn Any + Send)) -> &str {
    error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .expect("text panic payload")
}

fn literal<const N: usize>(entries: Vec<(LiteralAsset, u64)>) -> Basket<LiteralAsset> {
    basket::<_, N>(entries.try_into().unwrap())
}

#[test]
fn literal_arrays_match_lean_and_panic_exactly_on_duplicate_keys() {
    let data = fixtures();
    let cases = items(&data["convenience"]["literals"]);
    assert_eq!(cases.len(), 781);
    for case in cases {
        let entries = items(&case["entries"])
            .iter()
            .map(|e| (LiteralAsset(number(&e[0])), text(&e[1]).parse().unwrap()))
            .collect::<Vec<_>>();
        let result = std::panic::catch_unwind(|| match entries.len() {
            0 => literal::<0>(entries),
            1 => literal::<1>(entries),
            2 => literal::<2>(entries),
            3 => literal::<3>(entries),
            4 => literal::<4>(entries),
            length => panic!("unexpected fixture length {length}"),
        });
        match result {
            Ok(value) => assert_eq!(
                json!(
                    value
                        .iter()
                        .map(|(a, q)| (a.0, q.to_string()))
                        .collect::<Vec<_>>()
                ),
                case["expected"]
            ),
            Err(error) => {
                assert!(case["expected"].is_null());
                assert_eq!(
                    panic_text(error.as_ref()),
                    "basket entries must have unique assets"
                );
            }
        }
    }
}

#[test]
fn rate_shortcuts_match_checked_merge_or_panic_on_overflow() {
    let data = fixtures();
    let cases = items(&data["rateMerges"]);
    assert_eq!(cases.len(), 56);
    for case in cases {
        let mut original = Rate::<Role, Asset>::new();
        for pair in items(&case["target"]) {
            let role = Role(number(&pair[0]));
            original = original
                .consume(role, amounts(&pair[1]))
                .produce(role, amounts(&pair[1]))
                .preserve(role, amounts(&pair[1]));
        }
        for (kind, message) in [
            "rate consume basket quantity overflow",
            "rate produce basket quantity overflow",
            "rate preserve basket quantity overflow",
        ]
        .into_iter()
        .enumerate()
        {
            let role = Role(number(&case["role"]));
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| match kind {
                0 => original.clone().consume(role, amounts(&case["assets"])),
                1 => original.clone().produce(role, amounts(&case["assets"])),
                _ => original.clone().preserve(role, amounts(&case["assets"])),
            }));
            assert_eq!(result.is_err(), !case["overflow"].is_null());
            match result {
                Err(error) => assert_eq!(panic_text(error.as_ref()), message),
                Ok(updated) => {
                    assert_eq!(
                        json!(updated.roles().map(|r| r.0).collect::<Vec<_>>()),
                        case["roles"]
                    );
                    for field in 0..3 {
                        let observed = updated
                            .roles()
                            .filter_map(|role| {
                                let values = match field {
                                    0 => updated.consumed(role),
                                    1 => updated.produced(role),
                                    _ => updated.preserved(role),
                                }?;
                                Some((role.0, contents(values)))
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(
                            json!(observed),
                            case[if kind == field { "merged" } else { "target" }]
                        );
                    }
                    assert!(updated.distinct_roles().next().is_none());
                    assert!(updated.computed_consumed().is_empty());
                    assert!(updated.computed_produced().is_empty());
                    assert!(updated.computed_preserved().is_empty());
                    assert!(updated.conditions().is_empty());
                }
            }
        }
    }
}

struct Name<'a> {
    input: &'a str,
    fails: bool,
    calls: &'a Cell<usize>,
}
impl From<Name<'_>> for String {
    fn from(value: Name<'_>) -> Self {
        value.calls.set(value.calls.get() + 1);
        assert!(!value.fails, "scripted conversion panic");
        format!("{}!", value.input)
    }
}

fn callback<T>(case: &Value, operation: impl FnOnce(Name<'_>) -> T) -> Option<T> {
    let calls = Cell::new(0);
    let fails = case["fails"].as_bool().unwrap();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        operation(Name {
            input: text(&case["input"]),
            fails,
            calls: &calls,
        })
    }));
    assert_eq!(calls.get(), 1);
    assert_eq!(result.is_err(), fails);
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            assert_eq!(panic_text(error.as_ref()), "scripted conversion panic");
            None
        }
    }
}

#[test]
fn expression_proposal_invariant_and_condition_callbacks_match_lean() {
    let data = fixtures();
    let cases = items(&data["convenience"]["callbacks"]);
    assert_eq!(cases.len(), 72);
    for case in cases {
        if let Some(value) = callback(case, |name| {
            QuantityExpression::<Role, Asset>::parameter(name)
        }) {
            assert!(
                matches!(value, QuantityExpression::Parameter { name } if name == text(&case["label"]))
            );
        }
        if let Some(value) = callback(case, |name| LinearInvariant::<Asset>::new(name)) {
            assert_eq!(value.name(), text(&case["label"]));
            assert!(value.weights().next().is_none());
        }
        let proposal = Exchange::new(RuleId(9), Quantity::new(2))
            .bind(Role(1), Holder(2))
            .parameter("!", Quantity::new(7))
            .parameter("z", Quantity::new(3));
        if let Some(value) = callback(case, |name| {
            proposal.parameter(name, quantity(&case["value"]))
        }) {
            assert_eq!(*value.rate(), RuleId(9));
            assert_eq!(*value.units(), Quantity::new(2));
            assert_eq!(
                value.bindings().iter().collect::<Vec<_>>(),
                vec![(&Role(1), &Holder(2))]
            );
            assert_eq!(
                json!(
                    value
                        .parameters()
                        .iter()
                        .map(|(k, q)| (k, q.to_string()))
                        .collect::<Vec<_>>()
                ),
                case["parameters"]
            );
        }
        let left = QuantityExpression::balance(Role(5), Asset(7));
        let right = QuantityExpression::plus(
            QuantityExpression::parameter("param"),
            QuantityExpression::balance(Role(2), Asset(9)),
        );
        let comparison = match text(&case["comparison"]) {
            "equal" => QuantityComparison::Equal,
            "greater" => QuantityComparison::GreaterThanOrEqual,
            "less" => QuantityComparison::LessThanOrEqual,
            value => panic!("unknown fixture comparison {value}"),
        };
        let original = Rate::new()
            .consume(Role(1), basket([(Asset(0), 1)]))
            .distinct(Role(8), Role(8))
            .condition(
                "previous",
                QuantityExpression::constant(0),
                QuantityComparison::Equal,
                QuantityExpression::constant(0),
            );
        if let Some(value) = callback(case, |name| {
            original
                .clone()
                .condition(name, left.clone(), comparison, right.clone())
        }) {
            assert_eq!(
                json!(value.roles().map(|r| r.0).collect::<Vec<_>>()),
                case["roles"]
            );
            assert_eq!(value.conditions().len(), 2);
            let added = &value.conditions()[1];
            assert_eq!(added.name(), text(&case["label"]));
            assert_eq!(added.left(), &left);
            assert_eq!(added.right(), &right);
            assert_eq!(added.comparison(), comparison);
            assert_eq!(&value.conditions()[..1], original.conditions());
            assert_eq!(
                value.distinct_roles().collect::<Vec<_>>(),
                original.distinct_roles().collect::<Vec<_>>()
            );
            for role in value.roles() {
                assert_eq!(value.consumed(role), original.consumed(role));
                assert_eq!(value.produced(role), original.produced(role));
                assert_eq!(value.preserved(role), original.preserved(role));
            }
            assert_eq!(value.computed_consumed(), original.computed_consumed());
            assert_eq!(value.computed_produced(), original.computed_produced());
            assert_eq!(value.computed_preserved(), original.computed_preserved());
        }
    }
}
