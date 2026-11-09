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
    Treasury,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum RateId {
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum Role {
    Workshop,
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
    let decoded_trace: Trace<Test> = serde_json::from_str(&trace_json).unwrap();
    assert_eq!(decoded_trace, trace);
    assert!(decoded.replayed(&decoded_trace).is_ok());
}

#[test]
fn basket_deserialization_rejects_duplicates_and_canonicalizes_zero() {
    for duplicate in [
        r#"[["Raw","1"],["Raw","2"]]"#,
        r#"[["Raw","0"],["Raw","2"]]"#,
        r#"[["Raw","0"],["Raw","0"]]"#,
        r#"[["Raw","2"],["Raw","0"]]"#,
    ] {
        assert!(serde_json::from_str::<Basket<Asset>>(duplicate).is_err());
    }

    let canonical: Basket<Asset> = serde_json::from_str(r#"[["Raw","0"],["Tool","1"]]"#).unwrap();
    assert_eq!(canonical.len(), 1);
    assert_eq!(canonical.quantity(&Asset::Tool), Quantity::new(1));
}

#[test]
fn builder_reports_duplicate_model_identifiers() {
    let result = EconomyBuilder::<Test>::new()
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

#[test]
fn semantic_state_identity_is_independent_of_construction_order() {
    let first = EconomyBuilder::<Test>::new()
        .account(
            AccountId::Workshop,
            Account::from(basket([(Asset::Raw, 2), (Asset::Tool, 1)])),
        )
        .account(
            AccountId::Treasury,
            Account::from(basket([(Asset::Finished, 3), (Asset::Raw, 1)])),
        )
        .build()
        .unwrap();
    let second = EconomyBuilder::<Test>::new()
        .account(
            AccountId::Treasury,
            Account::from(basket([(Asset::Raw, 1), (Asset::Finished, 3)])),
        )
        .account(
            AccountId::Workshop,
            Account::from(basket([(Asset::Tool, 1), (Asset::Raw, 2)])),
        )
        .build()
        .unwrap();

    assert_eq!(first.state_key(), second.state_key());
    assert_eq!(first.state_fingerprint(), second.state_fingerprint());
}

#[test]
fn observation_keys_require_and_round_trip_visible_account_existence() {
    use axionomy::ObservationKey;
    let world: Economy<Bytes> = EconomyBuilder::new()
        .account(0, Account::default())
        .build()
        .unwrap();
    let key = world.view([0, 1]).observation_key();
    let mut encoded = serde_json::to_value(&key).unwrap();
    assert_eq!(encoded["existing_accounts"], serde_json::json!([0]));
    let decoded: ObservationKey<Bytes> = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded, key);
    encoded.as_object_mut().unwrap().remove("existing_accounts");
    assert!(serde_json::from_value::<ObservationKey<Bytes>>(encoded).is_err());
    let schema = serde_json::to_value(schemars::schema_for!(ObservationKey<Bytes>)).unwrap();
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("existing_accounts"))
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Bytes {}

impl axionomy::Ontology for Bytes {
    const NAME: &'static str = "bytes";
    type Account = u8;
    type Asset = u8;
    type Rate = u8;
    type Role = u8;
}
