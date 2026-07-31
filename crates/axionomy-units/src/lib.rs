#![doc = include_str!("../README.md")]

mod binding;
mod definition;
mod schema;

pub use binding::{MeasureScalar, MeasuredAsset, Rational, UnitBindingError};
pub use definition::{
    AssetDefinition, DimensionSignature, MeasureDefinition, MeasureDefinitionError, MeasureKind,
    StableMeasureKind, UnitAsset,
};
pub use schema::{
    AssetSchema, AssetSchemaError, UnitEconomy, UnitModelBuildError, UnitModelBuildResult,
};
pub use uom::si;
