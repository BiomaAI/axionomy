use axionomy::path;
use axionomy::{Account, AssetAmount, Basket, EconomyBuilder, Goal, Quantity, Rate};
use axionomy_units::si::energy::joule;
use axionomy_units::si::mass::{gram, kilogram};
use axionomy_units::si::rational64::{Energy, Mass, Torque};
use axionomy_units::si::torque::newton_meter;
use axionomy_units::{
    AssetDefinition, AssetSchema, AssetSchemaError, Rational, UnitAsset, UnitBindingError,
    UnitModelBuildError,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum AssetId {
    Cargo,
    Permission,
    Work,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Depot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Store,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Depot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Depot {}

impl axionomy::Ontology for Depot {
    const NAME: &'static str = "depot";
    type Account = AccountId;
    type Asset = UnitAsset<AssetId>;
    type Rate = RateId;
    type Role = Role;
}

#[test]
fn compatible_input_units_normalize_to_one_canonical_asset() {
    let mut schema = AssetSchema::new();
    let cargo = schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    let from_kilograms: AssetAmount<UnitAsset<AssetId>> = cargo
        .encode(Mass::new::<kilogram>(Rational::from_integer(1)))
        .unwrap();
    let from_grams: AssetAmount<UnitAsset<AssetId>> = cargo
        .encode(Mass::new::<gram>(Rational::from_integer(1_000)))
        .unwrap();

    assert_eq!(from_kilograms, from_grams);
    assert_eq!(from_kilograms.quantity(), Quantity::new(1_000));
}

#[test]
fn atoms_read_back_as_exact_physical_values() {
    let mut schema = AssetSchema::new();
    let cargo = schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    // Take 100 g from 1 kg: the kernel subtracts atoms, and the rest reads
    // back as exactly nine tenths of a kilogram.
    let held = cargo
        .encode(Mass::new::<kilogram>(Rational::from_integer(1)))
        .unwrap()
        .quantity();
    let taken = cargo
        .encode(Mass::new::<gram>(Rational::from_integer(100)))
        .unwrap()
        .quantity();
    let left = held.checked_sub(taken).unwrap();
    assert_eq!(left, Quantity::new(900));
    let mass = cargo.decode(left).unwrap();
    assert_eq!(mass.get::<kilogram>(), Rational::new(9, 10));
    assert_eq!(mass.get::<gram>(), Rational::from_integer(900));
    assert_eq!(cargo.encode(mass).unwrap().quantity(), left);
    // More atoms than a physical value can hold are refused, not wrapped.
    assert_eq!(
        cargo.decode(Quantity::MAX).unwrap_err(),
        UnitBindingError::NumericRange
    );
}

#[test]
fn one_logical_id_cannot_be_declared_with_two_atomic_bases() {
    let mut schema = AssetSchema::new();
    schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    let error = schema
        .define_measure(
            AssetId::Cargo,
            Mass::new::<kilogram>(Rational::from_integer(1)),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        AssetSchemaError::ConflictingDefinition {
            id: AssetId::Cargo,
            ..
        }
    ));
}

#[test]
fn discrete_and_measured_definitions_cannot_share_an_id() {
    let mut schema = AssetSchema::new();
    schema.define_discrete(AssetId::Permission).unwrap();
    let error = schema
        .define_measure(
            AssetId::Permission,
            Mass::new::<gram>(Rational::from_integer(1)),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        AssetSchemaError::ConflictingDefinition {
            id: AssetId::Permission,
            ..
        }
    ));
}

#[test]
fn measurement_kind_is_part_of_the_definition() {
    let mut schema = AssetSchema::new();
    schema
        .define_measure(
            AssetId::Work,
            Energy::new::<joule>(Rational::from_integer(1)),
        )
        .unwrap();
    let error = schema
        .define_measure(
            AssetId::Work,
            Torque::new::<newton_meter>(Rational::from_integer(1)),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        AssetSchemaError::ConflictingDefinition {
            id: AssetId::Work,
            ..
        }
    ));
}

#[test]
fn independently_conflicting_bindings_never_alias_and_fail_model_validation() {
    let mut gram_schema = AssetSchema::new();
    let grams = gram_schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    let mut kilogram_schema = AssetSchema::new();
    let kilograms = kilogram_schema
        .define_measure(
            AssetId::Cargo,
            Mass::new::<kilogram>(Rational::from_integer(1)),
        )
        .unwrap();

    assert_ne!(grams.asset(), kilograms.asset());

    let foreign = Basket::try_from_amounts([kilograms.asset().atoms(1)]).unwrap();
    let error = gram_schema
        .build_economy(
            EconomyBuilder::<Depot>::new().account(AccountId::Depot, Account::from(foreign)),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        UnitModelBuildError::Schema(AssetSchemaError::ConflictingDefinition {
            id: AssetId::Cargo,
            ..
        })
    ));
}

