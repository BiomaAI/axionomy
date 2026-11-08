use num_traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
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

/// Scalar operations required by a quantity backend's invariant measure.
///
/// The blanket implementation preserves the existing backend requirements.
pub trait QuantityMeasure: Clone + Eq + fmt::Debug + fmt::Display + Zero + CheckedAdd {}

impl<T> QuantityMeasure for T where T: Clone + Eq + fmt::Debug + fmt::Display + Zero + CheckedAdd {}

/// Exact scalar behavior required by authoritative economic quantities.
///
/// Implementations may have a wider representable domain than Axionomy
/// quantities. [`Quantity::try_from_scalar`] enforces non-negativity at the
/// boundary. Floating-point implementations are intentionally unsupported:
/// authoritative balances require exact equality and total ordering.
pub trait QuantityScalar:
    Clone
    + Eq
    + Ord
    + Hash
    + fmt::Debug
    + fmt::Display
    + Zero
    + CheckedAdd
    + CheckedSub
    + CheckedMul
    + CheckedDiv
{
    /// Signed exact type used while evaluating linear invariants.
    type SignedMeasure: QuantityMeasure;

    /// Returns whether this value is valid as a non-negative quantity.
    fn is_nonnegative(&self) -> bool;

    /// Converts an atomic count into this backend without loss.
    fn from_u64(value: u64) -> Option<Self>;

    /// Multiplies this non-negative value by a signed invariant coefficient.
    fn checked_weighted(&self, coefficient: i64) -> Option<Self::SignedMeasure>;

    /// Computes `self × rhs ÷ denominator` from an exact, full-width product,
    /// rounding an inexact quotient in the given direction.
    ///
    /// Only the quotient must be representable: an intermediate product that
    /// exceeds this backend must not cause a failure. Returns `None` for a zero
    /// denominator or an unrepresentable quotient.
    fn checked_mul_div(&self, rhs: &Self, denominator: &Self, rounding: Rounding) -> Option<Self>;
}

// A u64 × u64 product always fits u128, so widened arithmetic cannot overflow;
// only the final quotient is narrowed back to u64.
const fn widen(value: u64) -> u128 {
    value as u128
}

