//! Public inspection against Lean-generated first-match, weight and asset oracles.
use axionomy::{
    Account, AccountAssessment, AccountShortfall, Basket, ComputedAmount, EconomyBuilder,
    ExchangeAssessment, LinearInvariant, Quantity, QuantityComparison, QuantityExpression,
    QuantityScalar, Rate, RateCondition,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, panic::AssertUnwindSafe};

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
        struct $name(u64);
    };
}
identifier!(Asset);
identifier!(Role);
identifier!(Holder);
identifier!(RuleId);

fn fixtures() -> Value {
    let document: Value =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    document["inspection"].clone()
}

fn number(value: &Value) -> u64 {
    value.as_u64().unwrap()
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap()
}

fn items(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}

fn exact_iterator<I: Iterator>(mut iter: I, count: usize) -> Vec<I::Item> {
    let mut output = Vec::new();
    for remaining in (0..=count).rev() {
        assert_eq!(iter.size_hint(), (remaining, Some(remaining)));
        if remaining == 0 {
            assert!(iter.next().is_none());
        } else {
            output.push(iter.next().expect("entry promised by the size hint"));
        }
    }
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert!(iter.next().is_none());
    output
}

// Deliberately PartialEq-only. The public lookup must not require Eq, Hash,
// Ord, Copy or Clone, and must not invoke comparisons beyond its stopping point.
#[derive(Debug, Deserialize)]
struct ProbeId(u64);

thread_local! {
    static COMPARISONS: RefCell<(Option<u64>, usize)> = const { RefCell::new((None, 0)) };
}

impl PartialEq for ProbeId {
    fn eq(&self, other: &Self) -> bool {
        COMPARISONS.with(|state| {
            let mut state = state.borrow_mut();
            state.1 += 1;
            assert_ne!(Some(self.0), state.0, "scripted comparison failure");
        });
        self.0 == other.0
    }
}

fn check_lookup(case: &Value, expected: &Value, lookup: impl FnOnce() -> Option<u64>) {
    COMPARISONS.with(|state| *state.borrow_mut() = (case["panicAt"].as_u64(), 0));
    let result = std::panic::catch_unwind(AssertUnwindSafe(lookup));
    assert_eq!(result.is_err(), expected["panics"].as_bool().unwrap());
    if let Ok(value) = result {
        assert_eq!(json!(value), expected["value"]);
    }
    COMPARISONS.with(|state| assert_eq!(state.borrow().1 as u64, number(&expected["comparisons"])));
}

#[test]
fn assessment_lookup_matches_lean_including_duplicates_and_comparison_panics() {
    let data = fixtures();
    assert_eq!(items(&data["lookups"]).len(), 144);
    for case in items(&data["lookups"]) {
        // Serde only constructs otherwise-private records for this lookup test;
        // these cases do not claim to prove serialization or schema behavior.
        let accounts = items(&case["entries"])
            .iter()
            .map(|entry| {
                serde_json::from_value::<AccountAssessment<ProbeId, Asset>>(json!({
                    "account": number(&entry[0]), "available": [],
                    "required": [[0, number(&entry[1])]], "consumed": [],
                    "produced": [], "preserved": []
                }))
                .unwrap()
            })
            .collect();
        let shortfalls = items(&case["entries"])
            .iter()
            .rev()
            .map(|entry| {
                serde_json::from_value::<AccountShortfall<ProbeId, Asset>>(json!({
                    "account": number(&entry[0]), "missing": [[0, number(&entry[1])]]
                }))
                .unwrap()
            })
            .collect();
        let assessment: ExchangeAssessment<ProbeId, Asset, RuleId, Role> = match text(&case["kind"])
        {
            "applicable" => ExchangeAssessment::Applicable {
                accounts,
                projected_deltas: vec![],
            },
            "infeasible" => ExchangeAssessment::Infeasible {
                accounts,
                shortfalls,
            },
            "invalid" => ExchangeAssessment::Invalid { issues: vec![] },
            kind => panic!("unknown fixture kind {kind}"),
        };
        let query = ProbeId(number(&case["query"]));
        check_lookup(case, &case["account"], || {
            assessment.account(&query).map(|found| {
                assert!(
                    assessment
                        .accounts()
                        .iter()
                        .any(|stored| std::ptr::eq(stored, found))
                );
                found.required().quantity(&Asset(0)).get()
            })
        });
        check_lookup(case, &case["shortfall"], || {
            assessment.shortfall(&query).map(|found| {
                assert!(
                    assessment
                        .shortfalls()
                        .iter()
                        .any(|stored| std::ptr::eq(stored.missing(), found))
                );
                found.quantity(&Asset(0)).get()
            })
        });
    }
}

