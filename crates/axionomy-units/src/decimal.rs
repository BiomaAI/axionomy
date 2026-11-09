use crate::{DecimalPrecision, MeasureScalar, UnitAsset, UnitBindingError};
use axionomy::{AssetAmount, Quantity};
use thiserror::Error;

/// Typed authoring handle for one schema-defined decimal asset.
///
/// Handles are issued by [`crate::AssetSchema::define_decimal`] or reconstructed
/// with [`crate::AssetSchema::decimal`]; there is no constructor that binds an
/// arbitrary raw asset identity to a local precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecimalAsset<Id> {
    asset: UnitAsset<Id>,
    precision: DecimalPrecision,
}

impl<Id> DecimalAsset<Id> {
    pub(crate) const fn new(asset: UnitAsset<Id>, precision: DecimalPrecision) -> Self {
        Self { asset, precision }
    }

    pub const fn asset(&self) -> &UnitAsset<Id> {
        &self.asset
    }

    pub const fn precision(&self) -> DecimalPrecision {
        self.precision
    }

    /// Lowers an exact number of whole display units into canonical atoms.
    ///
    /// Values that are not an exact multiple of one atom are rejected rather
    /// than rounded.
    pub fn encode(
        &self,
        units: MeasureScalar,
    ) -> Result<AssetAmount<UnitAsset<Id>>, UnitBindingError>
    where
        Id: Clone,
    {
        let numerator = i128::from(*units.numer());
        let denominator = i128::from(*units.denom());
        if denominator == 0 {
            return Err(UnitBindingError::InexactAtomicConversion);
        }
        if numerator != 0 && (numerator < 0) != (denominator < 0) {
            return Err(UnitBindingError::NegativeValue);
        }
        // |numerator| <= 2^63 and atoms_per_unit <= 10^18, so the product fits u128.
        let scaled = numerator.unsigned_abs() * u128::from(self.precision.atoms_per_unit());
        let divisor = denominator.unsigned_abs();
        if scaled % divisor != 0 {
            return Err(UnitBindingError::InexactAtomicConversion);
        }
        Ok(self.asset.amount(Quantity::from(scaled / divisor)))
    }

    /// Parses a plain decimal such as `12.50` into canonical atoms.
    ///
    /// The grammar is `digits [ "." digits ]`: no sign, exponent, separators,
    /// or surrounding whitespace. Fractional digits beyond the precision are
    /// accepted only when they are zero, so the result is always exact. Any
    /// amount below `2²⁵⁶` atoms is accepted.
    pub fn parse(&self, text: &str) -> Result<AssetAmount<UnitAsset<Id>>, DecimalParseError>
    where
        Id: Clone,
    {
        let (whole, fraction) = match text.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (text, None),
        };
        let digits =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if !digits(whole) || fraction.is_some_and(|fraction| !digits(fraction)) {
            return Err(DecimalParseError::InvalidSyntax);
        }
        let decimals = usize::from(self.precision.decimals());
        let fraction = fraction.unwrap_or("");
        let (kept, excess) = fraction.split_at(fraction.len().min(decimals));
        if excess.bytes().any(|byte| byte != b'0') {
            return Err(UnitBindingError::InexactAtomicConversion.into());
        }
        let atoms = format!("{whole}{kept:0<decimals$}")
            .parse::<Quantity>()
            .map_err(|_| UnitBindingError::NumericRange)?;
        Ok(self.asset.amount(atoms))
    }

    /// Renders a quantity of this asset with exactly `decimals` fraction digits.
    ///
    /// The output round-trips through [`Self::parse`].
    pub fn format(&self, quantity: Quantity) -> String {
        let decimals = usize::from(self.precision.decimals());
        let digits = quantity.to_string();
        if decimals == 0 {
            return digits;
        }
        let padded = format!("{digits:0>width$}", width = decimals + 1);
        let (whole, fraction) = padded.split_at(padded.len() - decimals);
        format!("{whole}.{fraction}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DecimalParseError {
    #[error("decimal text must be digits with an optional fractional part")]
    InvalidSyntax,
    #[error(transparent)]
    Binding(#[from] UnitBindingError),
}
