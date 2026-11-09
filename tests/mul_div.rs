//! Full-width mul-div with an explicit rounding direction.
//!
//! The Lean theorems `MulDivRefinement.quantity_refines` and
//! `EvaluationRefinement.evaluate_refines` cover the u64 implementation and the
//! evaluator for every input. These tests exercise the public API end to end,
//! cross-check against an independent wide-integer reference, and pin the
//! serialized form.

use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity,
    QuantityExpression as Q, Rate, Rounding, basket,
};
use proptest::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
enum Asset {
    Credit,
    Share,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
enum AccountId {
    Treasury,
    Trader,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
enum Role {
    Treasury,
    Trader,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
enum RateId {
    Charge,
    Credit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Test {}

impl axionomy::Ontology for Test {
    const NAME: &'static str = "test";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}

type World = Economy<Test>;

/// Independent reference: a u64 × u64 product and its quotient fit u128 exactly.
fn reference(left: u64, right: u64, denominator: u64, rounding: Rounding) -> Option<u128> {
    if denominator == 0 {
        return None;
    }
    let product = u128::from(left) * u128::from(right);
    let divisor = u128::from(denominator);
    let mut quotient = product / divisor;
    if rounding == Rounding::Ceil && product % divisor != 0 {
        quotient += 1;
    }
    Some(quotient)
}

fn mul_div(left: u64, right: u64, denominator: u64, rounding: Rounding) -> Option<u128> {
    Quantity::new(left)
        .checked_mul_div(Quantity::new(right), Quantity::new(denominator), rounding)
        .map(|quotient| u128::try_from(quotient.value()).unwrap())
}

fn quantity(value: u64) -> Quantity {
    Quantity::new(value)
}

fn rounding() -> impl Strategy<Value = Rounding> {
    prop_oneof![Just(Rounding::Floor), Just(Rounding::Ceil)]
}

fn edge() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0),
        Just(1),
        Just(2),
        Just(u64::MAX),
        Just(u64::MAX - 1),
        Just(u64::MAX / 2),
        Just(1 << 32),
        any::<u64>(),
    ]
}

proptest! {
    #[test]
    fn u64_matches_wide_reference(left in edge(), right in edge(), denominator in edge(), rounding in rounding()) {
        prop_assert_eq!(
            mul_div(left, right, denominator, rounding),
            reference(left, right, denominator, rounding)
        );
    }

    #[test]
    fn ceiling_is_floor_or_one_more(left in any::<u32>(), right in any::<u32>(), denominator in 1_u64..) {
        let (left, right) = (u64::from(left), u64::from(right));
        let floor = mul_div(left, right, denominator, Rounding::Floor).unwrap();
        let ceil = mul_div(left, right, denominator, Rounding::Ceil).unwrap();
        let exact = (u128::from(left) * u128::from(right)) % u128::from(denominator) == 0;
        prop_assert_eq!(ceil, if exact { floor } else { floor + 1 });
    }
}

#[test]
fn intermediate_products_never_overflow() {
    let max = Quantity::MAX;
    let below = max.checked_sub(quantity(1)).unwrap();
    assert_eq!(max.checked_mul(max), None);
    assert_eq!(max.checked_mul_div(max, max, Rounding::Floor), Some(max));
    assert_eq!(max.checked_mul_div(max, max, Rounding::Ceil), Some(max));
    // Only the rounded quotient must fit.
    let mul_div = |left: Quantity, right, denominator, rounding| {
        left.checked_mul_div(quantity(right), quantity(denominator), rounding)
    };
    assert_eq!(mul_div(max, 2, 1, Rounding::Floor), None);
    assert_eq!(mul_div(max, 2, 2, Rounding::Floor), Some(max));
    assert_eq!(max.checked_mul_div(below, max, Rounding::Ceil), Some(below));
    assert_eq!(mul_div(max, 3, 2, Rounding::Floor), None);
    assert_eq!(mul_div(max, 1, 1, Rounding::Ceil), Some(max));
    assert_eq!(mul_div(quantity(7), 1, 0, Rounding::Floor), None);
}

fn fee(rounding: Rounding) -> Q<Test> {
    // fee = units × bps ÷ 10_000, rounded in the requested direction.
    Q::mul_div(
        Q::units(),
        Q::parameter("bps"),
        Q::parameter("scale"),
        rounding,
    )
}

fn world(rounding: Rounding) -> World {
    let charge = fee(rounding);
    EconomyBuilder::new()
        .account(
            AccountId::Treasury,
            Account::from(basket([(Asset::Credit, 0)])),
        )
        .account(
            AccountId::Trader,
            Account::from(basket([(Asset::Credit, 1_000)])),
        )
        .rate(
            RateId::Charge,
            Rate::new()
                .consume_computed(Role::Trader, Asset::Credit, charge.clone())
                .produce_computed(Role::Treasury, Asset::Credit, charge)
                .distinct(Role::Treasury, Role::Trader),
        )
        .rate(
            RateId::Credit,
            Rate::new().produce_computed(
                Role::Trader,
                Asset::Share,
                Q::divide(Q::parameter("bps"), Q::parameter("scale"), Rounding::Ceil),
            ),
        )
        .invariant(LinearInvariant::new("credit").weight(Asset::Credit, 1))
        .build()
        .unwrap()
}

fn charge(units: u64, bps: u64, scale: u64) -> Exchange<Test> {
    charge_exact(quantity(units), quantity(bps), quantity(scale))
}

fn charge_exact(units: Quantity, bps: Quantity, scale: Quantity) -> Exchange<Test> {
    Exchange::new(RateId::Charge, units)
        .bind(Role::Treasury, AccountId::Treasury)
        .bind(Role::Trader, AccountId::Trader)
        .parameter("bps", bps)
        .parameter("scale", scale)
}

#[test]
fn fees_round_in_the_declared_direction_and_conserve_credit() {
    // 7 units at 30 bps is 0.021 credits: the payer is charged one credit
    // when rounding up, and nothing when rounding down.
    let mut ceil = world(Rounding::Ceil);
    ceil.apply(charge(7, 30, 10_000)).unwrap();
    assert_eq!(
        ceil.balance(&AccountId::Treasury, &Asset::Credit),
        Quantity::new(1)
    );
    assert_eq!(
        ceil.balance(&AccountId::Trader, &Asset::Credit),
        Quantity::new(999)
    );

    let mut floor = world(Rounding::Floor);
    floor.apply(charge(7, 30, 10_000)).unwrap();
    assert_eq!(
        floor.balance(&AccountId::Treasury, &Asset::Credit),
        Quantity::ZERO
    );
    assert_eq!(
        floor.balance(&AccountId::Trader, &Asset::Credit),
        Quantity::new(1_000)
    );

    // Exact charges agree in both directions.
    let mut exact = world(Rounding::Ceil);
    exact.apply(charge(10_000, 30, 10_000)).unwrap();
    assert_eq!(
        exact.balance(&AccountId::Treasury, &Asset::Credit),
        Quantity::new(30)
    );
}

#[test]
fn divide_rounds_in_the_named_direction() {
    let mut world = world(Rounding::Ceil);
    let credit = Exchange::new(RateId::Credit, Quantity::new(1))
        .bind(Role::Trader, AccountId::Trader)
        .parameter("bps", Quantity::new(7))
        .parameter("scale", Quantity::new(2));
    world.apply(credit).unwrap();
    assert_eq!(
        world.balance(&AccountId::Trader, &Asset::Share),
        Quantity::new(4)
    );
    assert_eq!(
        Q::<Test>::divide(Q::constant(7), Q::constant(2), Rounding::Ceil),
        Q::mul_div(
            Q::constant(7),
            Q::constant(1),
            Q::constant(2),
            Rounding::Ceil
        )
    );
}

fn evaluation_error(world: &World, exchange: &Exchange<Test>) -> String {
    let before = world.state_key();
    let assessment = world.assess(exchange);
    assert_eq!(world.state_key(), before);
    match assessment.issues() {
        [ApplyError::RateLawEvaluation { message, .. }] => message.clone(),
        other => panic!("expected a rate-law evaluation error, got {other:?}"),
    }
}

#[test]
fn zero_denominator_and_quotient_overflow_are_reported_without_mutation() {
    let world = world(Rounding::Ceil);
    assert_eq!(
        evaluation_error(&world, &charge(7, 30, 0)),
        "division by zero"
    );
    assert_eq!(
        evaluation_error(
            &world,
            &charge_exact(Quantity::MAX, quantity(3), quantity(2))
        ),
        "multiplication overflow"
    );
    // A huge product with a fitting quotient is fine.
    assert!(world.is_applicable(&charge_exact(Quantity::MAX, quantity(1), Quantity::MAX)));
}

#[test]
fn operand_errors_take_precedence_in_left_to_right_order() {
    let world = |expression: Q<Test>| {
        EconomyBuilder::<Test>::new()
            .account(AccountId::Trader, Account::default())
            .rate(
                RateId::Credit,
                Rate::new().produce_computed(Role::Trader, Asset::Share, expression),
            )
            .build()
            .unwrap()
    };
    let exchange =
        Exchange::new(RateId::Credit, Quantity::new(1)).bind(Role::Trader, AccountId::Trader);
    let underflow = || Q::constant(0) - Q::constant(1);
    let overflow = || {
        Q::Constant {
            value: Quantity::MAX,
        } + Q::constant(1)
    };
    let error = |expression| evaluation_error(&world(expression), &exchange);

    assert_eq!(
        error(Q::mul_div(
            underflow(),
            overflow(),
            Q::constant(0),
            Rounding::Floor
        )),
        "subtraction underflow"
    );
    assert_eq!(
        error(Q::mul_div(
            Q::constant(1),
            overflow(),
            underflow(),
            Rounding::Ceil
        )),
        "addition overflow"
    );
    assert_eq!(
        error(Q::mul_div(
            Q::constant(1),
            Q::constant(1),
            underflow(),
            Rounding::Ceil
        )),
        "subtraction underflow"
    );
    // A zero denominator is reported before the quotient range is checked.
    assert_eq!(
        error(Q::mul_div(
            Q::Constant {
                value: Quantity::MAX
            },
            Q::constant(2),
            Q::constant(0),
            Rounding::Floor
        )),
        "division by zero"
    );
}

#[test]
fn discovery_reads_every_operand() {
    let expression = Q::mul_div(
        Q::balance(Role::Treasury, Asset::Credit),
        Q::parameter("left"),
        Q::parameter("denominator") + Q::balance(Role::Trader, Asset::Share),
        Rounding::Floor,
    );
    let rate = Rate::<Test>::new().produce_computed(Role::Trader, Asset::Credit, expression);
    let roles = rate.roles().copied().collect::<Vec<_>>();
    assert!(roles.contains(&Role::Treasury) && roles.contains(&Role::Trader));
    assert_eq!(
        rate.parameter_names().into_iter().collect::<Vec<_>>(),
        ["denominator", "left"]
    );
    let world = EconomyBuilder::<Test>::new()
        .rate(RateId::Credit, rate)
        .build()
        .unwrap();
    assert_eq!(
        world.asset_keys().into_iter().copied().collect::<Vec<_>>(),
        [Asset::Credit, Asset::Share]
    );
}

#[test]
fn serialized_form_names_the_rounding_direction() {
    let expression: Q<Test> = Q::mul_div(
        Q::units(),
        Q::parameter("bps"),
        Q::constant(10_000),
        Rounding::Ceil,
    );
    let json = serde_json::to_value(&expression).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "mul_div",
            "left": {"kind": "units"},
            "right": {"kind": "parameter", "name": "bps"},
            "denominator": {"kind": "constant", "value": "10000"},
            "rounding": "ceil",
        })
    );
    assert_eq!(serde_json::from_value::<Q<Test>>(json).unwrap(), expression);
    assert_eq!(serde_json::to_value(Rounding::Floor).unwrap(), "floor");
    assert!(serde_json::from_value::<Rounding>(serde_json::json!("half_even")).is_err());
    let missing = serde_json::json!({
        "kind": "mul_div",
        "left": {"kind": "units"},
        "right": {"kind": "units"},
        "denominator": {"kind": "units"},
    });
    assert!(serde_json::from_value::<Q<Test>>(missing).is_err());
}

#[test]
fn json_schema_exposes_the_rounding_enum() {
    let schema = serde_json::to_string(&schemars::schema_for!(Q<Test>)).unwrap();
    assert!(schema.contains("mul_div"));
    assert!(schema.contains("\"floor\"") && schema.contains("\"ceil\""));
}