const fn narrow(value: u128) -> Option<u64> {
    if value > u64::MAX as u128 {
        None
    } else {
        Some(value as u64)
    }
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

    fn checked_mul_div(&self, rhs: &Self, denominator: &Self, rounding: Rounding) -> Option<Self> {
        if *denominator == 0 {
            return None;
        }
        let product = widen(*self) * widen(*rhs);
        let divisor = widen(*denominator);
        let quotient = product / divisor;
        match rounding {
            Rounding::Floor => narrow(quotient),
            Rounding::Ceil => {
                // Aeneas models the primitive remainder operator, which the
                // proof uses directly; `is_multiple_of` is not extracted.
                let remainder = product % divisor;
                if remainder == 0 {
                    narrow(quotient)
                } else {
                    narrow(quotient + 1)
                }
            }
        }
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

    fn checked_mul_div(&self, rhs: &Self, denominator: &Self, rounding: Rounding) -> Option<Self> {
        if denominator.is_zero() {
            return None;
        }
        let product = self * rhs;
        let quotient = &product / denominator;
        match rounding {
            Rounding::Ceil if !(&product % denominator).is_zero() => Some(quotient + 1_u32),
            Rounding::Floor | Rounding::Ceil => Some(quotient),
        }
    }
}

/// A checked non-negative coefficient in an economy's numeric backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quantity<N = u64>(N);

impl<N: Zero> Quantity<N> {
    // Internal operations need only the scalar behavior they actually use;
    // the public API continues to require the complete QuantityScalar contract.
    pub(crate) fn scalar_is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

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
        self.scalar_is_zero()
    }

    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        Self::from_checked(self.0.checked_add(&rhs.0))
    }

    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        Self::from_checked(self.0.checked_sub(&rhs.0))
    }

    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        Self::from_checked(self.0.checked_mul(&rhs.0))
    }

    pub fn checked_div(&self, rhs: &Self) -> Option<Self> {
        Self::from_checked(self.0.checked_div(&rhs.0))
    }

    /// Returns `self × rhs ÷ denominator`, rounded as requested, without
    /// intermediate overflow.
    ///
    /// The product is computed at full width; `None` means the denominator is
    /// zero or the rounded quotient is not a representable quantity.
    ///
    /// ```
    /// use axionomy::{Quantity, Rounding};
    /// let max = Quantity::new(u64::MAX);
    /// assert_eq!(max.checked_mul_div(&max, &max, Rounding::Floor), Some(max));
    /// let (seven, one, two) = (Quantity::new(7), Quantity::new(1), Quantity::new(2));
    /// assert_eq!(seven.checked_mul_div(&one, &two, Rounding::Floor), Some(Quantity::new(3)));
    /// assert_eq!(seven.checked_mul_div(&one, &two, Rounding::Ceil), Some(Quantity::new(4)));
    /// assert_eq!(seven.checked_mul_div(&one, &Quantity::ZERO, Rounding::Ceil), None);
    /// ```
    pub fn checked_mul_div(
        &self,
        rhs: &Self,
        denominator: &Self,
        rounding: Rounding,
    ) -> Option<Self> {
        Self::from_checked(self.0.checked_mul_div(&rhs.0, &denominator.0, rounding))
    }

    // Explicit matching lets Aeneas extract the complete conversion without
    // a model of the standard library's generic Result::ok drop handling.
    #[allow(clippy::manual_ok_err, reason = "Aeneas needs the explicit conversion")]
    fn from_checked(value: Option<N>) -> Option<Self> {
        match value {
            Some(value) => match Self::try_from_scalar(value) {
                Ok(quantity) => Some(quantity),
                Err(_) => None,
            },
            None => None,
        }
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

impl<N> JsonSchema for Quantity<N>
where
    N: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("Quantity_{}", N::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        generator.subschema_for::<N>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum QuantityError {
    #[error("quantity cannot be negative")]
    Negative,
}

#[cfg(test)]
mod signed_scalar_tests {
    use super::{Quantity, QuantityScalar, Rounding};

    // A backend can represent negative intermediate results even though a
    // Quantity cannot. Keep this generic boundary intact when proving u64.
    impl QuantityScalar for i64 {
        type SignedMeasure = i128;

        fn is_nonnegative(&self) -> bool {
            *self >= 0
        }

        fn from_u64(value: u64) -> Option<Self> {
            Self::try_from(value).ok()
        }

        fn checked_weighted(&self, coefficient: i64) -> Option<Self::SignedMeasure> {
            i128::from(*self).checked_mul(i128::from(coefficient))
        }

        fn checked_mul_div(
            &self,
            rhs: &Self,
            denominator: &Self,
            rounding: Rounding,
        ) -> Option<Self> {
            let product = i128::from(*self) * i128::from(*rhs);
            let divisor = i128::from(*denominator);
            let quotient = product.checked_div_euclid(divisor)?;
            let quotient = match rounding {
                Rounding::Ceil if product.rem_euclid(divisor) != 0 => quotient + 1,
                Rounding::Floor | Rounding::Ceil => quotient,
            };
            Self::try_from(quotient).ok()
        }
    }

    #[test]
    fn runtime_quantity_validation_matches_lean() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        let cases = fixtures["publicData"]["validation"].as_array().unwrap();
        assert_eq!(cases.len(), 5);
        for case in cases {
            let input = case["input"].as_str().unwrap().parse::<i64>().unwrap();
            let actual = Quantity::try_from_scalar(input)
                .ok()
                .map(|value| value.into_scalar().to_string());
            assert_eq!(serde_json::json!(actual), case["accepted"]);
        }
    }

    #[test]
    fn checked_results_still_enforce_nonnegativity_for_other_backends() {
        let one = Quantity::try_from_scalar(1_i64).unwrap();
        let two = Quantity::try_from_scalar(2_i64).unwrap();
        assert_eq!(one.checked_sub(&two), None);
        assert_eq!(two.checked_sub(&one), Some(one));
        assert!(Quantity::try_from_scalar(-1_i64).is_err());
    }
}

#[cfg(all(test, not(feature = "bigint")))]
mod tests {
    use super::{Quantity, QuantityScalar, Rounding};
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

        fn checked_mul_div(
            &self,
            rhs: &Self,
            denominator: &Self,
            rounding: Rounding,
        ) -> Option<Self> {
            let zero = BigUint::from(0_u8);
            if denominator == &zero {
                return None;
            }
            let product = self * rhs;
            let quotient = &product / denominator;
            Some(match rounding {
                Rounding::Ceil if &product % denominator != zero => quotient + 1_u32,
                Rounding::Floor | Rounding::Ceil => quotient,
            })
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
