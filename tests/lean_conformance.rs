//! Finite cross-language regression checks against executable Lean definitions.
//! This is not a proof that the Rust implementation refines those definitions.
use axionomy::{
    Account, EconomyBuilder, Exchange, LinearInvariant, Quantity, QuantityComparison,
    QuantityExpression as Q, Rate, basket,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixtures {
    version: u32,
    scalars: Vec<ScalarCase>,
    exchanges: Vec<ExchangeCase>,
    expressions: Vec<ExpressionCase>,
}

#[derive(Debug, Deserialize)]
struct ScalarCase {
    left: String,
    right: String,
    add: Option<String>,
    sub: Option<String>,
    mul: Option<String>,
    div: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExchangeCase {
    before: String,
    consumed: String,
    produced: String,
    preserved: String,
    units: String,
    conserve: bool,
    accepted: bool,
    after: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpressionCase {
    balance: String,
    units: String,
    denominator: String,
    minimum: String,
    evaluated: Option<String>,
    accepted: bool,
    source_after: String,
    sink_after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Asset {
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum AccountId {
    Source,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Source,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum RateId {
    Change,
}

fn fixtures() -> Fixtures {
    let fixtures: Fixtures =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    assert_eq!(fixtures.version, 1);
    fixtures
}

fn number(text: &str) -> u64 {
    text.parse().unwrap()
}

#[test]
fn checked_quantities_match_lean() {
    for case in fixtures().scalars {
        let left = Quantity::new(number(&case.left));
        let right = Quantity::new(number(&case.right));
        for (actual, expected) in [
            (left.checked_add(&right), &case.add),
            (left.checked_sub(&right), &case.sub),
            (left.checked_mul(&right), &case.mul),
            (left.checked_div(&right), &case.div),
        ] {
            assert_eq!(
                actual.map(|q| q.to_string()).as_ref(),
                expected.as_ref(),
                "{case:?}"
            );
        }
    }
}

#[test]
fn static_exchange_assessment_commit_and_receipt_match_lean() {
    for case in fixtures().exchanges {
        let rate = Rate::new()
            .consume(
                Role::Source,
                basket([(Asset::Token, number(&case.consumed))]),
            )
            .produce(
                Role::Source,
                basket([(Asset::Token, number(&case.produced))]),
            )
            .preserve(
                Role::Source,
                basket([(Asset::Token, number(&case.preserved))]),
            );
        let mut builder = EconomyBuilder::new()
            .account(
                AccountId::Source,
                Account::from(basket([(Asset::Token, number(&case.before))])),
            )
            .rate(RateId::Change, rate);
        if case.conserve {
            builder = builder.invariant(LinearInvariant::new("tokens").weight(Asset::Token, 1));
        }
        let mut world = builder.build().unwrap();
        let action = Exchange::new(RateId::Change, Quantity::new(number(&case.units)))
            .bind(Role::Source, AccountId::Source);
        let initial = world.state_key();
        let assessment = world.assess(&action);
        assert_eq!(assessment.is_applicable(), case.accepted, "{case:?}");
        assert_eq!(
            world.state_key(),
            initial,
            "assessment mutated state: {case:?}"
        );
        let outcome = world.apply(action);
        assert_eq!(outcome.is_ok(), case.accepted, "{case:?}");
        assert_eq!(
            world.balance(&AccountId::Source, &Asset::Token).get(),
            number(&case.after),
            "{case:?}"
        );
        if let Ok(receipt) = outcome {
            let delta = &receipt.deltas()[0];
            assert_eq!(
                delta.consumed().quantity(&Asset::Token).get(),
                number(&case.consumed) * number(&case.units)
            );
            assert_eq!(
                delta.produced().quantity(&Asset::Token).get(),
                number(&case.produced) * number(&case.units)
            );
            assert_eq!(
                delta.preserved().quantity(&Asset::Token).get(),
                number(&case.preserved)
            );
        } else {
            assert_eq!(
                world.state_key(),
                initial,
                "rejection mutated state: {case:?}"
            );
        }
    }
}

fn expression() -> Q<Role, Asset> {
    Q::divide_floor(
        Q::multiply(Q::balance(Role::Source, Asset::Token), Q::units()),
        Q::parameter("denominator"),
    )
}

#[test]
fn computed_effects_use_the_same_pre_exchange_snapshot_as_lean() {
    for case in fixtures().expressions {
        // First isolate expression evaluation through a public computed rate.
        let mut evaluator = EconomyBuilder::new()
            .account(
                AccountId::Source,
                Account::from(basket([(Asset::Token, number(&case.balance))])),
            )
            .account(AccountId::Sink, Account::default())
            .rate(
                RateId::Change,
                Rate::new().produce_computed(Role::Sink, Asset::Token, expression()),
            )
            .build()
            .unwrap();
        let action = Exchange::new(RateId::Change, Quantity::new(number(&case.units)))
            .bind(Role::Source, AccountId::Source)
            .bind(Role::Sink, AccountId::Sink)
            .parameter("denominator", Quantity::new(number(&case.denominator)));
        let evaluated = evaluator.apply(action.clone());
        assert_eq!(evaluated.is_ok(), case.evaluated.is_some(), "{case:?}");
        if let Some(expected) = &case.evaluated {
            assert_eq!(
                evaluator.balance(&AccountId::Sink, &Asset::Token).get(),
                number(expected),
                "{case:?}"
            );
        }

        let rate = Rate::new()
            .consume_computed(Role::Source, Asset::Token, expression())
            .produce_computed(Role::Sink, Asset::Token, expression())
            .condition(
                "minimum",
                expression(),
                QuantityComparison::GreaterThanOrEqual,
                Q::parameter("minimum"),
            )
            .distinct(Role::Source, Role::Sink);
        let mut world = EconomyBuilder::new()
            .account(
                AccountId::Source,
                Account::from(basket([(Asset::Token, number(&case.balance))])),
            )
            .account(AccountId::Sink, Account::default())
            .rate(RateId::Change, rate)
            .invariant(LinearInvariant::new("tokens").weight(Asset::Token, 1))
            .build()
            .unwrap();
        let initial = world.state_key();
        let action = action.parameter("minimum", Quantity::new(number(&case.minimum)));
        assert_eq!(
            world.assess(&action).is_applicable(),
            case.accepted,
            "{case:?}"
        );
        assert_eq!(world.apply(action).is_ok(), case.accepted, "{case:?}");
        assert_eq!(
            world.balance(&AccountId::Source, &Asset::Token).get(),
            number(&case.source_after),
            "{case:?}"
        );
        assert_eq!(
            world.balance(&AccountId::Sink, &Asset::Token).get(),
            number(&case.sink_after),
            "{case:?}"
        );
        if !case.accepted {
            assert_eq!(world.state_key(), initial, "{case:?}");
        }
    }
}
