use crate::{Quantity, QuantityScalar};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One asset-qualified economic quantity.
///
/// `A` preserves what the value means and names its atomic basis. `N`
/// determines how the exact non-negative coefficient is represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "A: Serialize, N: Serialize",
    deserialize = "A: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
))]
pub struct AssetAmount<A, N = u64> {
    asset: A,
    quantity: Quantity<N>,
}

impl<A, N> AssetAmount<A, N> {
    pub const fn new(asset: A, quantity: Quantity<N>) -> Self {
        Self { asset, quantity }
    }

    pub const fn asset(&self) -> &A {
        &self.asset
    }

    pub const fn quantity(&self) -> &Quantity<N> {
        &self.quantity
    }

    pub fn into_parts(self) -> (A, Quantity<N>) {
        (self.asset, self.quantity)
    }
}

impl<A, N> From<(A, Quantity<N>)> for AssetAmount<A, N> {
    fn from((asset, quantity): (A, Quantity<N>)) -> Self {
        Self::new(asset, quantity)
    }
}

impl<A, N> From<AssetAmount<A, N>> for (A, Quantity<N>) {
    fn from(amount: AssetAmount<A, N>) -> Self {
        amount.into_parts()
    }
}
