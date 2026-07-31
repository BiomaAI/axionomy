use crate::{Basket, Quantity, QuantityScalar};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "A: Serialize, N: Serialize",
    deserialize = "A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
pub struct Account<A, N = u64> {
    balances: Basket<A, N>,
}

impl<A, N> Account<A, N> {
    pub fn new(balances: Basket<A, N>) -> Self {
        Self { balances }
    }

    pub fn balances(&self) -> &Basket<A, N> {
        &self.balances
    }

    pub fn into_balances(self) -> Basket<A, N> {
        self.balances
    }
}

impl<A, N> Account<A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    pub fn balance(&self, asset: &A) -> Quantity<N> {
        self.balances.quantity(asset)
    }
}

impl<A, N> Account<A, N>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    pub fn deposit(&mut self, assets: &Basket<A, N>) -> Result<(), AccountError<A, N>> {
        let mut updated = self.balances.clone();

        for (asset, amount) in assets.iter() {
            let balance = updated.quantity(asset);
            let Some(balance) = balance.checked_add(amount) else {
                return Err(AccountError::Overflow {
                    asset: asset.clone(),
                });
            };
            updated.insert(asset.clone(), balance);
        }

        self.balances = updated;
        Ok(())
    }

    pub fn withdraw(&mut self, assets: &Basket<A, N>) -> Result<(), AccountError<A, N>> {
        let shortfall = self.balances.shortfall(assets);
        if !shortfall.is_empty() {
            return Err(AccountError::InsufficientBalance { shortfall });
        }

        let mut updated = self.balances.clone();
        for (asset, amount) in assets.iter() {
            let balance = updated
                .quantity(asset)
                .checked_sub(amount)
                .expect("shortfall was checked before withdrawal");
            updated.insert(asset.clone(), balance);
        }

        self.balances = updated;
        Ok(())
    }
}

impl<A, N> Default for Account<A, N> {
    fn default() -> Self {
        Self::new(Basket::new())
    }
}

impl<A, N> PartialEq for Account<A, N>
where
    A: Eq + Hash,
    N: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.balances == other.balances
    }
}

impl<A, N> Eq for Account<A, N>
where
    A: Eq + Hash,
    N: Eq,
{
}

impl<A, N> From<Basket<A, N>> for Account<A, N> {
    fn from(balances: Basket<A, N>) -> Self {
        Self::new(balances)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "A: Serialize, N: Serialize",
    deserialize = "A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
pub enum AccountError<A, N = u64>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    #[error("insufficient balance")]
    InsufficientBalance { shortfall: Basket<A, N> },
    #[error("account balance overflow")]
    Overflow { asset: A },
}