fn weights(value: &LinearInvariant<Asset>) -> Value {
    json!(
        value
            .weights()
            .map(|(a, w)| (a.0, w.to_string()))
            .collect::<Vec<_>>()
    )
}

#[test]
fn invariant_updates_and_every_iterator_suffix_match_lean() {
    let data = fixtures();
    assert_eq!(items(&data["invariants"]).len(), 153);
    for case in items(&data["invariants"]) {
        let mut value = LinearInvariant::new(text(&case["name"]));
        for (index, expected) in items(&case["snapshots"]).iter().enumerate() {
            assert_eq!(value.name(), text(&case["name"]));
            assert_eq!(weights(&value), *expected);
            let borrowed = exact_iterator(value.weights(), items(expected).len());
            for ((asset, weight), (again, again_weight)) in
                borrowed.into_iter().zip(value.weights())
            {
                assert!(std::ptr::eq(asset, again));
                assert_eq!(weight, again_weight);
            }
            if let Some(update) = items(&case["updates"]).get(index) {
                value = value.weight(Asset(number(&update[0])), text(&update[1]).parse().unwrap());
            }
        }
    }
}

#[test]
fn invariant_name_conversion_is_called_once_and_propagates_panics() {
    struct Name<'a>(&'a std::cell::Cell<usize>, bool);
    impl From<Name<'_>> for String {
        fn from(value: Name<'_>) -> Self {
            value.0.set(value.0.get() + 1);
            assert!(!value.1, "scripted name conversion failure");
            "éλ🛠".to_owned()
        }
    }
    let calls = std::cell::Cell::new(0);
    let invariant = LinearInvariant::<Asset>::new(Name(&calls, false));
    assert_eq!(calls.get(), 1);
    assert_eq!(invariant.name(), "éλ🛠");
    assert_eq!(invariant.weights().count(), 0);
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            LinearInvariant::<Asset>::new(Name(&calls, true))
        }))
        .is_err()
    );
    assert_eq!(calls.get(), 2);
}

fn basket<N: QuantityScalar>(entries: &Value, parse: &impl Fn(&str) -> N) -> Basket<Asset, N> {
    Basket::try_from_entries(items(entries).iter().map(|entry| {
        (
            Asset(number(&entry[0])),
            Quantity::try_from_scalar(parse(text(&entry[1]))).unwrap(),
        )
    }))
    .unwrap()
}

fn basket_json<N: QuantityScalar>(contents: &Basket<Asset, N>) -> Value {
    json!(
        contents
            .iter()
            .map(|(a, q)| (a.0, q.to_string()))
            .collect::<Vec<_>>()
    )
}

fn expression<N: QuantityScalar>(
    input: &Value,
    parse: &impl Fn(&str) -> N,
) -> QuantityExpression<Role, Asset, N> {
    use QuantityExpression as E;
    match text(&input["kind"]) {
        "constant" => E::Constant {
            value: Quantity::try_from_scalar(parse(text(&input["value"]))).unwrap(),
        },
        "units" => E::units(),
        "parameter" => E::parameter(text(&input["name"])),
        "balance" => E::balance(Role(number(&input["role"])), Asset(number(&input["asset"]))),
        "mul_div" => E::mul_div(
            expression(&input["left"], parse),
            expression(&input["right"], parse),
            expression(&input["denominator"], parse),
            serde_json::from_value(input["rounding"].clone()).unwrap(),
        ),
        kind => {
            let left = expression(&input["left"], parse);
            let right = expression(&input["right"], parse);
            match kind {
                "add" => E::plus(left, right),
                "subtract" => E::subtract(left, right),
                "multiply" => E::multiply(left, right),
                "minimum" => E::minimum(left, right),
                "maximum" => E::maximum(left, right),
                _ => panic!("unknown expression {kind}"),
            }
        }
    }
}

fn expression_json<N: QuantityScalar>(value: &QuantityExpression<Role, Asset, N>) -> Value {
    use QuantityExpression as E;
    let (kind, left, right) = match value {
        E::Constant { value } => return json!({"kind": "constant", "value": value.to_string()}),
        E::Units => return json!({"kind": "units"}),
        E::Parameter { name } => return json!({"kind": "parameter", "name": name}),
        E::Balance { role, asset } => {
            return json!({"kind": "balance", "role": role.0, "asset": asset.0});
        }
        E::Add { left, right } => ("add", left, right),
        E::Subtract { left, right } => ("subtract", left, right),
        E::Multiply { left, right } => ("multiply", left, right),
        E::Minimum { left, right } => ("minimum", left, right),
        E::Maximum { left, right } => ("maximum", left, right),
        E::MulDiv {
            left,
            right,
            denominator,
            rounding,
        } => {
            return json!({
                "kind": "mul_div",
                "left": expression_json(left),
                "right": expression_json(right),
                "denominator": expression_json(denominator),
                "rounding": rounding,
            });
        }
    };
    json!({"kind": kind, "left": expression_json(left), "right": expression_json(right)})
}

