use crate::{Basket, Quantity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "A: Serialize",
    deserialize = "A: Deserialize<'de> + Eq + Hash"
))]
pub struct Account<A> {
    balances: Basket<A>,
}

impl<A> Account<A> {
    pub fn new(balances: Basket<A>) -> Self {
        Self { balances }
    }

    pub fn balances(&self) -> &Basket<A> {
        &self.balances
    }
}

impl<A: Eq + Hash> Account<A> {
    pub fn balance(&self, asset: &A) -> Quantity {
        self.balances.quantity(asset)
    }
}

impl<A: Clone + Eq + Hash> Account<A> {
    /// Adds every asset, or changes nothing if any balance would overflow.
    pub fn deposit(&mut self, assets: &Basket<A>) -> Result<(), AccountError<A>> {
        let mut updated = self.balances.clone();
        updated
            .checked_add(assets)
            .map_err(|asset| AccountError::Overflow { asset })?;
        self.balances = updated;
        Ok(())
    }

    /// Removes every asset, or changes nothing if any balance is short.
    pub fn withdraw(&mut self, assets: &Basket<A>) -> Result<(), AccountError<A>> {
        let shortfall = self.balances.shortfall(assets);
        if !shortfall.is_empty() {
            return Err(AccountError::InsufficientBalance { shortfall });
        }
        for (asset, amount) in assets.iter() {
            let balance = self.balances.quantity(asset);
            let remaining = balance
                .checked_sub(*amount)
                .expect("the shortfall check covers every asset");
            self.balances.insert(asset.clone(), remaining);
        }
        Ok(())
    }

    /// Stages an exchange on a private copy, withdrawing before depositing.
    /// The source account remains unchanged even if the deposit overflows.
    pub(crate) fn prepare_exchange(
        &self,
        consumed: &Basket<A>,
        produced: &Basket<A>,
    ) -> Result<Self, AccountError<A>> {
        let mut updated = self.clone();
        updated.withdraw(consumed)?;
        updated.deposit(produced)?;
        Ok(updated)
    }
}

impl<A> Default for Account<A> {
    fn default() -> Self {
        Self::new(Basket::new())
    }
}

impl<A: Eq + Hash> PartialEq for Account<A> {
    fn eq(&self, other: &Self) -> bool {
        self.balances == other.balances
    }
}

impl<A: Eq + Hash> Eq for Account<A> {}

impl<A> From<Basket<A>> for Account<A> {
    fn from(balances: Basket<A>) -> Self {
        Self::new(balances)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "A: Serialize",
    deserialize = "A: Deserialize<'de> + Eq + Hash"
))]
pub enum AccountError<A: Eq + Hash> {
    #[error("insufficient balance")]
    InsufficientBalance { shortfall: Basket<A> },
    #[error("account balance overflow")]
    Overflow { asset: A },
}
