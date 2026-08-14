use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity,
    QuantityComparison, QuantityExpression as Q, Rate, basket,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Asset {
    Energy,
    Credit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum AccountId {
    Pool,
    Trader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Role {
    Pool,
    Trader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum RateId {
    BuyEnergy,
}

type World = Economy<AccountId, Asset, RateId, Role>;
type Action = Exchange<RateId, Role, AccountId>;

fn output_expression() -> Q<Role, Asset> {
    // floor(pool_energy * 997 * input / (pool_credit * 1000 + 997 * input))
    let input_after_fee = Q::multiply(Q::constant(997), Q::units());
    Q::divide_floor(
        Q::multiply(
            Q::balance(Role::Pool, Asset::Energy),
            input_after_fee.clone(),
        ),
        Q::plus(
            Q::multiply(Q::balance(Role::Pool, Asset::Credit), Q::constant(1_000)),
            input_after_fee,
        ),
    )
}

fn world() -> World {
    let output = output_expression();
    EconomyBuilder::new()
        .account(
            AccountId::Pool,
            Account::from(basket([(Asset::Energy, 1_000), (Asset::Credit, 10_000)])),
        )
        .account(
            AccountId::Trader,
            Account::from(basket([(Asset::Credit, 10_000)])),
        )
        .rate(
            RateId::BuyEnergy,
            Rate::new()
                .consume(Role::Trader, basket([(Asset::Credit, 1)]))
                .produce(Role::Pool, basket([(Asset::Credit, 1)]))
                .consume_computed(Role::Pool, Asset::Energy, output.clone())
                .produce_computed(Role::Trader, Asset::Energy, output.clone())
                .condition(
                    "minimum output",
                    output,
                    QuantityComparison::GreaterThanOrEqual,
                    Q::parameter("minimum_output"),
                )
                .distinct(Role::Pool, Role::Trader),
        )
        .invariant(LinearInvariant::new("energy").weight(Asset::Energy, 1))
        .invariant(LinearInvariant::new("credit").weight(Asset::Credit, 1))
        .build()
        .unwrap()
}

fn buy(input: u64, minimum_output: u64) -> Action {
    Exchange::new(RateId::BuyEnergy, Quantity::new(input))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::Trader, AccountId::Trader)
        .parameter("minimum_output", Quantity::new(minimum_output))
}

#[test]
fn state_evaluated_rate_applies_exact_constant_product_swap() {
    let mut world = world();
    let receipt = world.apply(buy(1_000, 90)).unwrap();

    assert_eq!(
        world.balance(&AccountId::Pool, &Asset::Energy),
        Quantity::new(910)
    );
    assert_eq!(
        world.balance(&AccountId::Pool, &Asset::Credit),
        Quantity::new(11_000)
    );
    assert_eq!(
        world.balance(&AccountId::Trader, &Asset::Energy),
        Quantity::new(90)
    );
    assert_eq!(
        world.balance(&AccountId::Trader, &Asset::Credit),
        Quantity::new(9_000)
    );
    assert_eq!(receipt.deltas().len(), 2);

    let product_before = 1_000_u64 * 10_000;
    let product_after = 910_u64 * 11_000;
    assert!(product_after >= product_before);
}

#[test]
fn minimum_output_is_authoritative_and_non_mutating() {
    let world = world();
    let before = world.state_key();
    let assessment = world.assess(&buy(1_000, 91));

    assert!(!assessment.is_applicable());
    assert!(matches!(
        assessment.issues(),
        [ApplyError::RateConditionFailed { condition, .. }] if condition == "minimum output"
    ));
    assert_eq!(world.state_key(), before);
}

#[test]
fn parameters_are_declared_by_the_rate_and_survive_serde() {
    let world = world();
    let encoded = serde_json::to_string(&world).unwrap();
    let decoded: World = serde_json::from_str(&encoded).unwrap();
    assert!(decoded.is_applicable(&buy(1_000, 90)));

    let missing = Exchange::new(RateId::BuyEnergy, Quantity::new(1_000))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::Trader, AccountId::Trader);
    assert!(matches!(
        decoded.assess(&missing).issues(),
        [ApplyError::MissingParameter { name }] if name == "minimum_output"
    ));

    let unknown = buy(1_000, 90).parameter("oracle_price", Quantity::new(10));
    assert!(matches!(
        decoded.assess(&unknown).issues(),
        [ApplyError::UnknownParameter { name }] if name == "oracle_price"
    ));
}
