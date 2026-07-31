use crate::{Quantity, QuantityScalar};
use std::collections::HashMap;
use std::hash::Hash;
use std::iter::FromIterator;

#[derive(Debug, Clone)]
pub struct Basket<A, N = u64> {
    quantities: HashMap<A, Quantity<N>>,
}

impl<A, N> Basket<A, N> {
    pub fn new() -> Self {
        Self {
            quantities: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.quantities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.quantities.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&A, &Quantity<N>)> {
        self.quantities.iter()
    }

    /// Iterates by asset order for stable observations, reports, and encoding.
    pub fn iter_sorted(&self) -> impl Iterator<Item = (&A, &Quantity<N>)>
    where
        A: Ord,
    {
        let mut entries = self.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(asset, _)| *asset);
        entries.into_iter()
    }
}

impl<A, N> Basket<A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    pub fn get(&self, asset: &A) -> Option<&Quantity<N>> {
        self.quantities.get(asset)
    }

    pub fn quantity(&self, asset: &A) -> Quantity<N> {
        self.quantities.get(asset).cloned().unwrap_or_default()
    }

    pub fn insert(&mut self, asset: A, quantity: Quantity<N>) -> Option<Quantity<N>> {
        if quantity.is_zero() {
            self.quantities.remove(&asset)
        } else {
            self.quantities.insert(asset, quantity)
        }
    }
}

impl<A, N> Basket<A, N>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    pub fn checked_add(&mut self, other: &Self) -> Result<(), A> {
        let mut updated = self.clone();

        for (asset, quantity) in other.iter() {
            let Some(quantity) = updated.quantity(asset).checked_add(quantity) else {
                return Err(asset.clone());
            };
            updated.insert(asset.clone(), quantity);
        }

        *self = updated;
        Ok(())
    }

    pub fn checked_scale(&self, units: &Quantity<N>) -> Result<Self, A> {
        let mut scaled = Self::new();

        for (asset, quantity) in self.iter() {
            let Some(quantity) = quantity.checked_mul(units) else {
                return Err(asset.clone());
            };
            scaled.insert(asset.clone(), quantity);
        }

        Ok(scaled)
    }

    pub fn shortfall(&self, required: &Self) -> Self {
        let mut shortfall = Self::new();

        for (asset, required_quantity) in required.iter() {
            let available = self.quantity(asset);
            if available < *required_quantity {
                let missing = required_quantity
                    .checked_sub(&available)
                    .expect("available quantity is smaller than required quantity");
                shortfall.insert(asset.clone(), missing);
            }
        }

        shortfall
    }

    pub fn contains(&self, required: &Self) -> bool {
        self.shortfall(required).is_empty()
    }
}

impl<A, N> Default for Basket<A, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A, N> PartialEq for Basket<A, N>
where
    A: Eq + Hash,
    N: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.quantities == other.quantities
    }
}

impl<A, N> Eq for Basket<A, N>
where
    A: Eq + Hash,
    N: Eq,
{
}

impl<A, N> FromIterator<(A, Quantity<N>)> for Basket<A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    fn from_iter<T>(entries: T) -> Self
    where
        T: IntoIterator<Item = (A, Quantity<N>)>,
    {
        let mut basket = Self::new();
        for (asset, quantity) in entries {
            basket.insert(asset, quantity);
        }
        basket
    }
}

impl<A, N, const LEN: usize> From<[(A, Quantity<N>); LEN]> for Basket<A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    fn from(entries: [(A, Quantity<N>); LEN]) -> Self {
        entries.into_iter().collect()
    }
}
