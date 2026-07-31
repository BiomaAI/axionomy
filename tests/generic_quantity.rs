#![cfg(feature = "bigint")]

use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity, Rate,
};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Asset {
    Raw,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum AccountId {
    Workshop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum RateId {
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Role {
    Workshop,
}

type World = Economy<AccountId, Asset, RateId, Role, BigUint>;

fn quantity(value: impl Into<BigUint>) -> Quantity<BigUint> {
    Quantity::try_from_scalar(value.into()).expect("BigUint is non-negative")
}

#[test]
fn economy_supports_non_copy_unbounded_quantities_and_signed_invariants() {
    let initial = BigUint::from(u64::MAX) + BigUint::from(2_u8);
    let balances = Basket::from([(Asset::Raw, quantity(initial.clone()))]);
    let rate = Rate::new()
        .consume(Role::Workshop, Basket::from([(Asset::Raw, quantity(1_u8))]))
        .produce(
            Role::Workshop,
            Basket::from([(Asset::Finished, quantity(1_u8))]),
        );
    let mut world: World = EconomyBuilder::new()
        .account(AccountId::Workshop, Account::from(balances))
        .rate(RateId::Build, rate)
        .invariant(
            LinearInvariant::new("material")
                .weight(Asset::Raw, 1)
                .weight(Asset::Finished, 1),
        )
        .build()
        .expect("test model is valid");

    let action =
        Exchange::new(RateId::Build, quantity(2_u8)).bind(Role::Workshop, AccountId::Workshop);
    let assessment = world.assess(&action);
    assert!(assessment.is_applicable());

    let receipt = world.apply(action).unwrap();
    assert_eq!(receipt.deltas().len(), 1);
    assert_eq!(
        world.balance(&AccountId::Workshop, &Asset::Raw),
        quantity(initial - BigUint::from(2_u8))
    );
    assert_eq!(
        world.balance(&AccountId::Workshop, &Asset::Finished),
        quantity(2_u8)
    );

    let encoded = serde_json::to_string(&world).unwrap();
    let decoded: World = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.state_key(), world.state_key());
}
