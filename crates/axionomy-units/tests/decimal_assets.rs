use axionomy::{Account, AssetAmount, Basket, EconomyBuilder, Quantity};
use axionomy_units::si::mass::gram;
use axionomy_units::si::rational64::Mass;
use axionomy_units::{
    AssetDefinition, AssetSchema, AssetSchemaError, DecimalParseError, DecimalPrecision,
    DecimalPrecisionError, MAX_DECIMALS, Rational, UnitAsset, UnitBindingError,
    UnitModelBuildError,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
enum AssetId {
    Usd,
    Yen,
    Cargo,
    Ticket,
}

type MassMeasure = axionomy_units::MeasuredAsset<
    AssetId,
    axionomy_units::si::mass::Dimension,
    axionomy_units::si::SI<axionomy_units::MeasureScalar>,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Treasury,
}

fn usd() -> (AssetSchema<AssetId>, axionomy_units::DecimalAsset<AssetId>) {
    let mut schema = AssetSchema::new();
    let usd = schema.define_decimal(AssetId::Usd, 2).unwrap();
    (schema, usd)
}

fn atoms(amount: &AssetAmount<UnitAsset<AssetId>>) -> u64 {
    amount.quantity().to_u64()
}

#[test]
fn decimal_text_lowers_to_exact_minor_units_and_formats_back() {
    let (_, usd) = usd();
    let amount = usd.parse("12.50").unwrap();

    assert_eq!(atoms(&amount), 1_250);
    assert_eq!(amount.asset(), usd.asset());
    assert_eq!(usd.format(amount.quantity()), "12.50");
    assert_eq!(usd.format(Quantity::new(7)), "0.07");
    assert_eq!(usd.format(Quantity::ZERO), "0.00");
    assert_eq!(usd.precision().atoms_per_unit(), 100);
}

#[test]
fn zero_decimal_assets_count_whole_units() {
    let mut schema = AssetSchema::new();
    let yen = schema.define_decimal(AssetId::Yen, 0).unwrap();

    assert_eq!(atoms(&yen.parse("1500").unwrap()), 1_500);
    assert_eq!(atoms(&yen.parse("1500.000").unwrap()), 1_500);
    assert_eq!(yen.format(Quantity::new(1_500)), "1500");
    assert_eq!(
        yen.parse("1500.5"),
        Err(UnitBindingError::InexactAtomicConversion.into())
    );
}

#[test]
fn precision_is_part_of_asset_identity() {
    let mut cents = AssetSchema::new();
    let cents = cents.define_decimal(AssetId::Usd, 2).unwrap();
    let mut micros = AssetSchema::new();
    let micros = micros.define_decimal(AssetId::Usd, 6).unwrap();

    assert_ne!(cents.asset(), micros.asset());

    // One logical ID cannot be redeclared with another precision.
    let (mut schema, _) = usd();
    assert!(matches!(
        schema.define_decimal(AssetId::Usd, 6),
        Err(AssetSchemaError::ConflictingDefinition {
            id: AssetId::Usd,
            ..
        })
    ));
    assert!(matches!(
        schema.define_decimal(AssetId::Usd, 2),
        Err(AssetSchemaError::DuplicateAsset { id: AssetId::Usd })
    ));

    // Keys carried by a model cannot silently mix precisions either.
    assert!(matches!(
        AssetSchema::from_assets([cents.asset(), micros.asset()]),
        Err(AssetSchemaError::ConflictingDefinition {
            id: AssetId::Usd,
            ..
        })
    ));

    // A basket keeps the two keys apart instead of adding cents to micros.
    let basket =
        Basket::try_from_amounts([cents.parse("1").unwrap(), micros.parse("1").unwrap()]).unwrap();
    assert_eq!(basket.len(), 2);
    assert_eq!(basket.quantity(cents.asset()), Quantity::new(100));
    assert_eq!(basket.quantity(micros.asset()), Quantity::new(1_000_000));
}

