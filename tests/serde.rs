use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, LinearInvariant, ModelIssue, Quantity,
    Rate, Trace, basket,
};
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    axionomy::Segments,
)]
enum Asset {
    Raw,
    Finished,
    Tool,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    axionomy::Segments,
)]
enum AccountId {
    Workshop,
    Treasury,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    axionomy::Segments,
)]
enum RateId {
    Build,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    axionomy::Segments,
)]
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
            LinearInvariant::new(path!("material"))
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
                rate: axionomy::Pattern::literal(RateId::Build),
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

/// An ontology whose identifiers carry a number, for a rate with a variable.
mod visits {
    use axionomy::{
        Account, Economy, EconomyBuilder, Pattern, Quantity, QuantityExpression, Rate, Relation,
        Variable, basket,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
    pub enum Asset {
        Token,
        Visited(u8),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
    pub enum AccountId {
        Walker,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
    pub enum RateId {
        Visit(u8),
        Step(u8, u8),
        Rest,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
    pub enum Role {
        Walker,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Visits {}

    impl axionomy::Ontology for Visits {
        const NAME: &'static str = "visits";
        type Account = AccountId;
        type Asset = Asset;
        type Rate = RateId;
        type Role = Role;
    }

    pub fn world() -> Economy<Visits> {
        EconomyBuilder::new()
            .account(
                AccountId::Walker,
                Account::from(basket([(Asset::Token, 3)])),
            )
            .rate_at(
                "visit/{place}".parse().unwrap(),
                Rate::new()
                    .variable(Variable::range("place".parse().unwrap(), 1, 4))
                    .account(Role::Walker, Pattern::literal(AccountId::Walker))
                    .consume(Role::Walker, basket([(Asset::Token, 1)]))
                    .produce_at(
                        Role::Walker,
                        [(
                            "visited/{place}".parse::<Pattern<Asset>>().unwrap(),
                            Quantity::new(1),
                        )],
                    ),
            )
            .rate_at(
                "step/{from}/{to}".parse().unwrap(),
                Rate::new()
                    .variable(Variable::range("from".parse().unwrap(), 1, 4))
                    .variable(Variable::range("to".parse().unwrap(), 1, 4))
                    .relation(Relation::new(
                        ["from".parse().unwrap(), "to".parse().unwrap()],
                        [[1, 2], [2, 3], [3, 4]].map(|row| row.map(u64::into)),
                    ))
                    .derive(
                        "cost".parse().unwrap(),
                        QuantityExpression::variable("to".parse().unwrap())
                            - QuantityExpression::variable("from".parse().unwrap()),
                    )
                    .consume_computed_at(
                        Role::Walker,
                        Pattern::literal(Asset::Token),
                        QuantityExpression::variable("cost".parse().unwrap()),
                    )
                    .produce_at(
                        Role::Walker,
                        [(
                            "visited/{to}".parse::<Pattern<Asset>>().unwrap(),
                            Quantity::new(1),
                        )],
                    ),
            )
            .rate(
                RateId::Rest,
                Rate::new().produce(Role::Walker, basket([(Asset::Token, 1)])),
            )
            .build()
            .expect("the visits model is valid")
    }
}

/// A typed economy whose rate has a variable is written down as a `Paths`
/// economy: the same JSON and postcard bytes read either way, and the same
/// instance applied either way reaches the same economy.
#[test]
fn a_typed_rate_with_variables_written_down_is_a_paths_rate() {
    use axionomy::paths;
    use visits::{AccountId, RateId, Role};

    let world = visits::world();
    let json = serde_json::to_value(&world).unwrap();
    let read: paths::Economy = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(&read).unwrap(), json);
    assert_eq!(
        postcard::to_allocvec(&read).unwrap(),
        postcard::to_allocvec(&world).unwrap()
    );
    let decoded: Economy<visits::Visits> =
        postcard::from_bytes(&postcard::to_allocvec(&world).unwrap()).unwrap();
    assert_eq!(decoded.state_key(), world.state_key());

    let mut typed = world.clone();
    let mut untyped = read;
    for (rate, path) in [
        (RateId::Visit(2), path!("visit/2")),
        (RateId::Step(2, 3), path!("step/2/3")),
    ] {
        typed
            .apply(Exchange::new(rate, Quantity::new(1)).bind(Role::Walker, AccountId::Walker))
            .unwrap();
        untyped
            .apply(
                paths::Exchange::new(path, Quantity::new(1)).bind(path!("walker"), path!("walker")),
            )
            .unwrap();
    }
    assert!(!typed.is_applicable(
        &Exchange::new(RateId::Step(1, 3), Quantity::new(1)).bind(Role::Walker, AccountId::Walker)
    ));
    assert_eq!(
        serde_json::to_value(&untyped).unwrap(),
        serde_json::to_value(&typed).unwrap()
    );
}
