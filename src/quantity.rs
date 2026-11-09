use ruint::UintTryFrom;
use ruint::aliases::{U256, U512};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Direction in which an inexact quotient is rounded.
///
/// Round amounts a participant pays up and amounts a participant receives
/// down, so rounding dust always stays with the counterparty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rounding {
    /// Round an inexact quotient down (`⌊x⌋`).
    Floor,
    /// Round an inexact quotient up (`⌈x⌉`).
    Ceil,
}

/// An exact non-negative whole number of atoms, up to `2²⁵⁶ − 1`.
///
/// Arithmetic is checked: every operation returns the exact result or `None`,
/// and never wraps. Quantities serialize as decimal strings so no consumer
/// silently loses precision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quantity(U256);

impl Quantity {
    pub const ZERO: Self = Self(U256::ZERO);
    pub const MAX: Self = Self(U256::MAX);

    pub const fn new(value: u64) -> Self {
        Self(U256::from_limbs([value, 0, 0, 0]))
    }

    /// The exact value.
    pub const fn value(self) -> U256 {
        self.0
    }

    /// The value as 32 big-endian bytes, which sort as the quantities do.
    pub fn to_be_bytes(self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    pub fn from_be_bytes(bytes: [u8; 32]) -> Self {
        Self(U256::from_be_bytes(bytes))
    }

    /// The value as a `u64`, for models whose quantities are counts known to
    /// fit. Use `u64::try_from` when the value may be larger.
    ///
    /// # Panics
    ///
    /// If the value exceeds `u64::MAX`.
    pub fn to_u64(self) -> u64 {
        u64::try_from(self).expect("quantity exceeds u64")
    }

    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        self.0.checked_mul(rhs.0).map(Self)
    }

    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        self.0.checked_div(rhs.0).map(Self)
    }

    /// Returns `self × rhs ÷ denominator`, rounded as requested.
    ///
    /// The product is computed at full 512-bit width, so only the quotient must
    /// fit: `None` means the denominator is zero or the rounded quotient
    /// exceeds `Quantity::MAX`.
    ///
    /// ```
    /// use axionomy::{Quantity, Rounding};
    /// let max = Quantity::MAX;
    /// assert_eq!(max.checked_mul_div(max, max, Rounding::Floor), Some(max));
    /// let (seven, one, two) = (Quantity::new(7), Quantity::new(1), Quantity::new(2));
    /// assert_eq!(seven.checked_mul_div(one, two, Rounding::Floor), Some(Quantity::new(3)));
    /// assert_eq!(seven.checked_mul_div(one, two, Rounding::Ceil), Some(Quantity::new(4)));
    /// assert_eq!(seven.checked_mul_div(one, Quantity::ZERO, Rounding::Ceil), None);
    /// ```
    pub fn checked_mul_div(self, rhs: Self, denominator: Self, rounding: Rounding) -> Option<Self> {
        if denominator.is_zero() {
            return None;
        }
        let product: U512 = self.0.widening_mul(rhs.0);
        let (quotient, remainder) = product.div_rem(U512::from(denominator.0));
        // A nonzero remainder needs a denominator above one, so the quotient is
        // below 2^511 and the increment cannot overflow.
        let quotient = match rounding {
            Rounding::Ceil if !remainder.is_zero() => quotient + U512::from(1_u8),
            Rounding::Floor | Rounding::Ceil => quotient,
        };
        U256::uint_try_from(quotient).ok().map(Self)
    }
}

impl From<u64> for Quantity {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<u32> for Quantity {
    fn from(value: u32) -> Self {
        Self::new(u64::from(value))
    }
}

impl From<u128> for Quantity {
    fn from(value: u128) -> Self {
        Self(U256::from(value))
    }
}

impl From<U256> for Quantity {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

impl TryFrom<Quantity> for u64 {
    type Error = QuantityError;

    fn try_from(quantity: Quantity) -> Result<Self, Self::Error> {
        u64::try_from(quantity.0).map_err(|_| QuantityError::OutOfRange)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl FromStr for Quantity {
    type Err = QuantityError;

    /// Parses a decimal whole number of atoms.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(QuantityError::Invalid);
        }
        U256::from_str_radix(text, 10)
            .map(Self)
            .map_err(|_| QuantityError::OutOfRange)
    }
}

impl Serialize for Quantity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Quantity {
    fn schema_name() -> Cow<'static, str> {
        "Quantity".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A non-negative whole number of atoms below 2^256, as decimal digits.",
            "type": "string",
            "pattern": "^[0-9]+$"
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum QuantityError {
    #[error("quantity must be decimal digits")]
    Invalid,
    #[error("quantity is out of range")]
    OutOfRange,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_exact_decimal_values() {
        let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        assert_eq!(max.parse::<Quantity>(), Ok(Quantity::MAX));
        assert_eq!(Quantity::MAX.to_string(), max);
        assert_eq!(
            "115792089237316195423570985008687907853269984665640564039457584007913129639936"
                .parse::<Quantity>(),
            Err(QuantityError::OutOfRange)
        );
        for invalid in ["", "-1", "1.5", "0x10", " 1"] {
            assert_eq!(invalid.parse::<Quantity>(), Err(QuantityError::Invalid));
        }
        assert_eq!(serde_json::to_string(&Quantity::new(42)).unwrap(), "\"42\"");
        assert_eq!(
            serde_json::from_str::<Quantity>("\"42\"").unwrap(),
            Quantity::new(42)
        );
        assert!(serde_json::from_str::<Quantity>("42").is_err());
    }

    #[test]
    fn narrows_only_when_the_value_fits() {
        assert_eq!(Quantity::new(7).to_u64(), 7);
        assert_eq!(u64::try_from(Quantity::new(u64::MAX)), Ok(u64::MAX));
        let beyond = Quantity::new(u64::MAX)
            .checked_add(Quantity::new(1))
            .unwrap();
        assert_eq!(u64::try_from(beyond), Err(QuantityError::OutOfRange));
    }
}
