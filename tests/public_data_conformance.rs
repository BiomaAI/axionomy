//! Public constructors and conversions against the executable Lean contracts.
use axionomy::{Account, AssetAmount, Basket, Exchange, Quantity, QuantityExpression};
use serde::Deserialize;
use serde_json::{Value, json};

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

type Expression = QuantityExpression<Role, Asset>;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../verification/conformance.json")).unwrap()
}

fn quantity(value: &Value) -> Quantity {
    Quantity::new(value.as_str().unwrap().parse().unwrap())
}

fn amounts(value: &Value) -> Basket<Asset> {
    Basket::try_from_entries(
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (Asset(entry[0].as_u64().unwrap()), quantity(&entry[1]))),
    )
    .unwrap()
}

fn observed(value: &Basket<Asset>) -> Value {
    json!(
        value
            .iter()
            .map(|(asset, quantity)| (asset.0, quantity.to_string()))
            .collect::<Vec<_>>()
    )
}

#[test]
fn typed_value_round_trips_and_updates_match_lean() {
    let document = fixtures();
    let cases = document["publicData"]["values"].as_array().unwrap();
    assert_eq!(cases.len(), 18);
    for case in cases {
        let asset = Asset(case["asset"].as_u64().unwrap());
        let expected = quantity(&case["value"]);
        let scalar = expected.get();
        assert_eq!(Quantity::from(scalar), expected);
        assert_eq!(
            Quantity::try_from_scalar(scalar).unwrap().into_scalar(),
            scalar
        );
        assert_eq!(*expected.as_scalar(), scalar);
        if let Ok(small) = u32::try_from(scalar) {
            assert_eq!(Quantity::from(small), expected);
        }
        let value = AssetAmount::new(asset, expected);
        assert_eq!((*value.asset(), *value.quantity()), (asset, expected));
        let parts: (Asset, Quantity) = value.into();
        assert_eq!(AssetAmount::from(parts).into_parts(), (asset, expected));
        let mut balances = amounts(&case["before"]);
        let holder = Account::new(balances.clone());
        assert_eq!(observed(holder.balances()), case["before"]);
        assert_eq!(
            Account::from(holder.into_balances()),
            Account::from(balances.clone())
        );
        let old = balances.insert_amount(AssetAmount::from(parts));
        assert_eq!(json!(old.map(|q| q.to_string())), case["old"]);
        assert_eq!(observed(&balances), case["after"]);
        let mut reversed = balances.iter().map(|(a, q)| (*a, *q)).collect::<Vec<_>>();
        reversed.reverse();
        let reordered = Basket::try_from_entries(reversed).unwrap();
        assert_eq!(Account::from(balances), Account::from(reordered));
    }
    assert!(Account::<Asset>::default().into_balances().is_empty());
}

#[test]
fn proposal_overwrites_and_typed_fields_match_lean() {
    let document = fixtures();
    let cases = document["publicData"]["proposals"].as_array().unwrap();
    assert_eq!(cases.len(), 120);
    for case in cases {
        let rate = RuleId(case["rate"].as_u64().unwrap());
        let units = quantity(&case["units"]);
        let mut proposal = Exchange::<RuleId, Role, Holder>::new(rate, units);
        assert!(proposal.bindings().is_empty());
        assert!(proposal.parameters().is_empty());
        for entry in case["bindings"].as_array().unwrap() {
            proposal = proposal.bind(
                Role(entry[0].as_u64().unwrap()),
                Holder(entry[1].as_u64().unwrap()),
            );
        }
        for entry in case["parameters"].as_array().unwrap() {
            proposal =
                proposal.parameter(entry[0].as_str().unwrap().to_owned(), quantity(&entry[1]));
        }
        assert_eq!(*proposal.rate(), rate);
        assert_eq!(*proposal.units(), units);
        assert_eq!(
            json!(
                proposal
                    .bindings()
                    .iter()
                    .map(|(r, h)| (r.0, h.0))
                    .collect::<Vec<_>>()
            ),
            case["expectedBindings"]
        );
        assert_eq!(
            json!(
                proposal
                    .parameters()
                    .iter()
                    .map(|(n, q)| (n, q.to_string()))
                    .collect::<Vec<_>>()
            ),
            case["expectedParameters"]
        );
    }
}

// Rebuild every fixture tree exclusively through public constructors. The Lean
// construct_refines/evaluate_constructed theorems connect this construction to
// the same expression specification used by the evaluator's conformance cases.
fn construct(expression: Expression) -> Expression {
    match expression {
        Expression::Constant { value } => Expression::constant(value.get()),
        Expression::Units => Expression::units(),
        Expression::Parameter { name } => Expression::parameter(name),
        Expression::Balance { role, asset } => Expression::balance(role, asset),
        Expression::Add { left, right } => {
            let left = construct(*left);
            let right = construct(*right);
            let result = Expression::plus(left.clone(), right.clone());
            assert_eq!(left + right, result);
            result
        }
        Expression::Subtract { left, right } => {
            Expression::subtract(construct(*left), construct(*right))
        }
        Expression::Multiply { left, right } => {
            Expression::multiply(construct(*left), construct(*right))
        }
        Expression::DivideFloor {
            numerator,
            denominator,
        } => Expression::divide_floor(construct(*numerator), construct(*denominator)),
        Expression::Minimum { left, right } => {
            Expression::minimum(construct(*left), construct(*right))
        }
        Expression::Maximum { left, right } => {
            Expression::maximum(construct(*left), construct(*right))
        }
        Expression::MulDiv {
            left,
            right,
            denominator,
            rounding,
        } => Expression::mul_div(
            construct(*left),
            construct(*right),
            construct(*denominator),
            rounding,
        ),
    }
}

#[test]
fn expression_constructors_preserve_lean_trees() {
    let document = fixtures();
    let cases = document["evaluation"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        for field in ["expression", "conditionRight"] {
            let expected: Expression = serde_json::from_value(case[field].clone()).unwrap();
            assert_eq!(construct(expected.clone()), expected);
        }
    }
}
