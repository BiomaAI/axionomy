use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, LinearInvariant, ModelIssue, Quantity,
    Rate, Trace, basket,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Asset {
    Raw,
    Finished,
    Tool,
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

type World = Economy<AccountId, Asset, RateId, Role>;

fn world() -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Workshop,
            Account::from(basket([(Asset::Raw, 2), (Asset::Tool, 1)])),
        )
        .rate(
            RateId::Build,
            Rate::new()
                .consume(Role::Workshop, basket([(Asset::Raw, 2)]))
                .preserve(Role::Workshop, basket([(Asset::Tool, 1)]))
                .produce(Role::Workshop, basket([(Asset::Finished, 1)])),
        )
        .invariant(
            LinearInvariant::new("material")
                .weight(Asset::Raw, 1)
                .weight(Asset::Finished, 2),
        )
        .build()
        .expect("test model is valid")
}

#[test]
fn economy_and_trace_round_trip_directly_through_serde() {
    let world = world();
    let encoded = serde_json::to_string_pretty(&world).unwrap();
    let decoded: World = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.state_key(), world.state_key());
    assert_eq!(
        decoded
            .rate(&RateId::Build)
            .unwrap()
            .roles()
            .copied()
            .collect::<Vec<_>>(),
        vec![Role::Workshop]
    );

    let mut trace = Trace::new();
    trace.push(
        Exchange::new(RateId::Build, Quantity::new(1)).bind(Role::Workshop, AccountId::Workshop),
    );
    let trace_json = serde_json::to_string(&trace).unwrap();
    let decoded_trace: Trace<RateId, Role, AccountId> = serde_json::from_str(&trace_json).unwrap();
    assert_eq!(decoded_trace, trace);
    assert!(decoded.replayed(&decoded_trace).is_ok());
}

#[test]
fn basket_deserialization_rejects_duplicates_and_canonicalizes_zero() {
    let duplicate = r#"[["Raw",1],["Raw",2]]"#;
    assert!(serde_json::from_str::<Basket<Asset>>(duplicate).is_err());

    let canonical: Basket<Asset> = serde_json::from_str(r#"[["Raw",0],["Tool",1]]"#).unwrap();
    assert_eq!(canonical.len(), 1);
    assert_eq!(canonical.quantity(&Asset::Tool), Quantity::new(1));
}

#[test]
fn builder_reports_duplicate_model_identifiers() {
    let result = EconomyBuilder::<AccountId, Asset, RateId, Role>::new()
        .account(AccountId::Workshop, Account::default())
        .account(AccountId::Workshop, Account::default())
        .rate(RateId::Build, Rate::new())
        .rate(RateId::Build, Rate::new())
        .build();

    let error = result.unwrap_err();
    assert_eq!(
        error.issues(),
        &[
            ModelIssue::DuplicateAccount {
                account: AccountId::Workshop,
            },
            ModelIssue::DuplicateRate {
                rate: RateId::Build,
            },
        ]
    );
}