#[test]
fn schema_validates_assets_across_accounts_rates_invariants_and_goals() {
    let mut schema = AssetSchema::new();
    let cargo = schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    let permission = schema.define_discrete(AssetId::Permission).unwrap();
    let balances = Basket::try_from_amounts([
        cargo
            .encode(Mass::new::<kilogram>(Rational::from_integer(2)))
            .unwrap(),
        permission.atoms(1),
    ])
    .unwrap();
    let required = Basket::try_from_amounts([permission.atoms(1)]).unwrap();
    let world = schema
        .build_economy(
            EconomyBuilder::<Depot>::new()
                .account(AccountId::Depot, Account::from(balances))
                .rate(
                    RateId::Store,
                    Rate::new().preserve(Role::Depot, required.clone()),
                )
                .invariant(
                    axionomy::LinearInvariant::new(path!("cargo")).weight(cargo.asset().clone(), 1),
                ),
        )
        .unwrap();
    let goal = Goal::<Depot>::new().require(AccountId::Depot, required);

    assert_eq!(world.asset_keys().len(), 2);
    schema.validate_goal(&goal).unwrap();
}

#[test]
fn asset_and_schema_serde_preserve_the_atomic_definition() {
    let mut schema = AssetSchema::new();
    let cargo = schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();

    let asset_json = serde_json::to_string(cargo.asset()).unwrap();
    let decoded_asset: UnitAsset<AssetId> = serde_json::from_str(&asset_json).unwrap();
    assert_eq!(&decoded_asset, cargo.asset());

    let schema_json = serde_json::to_string(&schema).unwrap();
    let decoded_schema: AssetSchema<AssetId> = serde_json::from_str(&schema_json).unwrap();
    assert_eq!(
        decoded_schema.definition(&AssetId::Cargo),
        schema.definition(&AssetId::Cargo)
    );
    assert!(matches!(
        decoded_asset.definition(),
        AssetDefinition::Measured(_)
    ));
}

#[test]
fn malformed_non_positive_atomic_basis_is_rejected_by_serde() {
    let invalid = r#"{
        "id":"Cargo",
        "definition":{"Measured":{
            "dimension":{"length":0,"mass":1,"time":0,"electric_current":0,"thermodynamic_temperature":0,"amount_of_substance":0,"luminous_intensity":0},
            "kind":"uom:plain",
            "atomic_base_value":[0,1]
        }}
    }"#;

    assert!(serde_json::from_str::<UnitAsset<AssetId>>(invalid).is_err());
}

#[test]
fn malformed_measurement_kind_is_rejected_by_serde() {
    let invalid = r#"{
        "id":"Cargo",
        "definition":{"Measured":{
            "dimension":{"length":0,"mass":1,"time":0,"electric_current":0,"thermodynamic_temperature":0,"amount_of_substance":0,"luminous_intensity":0},
            "kind":"",
            "atomic_base_value":[1,1000]
        }}
    }"#;

    assert!(serde_json::from_str::<UnitAsset<AssetId>>(invalid).is_err());
}

#[test]
fn embedded_definitions_reconstruct_a_schema_and_reject_conflicts() {
    let mut grams_schema = AssetSchema::new();
    let grams = grams_schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    let mut kilograms_schema = AssetSchema::new();
    let kilograms = kilograms_schema
        .define_measure(
            AssetId::Cargo,
            Mass::new::<kilogram>(Rational::from_integer(1)),
        )
        .unwrap();
    let balances =
        Basket::try_from_amounts([grams.asset().atoms(1), kilograms.asset().atoms(1)]).unwrap();
    let world = EconomyBuilder::<Depot>::new()
        .account(AccountId::Depot, Account::from(balances))
        .build()
        .unwrap();

    assert!(matches!(
        AssetSchema::from_economy(&world),
        Err(AssetSchemaError::ConflictingDefinition {
            id: AssetId::Cargo,
            ..
        })
    ));
}

proptest! {
    #[test]
    fn whole_gram_values_lower_exactly(grams in 0_u64..1_000_000) {
        let mut schema = AssetSchema::new();
        let cargo = schema
            .define_measure(
                AssetId::Cargo,
                Mass::new::<gram>(Rational::from_integer(1)),
            )
            .unwrap();
        let amount: AssetAmount<UnitAsset<AssetId>> = cargo
            .encode(Mass::new::<gram>(Rational::from_integer(grams as i64)))
            .unwrap();

        prop_assert_eq!(amount.quantity(), Quantity::new(grams));
        // Decoding is the exact inverse of encoding.
        let mass = cargo.decode(amount.quantity()).unwrap();
        prop_assert_eq!(mass.get::<gram>(), Rational::from_integer(grams as i64));
        prop_assert_eq!(cargo.encode(mass).unwrap(), amount);
    }
}
