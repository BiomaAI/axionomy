#![doc = include_str!("../README.md")]

use axionomy::{AssetAmount, Quantity, QuantityScalar};
use num_rational::Ratio;
use num_traits::{ToPrimitive, Zero};
use std::marker::PhantomData;
use thiserror::Error;
use uom::Conversion;
use uom::si::{Dimension, Quantity as UomQuantity, Units};

pub use num_rational::Ratio as Rational;
pub use uom::si;

/// Exact scalar used by typed physical values at the authoring boundary.
pub type MeasureScalar = Ratio<i64>;

/// Binds one dimensionally typed physical value to one atomic economic asset.
///
/// The `atomic` quantity defines what one unit of the asset means. Values with
/// the same `D` dimension and `U` unit system can then be lowered exactly into
/// [`AssetAmount`].
#[derive(Debug, Clone)]
pub struct MeasuredAsset<A, D, U>
where
    D: Dimension + ?Sized,
    U: Units<MeasureScalar> + ?Sized,
    MeasureScalar: Conversion<MeasureScalar>,
{
    asset: A,
    atomic_base_value: MeasureScalar,
    dimension: PhantomData<fn() -> *const D>,
    units: PhantomData<fn() -> *const U>,
}

impl<A, D, U> MeasuredAsset<A, D, U>
where
    D: Dimension + ?Sized,
    U: Units<MeasureScalar> + ?Sized,
    MeasureScalar: Conversion<MeasureScalar>,
{
    /// Creates a binding from an asset and the physical value of one atom.
    pub fn new(
        asset: A,
        atomic: UomQuantity<D, U, MeasureScalar>,
    ) -> Result<Self, UnitBindingError> {
        if atomic.value <= MeasureScalar::zero() {
            return Err(UnitBindingError::InvalidAtomicBasis);
        }
        Ok(Self {
            asset,
            atomic_base_value: atomic.value,
            dimension: PhantomData,
            units: PhantomData,
        })
    }

    pub fn asset(&self) -> &A {
        &self.asset
    }

    /// Lowers a typed physical value into an exact asset-qualified amount.
    pub fn encode<N>(
        &self,
        value: UomQuantity<D, U, MeasureScalar>,
    ) -> Result<AssetAmount<A, N>, UnitBindingError>
    where
        A: Clone,
        N: QuantityScalar,
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
        let scalar = N::from_u64(count).ok_or(UnitBindingError::NumericRange)?;
        let quantity =
            Quantity::try_from_scalar(scalar).map_err(|_| UnitBindingError::NegativeValue)?;
        Ok(AssetAmount::new(self.asset.clone(), quantity))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UnitBindingError {
    #[error("atomic physical basis must be greater than zero")]
    InvalidAtomicBasis,
    #[error("physical value cannot become a negative economic quantity")]
    NegativeValue,
    #[error("physical value is not an exact multiple of the asset's atomic basis")]
    InexactAtomicConversion,
    #[error("atomic count is outside the selected quantity backend's supported range")]
    NumericRange,
}

#[cfg(test)]
mod tests {
    use super::{MeasuredAsset, Rational};
    use axionomy::{AssetAmount, Quantity};
    use proptest::prelude::*;
    use uom::si::mass::{gram, kilogram};
    use uom::si::rational64::Mass;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Asset {
        CargoGram,
    }

    #[test]
    fn mass_lowers_exactly_into_an_asset_qualified_quantity() {
        let binding = MeasuredAsset::new(
            Asset::CargoGram,
            Mass::new::<gram>(Rational::from_integer(1)),
        )
        .unwrap();
        let amount: AssetAmount<Asset> = binding
            .encode(Mass::new::<kilogram>(Rational::new(25, 2)))
            .unwrap();

        assert_eq!(amount.asset(), &Asset::CargoGram);
        assert_eq!(amount.quantity(), &Quantity::new(12_500));
    }

    #[test]
    fn conversion_rejects_sub_atomic_values() {
        let binding = MeasuredAsset::new(
            Asset::CargoGram,
            Mass::new::<gram>(Rational::from_integer(1)),
        )
        .unwrap();
        let result: Result<AssetAmount<Asset>, _> =
            binding.encode(Mass::new::<gram>(Rational::new(1, 2)));

        assert!(result.is_err());
    }

    proptest! {
        #[test]
        fn every_whole_gram_round_trips_exactly(grams in 0_u64..1_000_000) {
            let binding = MeasuredAsset::new(
                Asset::CargoGram,
                Mass::new::<gram>(Rational::from_integer(1)),
            )
            .unwrap();
            let amount: AssetAmount<Asset> = binding
                .encode(Mass::new::<gram>(Rational::from_integer(grams as i64)))
                .unwrap();

            prop_assert_eq!(amount.quantity(), &Quantity::new(grams));
        }
    }
}