#[test]
fn schema_built_models_reject_a_foreign_precision() {
    let (schema, _) = usd();
    let mut other = AssetSchema::new();
    let micros = other.define_decimal(AssetId::Usd, 6).unwrap();

    let builder = EconomyBuilder::<AccountId, UnitAsset<AssetId>, (), ()>::new().account(
        AccountId::Treasury,
        Account::from(Basket::try_from_amounts([micros.parse("5").unwrap()]).unwrap()),
    );
    assert!(matches!(
        schema.build_economy(builder),
        Err(UnitModelBuildError::Schema(
            AssetSchemaError::ConflictingDefinition {
                id: AssetId::Usd,
                ..
            }
        ))
    ));
}

#[test]
fn precision_above_u64_capacity_is_rejected() {
    let mut schema = AssetSchema::<AssetId>::new();
    assert_eq!(
        schema.define_decimal(AssetId::Usd, MAX_DECIMALS + 1),
        Err(AssetSchemaError::InvalidPrecision(
            DecimalPrecisionError::TooManyDecimals { decimals: 19 }
        ))
    );
    assert!(schema.is_empty());

    let precision = DecimalPrecision::new(MAX_DECIMALS).unwrap();
    assert_eq!(precision.atoms_per_unit(), 1_000_000_000_000_000_000);

    let invalid = r#"{"id":"Usd","definition":{"Decimal":{"decimals":19}}}"#;
    assert!(serde_json::from_str::<UnitAsset<AssetId>>(invalid).is_err());
    let invalid_schema = r#"[["Usd",{"Decimal":{"decimals":200}}]]"#;
    assert!(serde_json::from_str::<AssetSchema<AssetId>>(invalid_schema).is_err());
}

#[test]
fn decimal_definitions_round_trip_through_serde() {
    let (schema, usd) = usd();

    let asset_json = serde_json::to_string(usd.asset()).unwrap();
    let decoded: UnitAsset<AssetId> = serde_json::from_str(&asset_json).unwrap();
    assert_eq!(&decoded, usd.asset());
    assert!(matches!(decoded.definition(), AssetDefinition::Decimal(_)));

    let schema_json = serde_json::to_string(&schema).unwrap();
    let decoded: AssetSchema<AssetId> = serde_json::from_str(&schema_json).unwrap();
    assert_eq!(decoded.decimal(&AssetId::Usd).unwrap(), usd);
}

#[test]
fn handles_are_reconstructed_only_for_matching_kinds() {
    let mut schema = AssetSchema::new();
    let usd = schema.define_decimal(AssetId::Usd, 2).unwrap();
    schema
        .define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))
        .unwrap();
    schema.define_discrete(AssetId::Ticket).unwrap();

    assert_eq!(schema.decimal(&AssetId::Usd).unwrap(), usd);
    assert_eq!(
        schema.decimal(&AssetId::Cargo),
        Err(AssetSchemaError::ExpectedDecimalAsset { id: AssetId::Cargo })
    );
    assert_eq!(
        schema.decimal(&AssetId::Ticket),
        Err(AssetSchemaError::ExpectedDecimalAsset {
            id: AssetId::Ticket
        })
    );
    assert_eq!(
        schema.decimal(&AssetId::Yen),
        Err(AssetSchemaError::UnknownAsset { id: AssetId::Yen })
    );
    let measured: Result<MassMeasure, _> = schema.measured(&AssetId::Usd);
    assert!(matches!(
        measured,
        Err(AssetSchemaError::ExpectedMeasuredAsset { id: AssetId::Usd })
    ));
}

#[test]
fn malformed_decimal_text_is_rejected() {
    let (_, usd) = usd();
    for text in [
        "", ".", "1.", ".5", "-1", "+1", "1e2", " 1", "1 ", "1,000", "1_000", "1.2.3", "0x10", "١",
    ] {
        assert_eq!(
            usd.parse(text),
            Err(DecimalParseError::InvalidSyntax),
            "{text:?}"
        );
    }
}