fn build_rate<N: QuantityScalar>(
    input: &Value,
    parse: &impl Fn(&str) -> N,
) -> Rate<Role, Asset, N> {
    let mut rule = Rate::new();
    for field in ["consume", "produce", "preserve"] {
        for entry in items(&input[field]) {
            let role = Role(number(&entry[0]));
            let amounts = basket(&entry[1], parse);
            rule = match field {
                "consume" => rule.consume(role, amounts),
                "produce" => rule.produce(role, amounts),
                _ => rule.preserve(role, amounts),
            };
        }
    }
    for pair in items(&input["distinct"]) {
        // Reverse each input pair: the builder must retain the canonical order.
        rule = rule.distinct(Role(number(&pair[1])), Role(number(&pair[0])));
    }
    for field in ["computedConsume", "computedProduce", "computedPreserve"] {
        for amount in items(&input[field]) {
            let role = Role(number(&amount["role"]));
            let asset = Asset(number(&amount["asset"]));
            let quantity = expression(&amount["quantity"], parse);
            rule = match field {
                "computedConsume" => rule.consume_computed(role, asset, quantity),
                "computedProduce" => rule.produce_computed(role, asset, quantity),
                _ => rule.preserve_computed(role, asset, quantity),
            };
        }
    }
    for condition in items(&input["conditions"]) {
        let comparison = match text(&condition["comparison"]) {
            "equal" => QuantityComparison::Equal,
            "greater" => QuantityComparison::GreaterThanOrEqual,
            "less" => QuantityComparison::LessThanOrEqual,
            kind => panic!("unknown comparison {kind}"),
        };
        rule = rule.condition(
            text(&condition["name"]),
            expression(&condition["left"], parse),
            comparison,
            expression(&condition["right"], parse),
        );
    }
    rule
}

fn amount_json<N: QuantityScalar>(amount: &ComputedAmount<Role, Asset, N>) -> Value {
    json!({"role": amount.role().0, "asset": amount.asset().0, "quantity": expression_json(amount.quantity())})
}

fn condition_json<N: QuantityScalar>(condition: &RateCondition<Role, Asset, N>) -> Value {
    let comparison = match condition.comparison() {
        QuantityComparison::Equal => "equal",
        QuantityComparison::GreaterThanOrEqual => "greater",
        QuantityComparison::LessThanOrEqual => "less",
    };
    json!({"name": condition.name(), "left": expression_json(condition.left()),
        "right": expression_json(condition.right()), "comparison": comparison})
}

fn check_rate<N: QuantityScalar>(rule: &Rate<Role, Asset, N>, expected: &Value) {
    assert_eq!(
        json!(rule.roles().map(|r| r.0).collect::<Vec<_>>()),
        expected["roles"]
    );
    assert_eq!(
        json!(
            rule.distinct_roles()
                .map(|(a, b)| (a.0, b.0))
                .collect::<Vec<_>>()
        ),
        expected["distinct"]
    );
    for field in ["consume", "produce", "preserve"] {
        for role in 0..=10 {
            let observed = match field {
                "consume" => rule.consumed(&Role(role)),
                "produce" => rule.produced(&Role(role)),
                _ => rule.preserved(&Role(role)),
            };
            let expected = items(&expected[field])
                .iter()
                .find(|e| number(&e[0]) == role)
                .map(|e| e[1].clone());
            assert_eq!(observed.map(basket_json), expected);
        }
    }
    assert_eq!(
        json!(
            rule.computed_consumed()
                .iter()
                .map(amount_json)
                .collect::<Vec<_>>()
        ),
        expected["computedConsume"]
    );
    assert_eq!(
        json!(
            rule.computed_produced()
                .iter()
                .map(amount_json)
                .collect::<Vec<_>>()
        ),
        expected["computedProduce"]
    );
    assert_eq!(
        json!(
            rule.computed_preserved()
                .iter()
                .map(amount_json)
                .collect::<Vec<_>>()
        ),
        expected["computedPreserve"]
    );
    assert_eq!(
        json!(
            rule.conditions()
                .iter()
                .map(condition_json)
                .collect::<Vec<_>>()
        ),
        expected["conditions"]
    );
    assert_eq!(
        json!(
            rule.computed_asset_keys()
                .iter()
                .map(|a| a.0)
                .collect::<Vec<_>>()
        ),
        expected["computedAssets"]
    );
    assert!(std::ptr::eq(
        rule.computed_consumed(),
        rule.computed_consumed()
    ));
    assert!(std::ptr::eq(
        rule.computed_produced(),
        rule.computed_produced()
    ));
    assert!(std::ptr::eq(
        rule.computed_preserved(),
        rule.computed_preserved()
    ));
    assert!(std::ptr::eq(rule.conditions(), rule.conditions()));
}

