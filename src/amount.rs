use crate::Quantity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One asset-qualified economic quantity.
///
/// `A` preserves what the value means and names its atomic basis, so asset
/// domains remain distinct at compile time:
///
/// ```compile_fail,E0308
/// use axionomy::{AssetAmount, Quantity};
/// struct Food;
/// struct Fuel;
/// fn prepare(_: AssetAmount<Food>) {}
/// prepare(AssetAmount::new(Fuel, Quantity::new(1)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AssetAmount<A> {
    asset: A,
    quantity: Quantity,
}

impl<A> AssetAmount<A> {
    pub const fn new(asset: A, quantity: Quantity) -> Self {
        Self { asset, quantity }
    }

    pub const fn asset(&self) -> &A {
        &self.asset
    }

    pub const fn quantity(&self) -> Quantity {
        self.quantity
    }

    pub fn into_parts(self) -> (A, Quantity) {
        (self.asset, self.quantity)
    }
}

impl<A> From<(A, Quantity)> for AssetAmount<A> {
    fn from((asset, quantity): (A, Quantity)) -> Self {
        Self::new(asset, quantity)
    }
}

impl<A> From<AssetAmount<A>> for (A, Quantity) {
    fn from(value: AssetAmount<A>) -> Self {
        value.into_parts()
    }
}
