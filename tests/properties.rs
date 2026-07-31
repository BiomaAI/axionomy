use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity, Rate, basket,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Asset {
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum AccountId {
    Sender,
    Receiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum RateId {
    Transfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Role {
    Sender,
    Receiver,
}

type World = Economy<AccountId, Asset, RateId, Role>;

fn world(balance: u64) -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Sender,
            Account::from(basket([(Asset::Token, balance)])),
        )
        .account(AccountId::Receiver, Account::default())
        .rate(
            RateId::Transfer,
            Rate::new()
                .consume(Role::Sender, basket([(Asset::Token, 1)]))
                .produce(Role::Receiver, basket([(Asset::Token, 1)]))
                .distinct(Role::Sender, Role::Receiver),
        )
        .invariant(LinearInvariant::new("tokens").weight(Asset::Token, 1))
        .build()
        .unwrap()
}

fn transfer(units: u64) -> Exchange<RateId, Role, AccountId> {
    Exchange::new(RateId::Transfer, Quantity::new(units))
        .bind(Role::Sender, AccountId::Sender)
        .bind(Role::Receiver, AccountId::Receiver)
}

proptest! {
    #[test]
    fn successful_assessment_matches_receipt_and_preserves_conservation(
        balance in 1_u64..10_000,
        requested in 1_u64..10_000,
    ) {
        let units = requested.min(balance);
        let mut world = world(balance);
        let action = transfer(units);
        let assessment = world.assess(&action);
        let projected = assessment.projected_deltas().unwrap();
        let receipt = world.apply(action).unwrap();

        prop_assert_eq!(projected.len(), receipt.deltas().len());
        for (expected, actual) in projected.iter().zip(receipt.deltas()) {
            prop_assert_eq!(expected.account(), actual.account());
            prop_assert_eq!(expected.consumed(), actual.consumed());
            prop_assert_eq!(expected.produced(), actual.produced());
            prop_assert_eq!(expected.preserved(), actual.preserved());
        }
        prop_assert_eq!(
            world.balance(&AccountId::Sender, &Asset::Token),
            Quantity::new(balance - units),
        );
        prop_assert_eq!(
            world.balance(&AccountId::Receiver, &Asset::Token),
            Quantity::new(units),
        );
    }

    #[test]
    fn infeasible_application_is_atomic(
        balance in 0_u64..10_000,
        extra in 1_u64..10_000,
    ) {
        let mut world = world(balance);
        let before = world.state_key();
        let action = transfer(balance.saturating_add(extra));
        let assessment = world.assess(&action);

        prop_assert!(!assessment.is_applicable());
        prop_assert!(world.apply(action).is_err());
        prop_assert_eq!(world.state_key(), before);
    }

    #[test]
    fn direct_serde_round_trip_preserves_canonical_state(balance in 0_u64..10_000) {
        let world = world(balance);
        let encoded = serde_json::to_vec(&world).unwrap();
        let decoded: World = serde_json::from_slice(&encoded).unwrap();

        prop_assert_eq!(decoded.state_key(), world.state_key());
        prop_assert_eq!(decoded.state_fingerprint(), world.state_fingerprint());
    }

    #[test]
    fn checked_quantity_arithmetic_agrees_with_u64(left in any::<u64>(), right in any::<u64>()) {
        let actual = Quantity::new(left).checked_add(&Quantity::new(right));
        let expected = left.checked_add(right).map(Quantity::new);

        prop_assert_eq!(actual, expected);
    }
}
