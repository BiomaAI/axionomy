#![doc = include_str!("../README.md")]

mod binding;
mod decimal;
mod definition;
mod schema;

pub use binding::{MeasureScalar, MeasuredAsset, Rational, UnitBindingError};
pub use decimal::{DecimalAsset, DecimalParseError};
pub use definition::{
    AssetDefinition, DecimalPrecision, DecimalPrecisionError, DimensionSignature, MAX_DECIMALS,
    MeasureDefinition, MeasureDefinitionError, MeasureKind, StableMeasureKind, UnitAsset,
};
pub use schema::{
    AssetSchema, AssetSchemaError, UnitEconomy, UnitModelBuildError, UnitModelBuildResult,
};
pub use uom::si;
