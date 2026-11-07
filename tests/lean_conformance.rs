//! Finite cross-language regression checks against executable Lean definitions.
//! This is not a proof that the Rust implementation refines those definitions.
use axionomy::{
    Account, AccountError, AssetAmount, Basket, BasketError, EconomyBuilder, Exchange,
    LinearInvariant, Quantity, QuantityComparison, QuantityExpression as Q, Rate, basket,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixtures {
    version: u32,
    scalars: Vec<ScalarCase>,
    exchanges: Vec<ExchangeCase>,
    expressions: Vec<ExpressionCase>,
    baskets: Vec<BasketCase>,
    accounts: Vec<AccountCase>,
}

#[derive(Debug, Deserialize)]
struct AccountCase {
    operation: String,
    before: Vec<(u64, String)>,
    amounts: Vec<(u64, String)>,
    after: Vec<(u64, String)>,
    overflow: Option<u64>,
    shortfall: Option<Vec<(u64, String)>>,
}

#[test]
fn account_updates_match_lean() {
    fn basket(entries: &[(u64, String)]) -> Basket<u64> {
        Basket::try_from_entries(
            entries
                .iter()
                .map(|(asset, value)| (*asset, Quantity::new(number(value)))),
        )
        .unwrap()
    }
    fn entries(basket: &Basket<u64>) -> Vec<(u64, String)> {
        basket
            .iter()
            .map(|(asset, value)| (*asset, value.to_string()))
            .collect()
    }
    let cases = fixtures().accounts;
    assert_eq!(cases.len(), 328);
    for case in cases {
        let mut account = Account::new(basket(&case.before));
        let amounts = basket(&case.amounts);
        let original = account.clone();
        let result = match case.operation.as_str() {
            "deposit" => account.deposit(&amounts),
            "withdraw" => account.withdraw(&amounts),
            other => panic!("unknown Lean account operation: {other}"),
        };
        match (&case.overflow, &case.shortfall, result) {
            (None, None, Ok(())) => {}
            (Some(expected), None, Err(AccountError::Overflow { asset })) => {
                assert_eq!(asset, *expected, "{case:?}");
                assert_eq!(
                    entries(account.balances()),
                    entries(original.balances()),
                    "{case:?}"
                );
            }
            (None, Some(expected), Err(AccountError::InsufficientBalance { shortfall })) => {
                assert_eq!(&entries(&shortfall), expected, "{case:?}");
                assert_eq!(
                    entries(account.balances()),
                    entries(original.balances()),
                    "{case:?}"
                );
            }
            (_, _, actual) => panic!("account result disagrees with Lean: {case:?}: {actual:?}"),
        }
        assert_eq!(entries(account.balances()), case.after, "{case:?}");
    }
}

#[derive(Debug, Deserialize)]
struct BasketCase {
    entries: Vec<(u64, String)>,
    duplicate: Option<u64>,
    canonical: Option<Vec<(u64, String)>>,
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
fn basket_construction_matches_lean() {
    let cases = fixtures().baskets;
    assert_eq!(cases.len(), 260);
    for case in cases {
        let entries = case
            .entries
            .iter()
            .map(|(asset, amount)| (*asset, Quantity::new(number(amount))))
            .collect::<Vec<_>>();
        let actual = Basket::try_from_entries(entries.clone());
        assert_eq!(
            Basket::try_from_amounts(entries.iter().copied().map(AssetAmount::from)),
            actual,
            "{case:?}"
        );
        let decoded = serde_json::from_value::<Basket<u64>>(serde_json::json!(entries));
        match (actual, case.duplicate, &case.canonical) {
            (Err(BasketError::DuplicateAsset { asset }), Some(expected), None) => {
                assert_eq!(asset, expected, "{case:?}");
                assert!(decoded.is_err(), "{case:?}");
            }
            (Ok(basket), None, Some(expected)) => {
                let observed = basket
                    .iter()
                    .map(|(asset, amount)| (*asset, amount.to_string()))
                    .collect::<Vec<_>>();
                assert_eq!(&observed, expected, "{case:?}");
                let decoded = decoded.unwrap();
                assert_eq!(
                    decoded.iter().collect::<Vec<_>>(),
                    basket.iter().collect::<Vec<_>>(),
                    "{case:?}"
                );
            }
            (actual, _, _) => panic!("basket disagrees with Lean: {case:?}: {actual:?}"),
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