#[test]
fn excess_precision_is_rejected_unless_it_is_zero() {
    let (_, usd) = usd();
    assert_eq!(atoms(&usd.parse("0.10").unwrap()), 10);
    assert_eq!(atoms(&usd.parse("0.1").unwrap()), 10);
    assert_eq!(atoms(&usd.parse("0.100000").unwrap()), 10);
    assert_eq!(atoms(&usd.parse("007.05").unwrap()), 705);
    assert_eq!(
        usd.parse("0.001"),
        Err(UnitBindingError::InexactAtomicConversion.into())
    );
    assert_eq!(
        usd.parse("0.105"),
        Err(UnitBindingError::InexactAtomicConversion.into())
    );
}

#[test]
fn atom_counts_up_to_the_quantity_limit_are_accepted() {
    let mut schema = AssetSchema::new();
    let whole = schema.define_decimal(AssetId::Yen, 0).unwrap();
    let (_, usd) = usd();
    let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";

    // Far beyond u64: 2^64 whole units and money with cents.
    assert_eq!(
        whole
            .parse("18446744073709551616")
            .unwrap()
            .quantity()
            .to_string(),
        "18446744073709551616"
    );
    assert_eq!(
        usd.parse("184467440737095516.16")
            .unwrap()
            .quantity()
            .to_string(),
        "18446744073709551616"
    );
    assert_eq!(whole.parse(max).unwrap().quantity(), Quantity::MAX);
    assert_eq!(
        whole.parse(&"9".repeat(80)),
        Err(UnitBindingError::NumericRange.into())
    );
    // The cent digits of 2^256 - 1 atoms would exceed the limit by 100x.
    assert_eq!(usd.parse(max), Err(UnitBindingError::NumericRange.into()));
}

#[test]
fn exact_rationals_encode_without_rounding() {
    let (_, usd) = usd();
    assert_eq!(atoms(&usd.encode(Rational::new(25, 2)).unwrap()), 1_250);
    assert_eq!(atoms(&usd.encode(Rational::new(1, 4)).unwrap()), 25);
    assert_eq!(atoms(&usd.encode(Rational::from_integer(0)).unwrap()), 0);
    assert_eq!(
        usd.encode(Rational::new(1, 3)),
        Err(UnitBindingError::InexactAtomicConversion)
    );
    assert_eq!(
        usd.encode(Rational::new(1, 1_000)),
        Err(UnitBindingError::InexactAtomicConversion)
    );
    assert_eq!(
        usd.encode(Rational::from_integer(-1)),
        Err(UnitBindingError::NegativeValue)
    );
    assert_eq!(
        usd.encode(Rational::new_raw(1, -2)),
        Err(UnitBindingError::NegativeValue)
    );
    assert_eq!(
        usd.encode(Rational::new_raw(1, 0)),
        Err(UnitBindingError::InexactAtomicConversion)
    );
    // Rational64 bounds the input, not the result: i64::MAX dollars in cents.
    assert_eq!(
        usd.encode(Rational::from_integer(i64::MAX))
            .unwrap()
            .quantity()
            .to_string(),
        "922337203685477580700"
    );
    assert_eq!(
        usd.encode(Rational::new_raw(-3, -4))
            .map(|amount| atoms(&amount)),
        Ok(75)
    );
}

proptest! {
    #[test]
    fn format_and_parse_round_trip(decimals in 0..=MAX_DECIMALS, value in any::<u64>()) {
        let mut schema = AssetSchema::new();
        let asset = schema.define_decimal(AssetId::Usd, decimals).unwrap();
        let text = asset.format(Quantity::new(value));
        let parsed = asset.parse(&text).unwrap();
        prop_assert_eq!(parsed.quantity().to_u64(), value);
        if decimals > 0 {
            let (_, fraction) = text.split_once('.').unwrap();
            prop_assert_eq!(fraction.len(), usize::from(decimals));
        } else {
            prop_assert!(!text.contains('.'));
        }
    }

    #[test]
    fn rational_encoding_matches_decimal_parsing(whole in 0_u64..1_000_000, cents in 0_u64..100) {
        let (_, usd) = usd();
        let ratio = Rational::new(i64::try_from(whole * 100 + cents).unwrap(), 100);
        let encoded = usd.encode(ratio).unwrap();
        let parsed = usd.parse(&format!("{whole}.{cents:02}")).unwrap();
        prop_assert_eq!(encoded, parsed);
    }
}
