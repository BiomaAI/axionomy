use num_traits::{CheckedAdd, CheckedMul, CheckedSub, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;
use thiserror::Error;

/// Exact scalar behavior required by authoritative economic quantities.
///
/// Implementations may have a wider representable domain than Axionomy
/// quantities. [`Quantity::try_from_scalar`] enforces non-negativity at the
/// boundary. Floating-point implementations are intentionally unsupported:
/// authoritative balances require exact equality and total ordering.
pub trait QuantityScalar:
    Clone + Eq + Ord + Hash + fmt::Debug + fmt::Display + Zero + CheckedAdd + CheckedSub + CheckedMul
{
    /// Signed exact type used while evaluating linear invariants.
    type SignedMeasure: Clone + Eq + fmt::Debug + fmt::Display + Zero + CheckedAdd;

    /// Returns whether this value is valid as a non-negative quantity.
    fn is_nonnegative(&self) -> bool;

    /// Converts an atomic count into this backend without loss.
    fn from_u64(value: u64) -> Option<Self>;

    /// Multiplies this non-negative value by a signed invariant coefficient.
    fn checked_weighted(&self, coefficient: i64) -> Option<Self::SignedMeasure>;
}

impl QuantityScalar for u64 {
    type SignedMeasure = i128;

    fn is_nonnegative(&self) -> bool {
        true
    }

    fn from_u64(value: u64) -> Option<Self> {
        Some(value)
    }

    fn checked_weighted(&self, coefficient: i64) -> Option<Self::SignedMeasure> {
        i128::from(*self).checked_mul(i128::from(coefficient))
    }
}

#[cfg(feature = "bigint")]
impl QuantityScalar for num_bigint::BigUint {
    type SignedMeasure = num_bigint::BigInt;

    fn is_nonnegative(&self) -> bool {
        true
    }

    fn from_u64(value: u64) -> Option<Self> {
        Some(value.into())
    }

    fn checked_weighted(&self, coefficient: i64) -> Option<Self::SignedMeasure> {
        Some(num_bigint::BigInt::from(self.clone()) * num_bigint::BigInt::from(coefficient))
    }
}

/// A checked non-negative coefficient in an economy's numeric backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quantity<N = u64>(N);

impl Quantity<u64> {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl<N> Quantity<N>
where
    N: QuantityScalar,
{
    /// Constructs a quantity from a scalar after validating non-negativity.
    pub fn try_from_scalar(value: N) -> Result<Self, QuantityError> {
        if value.is_nonnegative() {
            Ok(Self(value))
        } else {
            Err(QuantityError::Negative)
        }
    }

    pub fn as_scalar(&self) -> &N {
        &self.0
    }

    pub fn into_scalar(self) -> N {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        self.0
            .checked_add(&rhs.0)
            .and_then(|value| Self::try_from_scalar(value).ok())
    }

    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        self.0
            .checked_sub(&rhs.0)
            .and_then(|value| Self::try_from_scalar(value).ok())
    }

    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        self.0
            .checked_mul(&rhs.0)
            .and_then(|value| Self::try_from_scalar(value).ok())
    }
}

impl<N> Default for Quantity<N>
where
    N: QuantityScalar,
{
    fn default() -> Self {
        Self(N::zero())
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

impl<N> fmt::Display for Quantity<N>
where
    N: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<N> Serialize for Quantity<N>
where
    N: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, N> Deserialize<'de> for Quantity<N>
where
    N: Deserialize<'de> + QuantityScalar,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = N::deserialize(deserializer)?;
        Self::try_from_scalar(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum QuantityError {
    #[error("quantity cannot be negative")]
    Negative,
}

#[cfg(all(test, not(feature = "bigint")))]
mod tests {
    use super::{Quantity, QuantityScalar};
    use num_bigint::{BigInt, BigUint};

    impl QuantityScalar for BigUint {
        type SignedMeasure = BigInt;

        fn is_nonnegative(&self) -> bool {
            true
        }

        fn from_u64(value: u64) -> Option<Self> {
            Some(value.into())
        }

        fn checked_weighted(&self, coefficient: i64) -> Option<Self::SignedMeasure> {
            Some(BigInt::from(self.clone()) * BigInt::from(coefficient))
        }
    }

    #[test]
    fn non_copy_backend_uses_exact_checked_arithmetic() {
        let left = Quantity::try_from_scalar(BigUint::from(u64::MAX)).unwrap();
        let right = Quantity::try_from_scalar(BigUint::from(2_u8)).unwrap();
        let sum = left.checked_add(&right).unwrap();

        assert_eq!(
            sum.into_scalar(),
            BigUint::from(u64::MAX) + BigUint::from(2_u8)
        );
    }
}
