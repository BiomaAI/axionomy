use crate::{UnitAsset, definition::MeasureDefinition};
use axionomy::{AssetAmount, Quantity};
use num_rational::Ratio;
use num_traits::{CheckedMul, ToPrimitive, Zero};
use std::fmt;
use std::marker::PhantomData;
use thiserror::Error;
use uom::Conversion;
use uom::si::{Dimension, Quantity as UomQuantity, Units};

pub use num_rational::Ratio as Rational;

/// Exact scalar used by typed physical values at the authoring boundary.
pub type MeasureScalar = Ratio<i64>;

/// Typed authoring handle for one schema-defined measured asset.
///
/// Handles are issued by [`crate::AssetSchema`]. There is deliberately no
/// constructor that can bind an arbitrary raw asset identity to a local unit.
#[derive(Clone)]
pub struct MeasuredAsset<Id, D, U>
where
    D: Dimension + ?Sized,
    U: Units<MeasureScalar> + ?Sized,
    MeasureScalar: Conversion<MeasureScalar>,
{
    asset: UnitAsset<Id>,
    atomic_base_value: MeasureScalar,
    dimension: PhantomData<fn() -> *const D>,
    units: PhantomData<fn() -> *const U>,
}

impl<Id, D, U> fmt::Debug for MeasuredAsset<Id, D, U>
where
    Id: fmt::Debug,
    D: Dimension + ?Sized,
    U: Units<MeasureScalar> + ?Sized,
    MeasureScalar: Conversion<MeasureScalar>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasuredAsset")
            .field("asset", &self.asset)
            .field("atomic_base_value", &self.atomic_base_value)
            .finish_non_exhaustive()
    }
}

impl<Id, D, U> MeasuredAsset<Id, D, U>
where
    D: Dimension + ?Sized,
    U: Units<MeasureScalar> + ?Sized,
    MeasureScalar: Conversion<MeasureScalar>,
{
    pub(crate) fn new(asset: UnitAsset<Id>, definition: &MeasureDefinition) -> Self {
        Self {
            asset,
            atomic_base_value: *definition.atomic_base_value(),
            dimension: PhantomData,
            units: PhantomData,
        }
    }

    pub const fn asset(&self) -> &UnitAsset<Id> {
        &self.asset
    }

    /// Lowers a compatible physical value into the asset's canonical atoms.
    ///
    /// Unit arithmetic is exact over `uom`'s `Rational64` storage, so one
    /// lowered value is at most `i64::MAX` atoms; the economy's quantities
    /// themselves range up to `2²⁵⁶ − 1`.
    ///
    /// A handle accepts only the physical dimension with which it was defined:
    ///
    /// ```compile_fail
    /// use axionomy_units::si::length::meter;
    /// use axionomy_units::si::mass::gram;
    /// use axionomy_units::si::rational64::{Length, Mass};
    /// use axionomy_units::{AssetSchema, Rational};
    ///
    /// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    /// enum AssetId { Cargo }
    ///
    /// let mut schema = AssetSchema::new();
    /// let cargo = schema
    ///     .define_measure(
    ///         AssetId::Cargo,
    ///         Mass::new::<gram>(Rational::from_integer(1)),
    ///     )
    ///     .unwrap();
    /// cargo
    ///     .encode::<u64>(Length::new::<meter>(Rational::from_integer(1)))
    ///     .unwrap();
    /// ```
    pub fn encode(
        &self,
        value: UomQuantity<D, U, MeasureScalar>,
    ) -> Result<AssetAmount<UnitAsset<Id>>, UnitBindingError>
    where
        Id: Clone,
    {
        let atoms = value.value / self.atomic_base_value;
        if atoms < MeasureScalar::zero() {
            return Err(UnitBindingError::NegativeValue);
        }
        if !atoms.is_integer() {
            return Err(UnitBindingError::InexactAtomicConversion);
        }
        let count = atoms
            .to_integer()
            .to_u64()
            .ok_or(UnitBindingError::NumericRange)?;
        Ok(self.asset.amount(Quantity::new(count)))
    }

    /// The exact physical value of `quantity` atoms of this asset, the inverse
    /// of [`encode`](Self::encode): read it in any compatible unit, such as
    /// `decode(1_000 atoms)?.get::<kilogram>()` for gram atoms.
    pub fn decode(
        &self,
        quantity: Quantity,
    ) -> Result<UomQuantity<D, U, MeasureScalar>, UnitBindingError> {
        let atoms = u64::try_from(quantity)
            .ok()
            .and_then(|atoms| i64::try_from(atoms).ok())
            .map(MeasureScalar::from_integer)
            .ok_or(UnitBindingError::NumericRange)?;
        let value = atoms
            .checked_mul(&self.atomic_base_value)
            .ok_or(UnitBindingError::NumericRange)?;
        Ok(UomQuantity {
            dimension: PhantomData,
            units: PhantomData,
            value,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UnitBindingError {
    #[error("physical value cannot become a negative economic quantity")]
    NegativeValue,
    #[error("physical value is not an exact multiple of the asset's atomic basis")]
    InexactAtomicConversion,
    #[error("atomic count is outside the adapter or selected quantity backend's supported range")]
    NumericRange,
}