fn check_model<N: QuantityScalar>(case: &Value, parse: impl Fn(&str) -> N) {
    let mut builder = EconomyBuilder::<Holder, Asset, RuleId, Role, N>::new();
    for entry in items(&case["accounts"]) {
        builder = builder.account(
            Holder(number(&entry[0])),
            Account::new(basket(&entry[1], &parse)),
        );
    }
    for entry in items(&case["rates"]) {
        builder = builder.rate(RuleId(number(&entry[0])), build_rate(&entry[1], &parse));
    }
    for entry in items(&case["invariants"]) {
        let mut invariant = LinearInvariant::new(text(&entry["name"]));
        for weight in items(&entry["weights"]) {
            invariant =
                invariant.weight(Asset(number(&weight[0])), text(&weight[1]).parse().unwrap());
        }
        builder = builder.invariant(invariant);
    }
    let model = builder.build().unwrap();
    let before = model.state_key();
    let holders = exact_iterator(model.accounts(), items(&case["accounts"]).len());
    assert_eq!(
        json!(
            holders
                .iter()
                .map(|(id, a)| (id.0, basket_json(a.balances())))
                .collect::<Vec<_>>()
        ),
        case["accounts"]
    );
    let mut stored_assets: Vec<&Asset> = Vec::new();
    for (id, holder) in holders {
        assert!(std::ptr::eq(holder, model.account(id).unwrap()));
        stored_assets.extend(holder.balances().iter().map(|(a, _)| a));
    }
    let ids = exact_iterator(model.rate_ids(), items(&case["rates"]).len());
    for (id, expected) in ids.into_iter().zip(items(&case["rates"])) {
        assert_eq!(id.0, number(&expected[0]));
        let rule = model.rate(id).unwrap();
        assert!(std::ptr::eq(rule, model.rate(id).unwrap()));
        check_rate(rule, &expected[1]);
        for role in rule.roles() {
            for contents in [
                rule.consumed(role),
                rule.produced(role),
                rule.preserved(role),
            ]
            .into_iter()
            .flatten()
            {
                stored_assets.extend(contents.iter().map(|(a, _)| a));
            }
        }
        stored_assets.extend(rule.computed_asset_keys());
    }
    assert!(model.rate(&RuleId(999)).is_none());
    let invariants = exact_iterator(model.invariants(), items(&case["invariants"]).len());
    for (invariant, expected) in invariants.into_iter().zip(items(&case["invariants"])) {
        assert_eq!(invariant.name(), text(&expected["name"]));
        assert_eq!(weights(invariant), expected["weights"]);
        stored_assets.extend(
            exact_iterator(invariant.weights(), items(&expected["weights"]).len())
                .into_iter()
                .map(|(a, _)| a),
        );
    }
    let assets = model.asset_keys();
    assert_eq!(
        json!(assets.iter().map(|a| a.0).collect::<Vec<_>>()),
        case["assets"]
    );
    for asset in assets {
        assert!(
            stored_assets
                .iter()
                .any(|stored| std::ptr::eq(*stored, asset))
        );
    }
    assert_eq!(model.state_key(), before);
}

#[test]
fn model_inspection_and_asset_union_match_lean() {
    let data = fixtures();
    assert_eq!(items(&data["models"]).len(), 288);
    let mut checked = 0;
    for case in items(&data["models"]) {
        if text(&case["value"]).parse::<u64>().is_ok() {
            check_model(case, |s| s.parse::<u64>().unwrap());
            checked += 1;
        }
    }
    assert_eq!(checked, 192);
}

#[cfg(feature = "bigint")]
#[test]
fn generic_inspection_preserves_biguint_payloads_beyond_u64() {
    for case in items(&fixtures()["models"]) {
        check_model(case, |s| s.parse::<num_bigint::BigUint>().unwrap());
    }
}
