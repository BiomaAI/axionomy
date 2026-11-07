use crate::{AssetAmount, Quantity, QuantityScalar};
use indexmap::IndexMap;
use num_traits::Zero;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::hash::Hash;
use std::iter::FromIterator;
use thiserror::Error;

/// A sparse heterogeneous asset multiset with stable insertion order.
#[derive(Debug, Clone)]
pub struct Basket<A, N = u64> {
    quantities: IndexMap<A, Quantity<N>>,
}

// Storage adapter: construction filters quantities without mutating them.
// The proof models this thin IndexMap boundary explicitly.
pub(crate) fn retain_quantities<A, N>(
    quantities: &mut IndexMap<A, Quantity<N>>,
    mut keep: impl FnMut(&Quantity<N>) -> bool,
) {
    quantities.retain(|_, quantity| keep(quantity));
}

// Shared production constructor. Its minimal internal bounds avoid bringing
// unrelated scalar operations into the proof; public bounds remain unchanged.
pub(crate) fn construct_entries<A, N>(
    entries: impl IntoIterator<Item = (A, Quantity<N>)>,
) -> Result<Basket<A, N>, BasketError<A>>
where
    A: Eq + Hash,
    N: Zero,
{
    // Zero entries still establish that an asset has appeared in the input.
    let mut quantities = IndexMap::new();
    for (asset, quantity) in entries {
        if quantities.contains_key(&asset) {
            return Err(BasketError::DuplicateAsset { asset });
        }
        quantities.insert(asset, quantity);
    }
    retain_quantities(&mut quantities, |quantity| !quantity.scalar_is_zero());
    Ok(Basket { quantities })
}

// Convert one typed amount per next() call, preserving duplicate rejection's
// short circuit even when the unused iterator tail panics or never terminates.
struct AmountEntries<I>(I);

impl<A, N, I> Iterator for AmountEntries<I>
where
    I: Iterator<Item = AssetAmount<A, N>>,
{
    type Item = (A, Quantity<N>);

    #[allow(clippy::manual_map)] // Keep the primitive next/typed conversion explicit for extraction.
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(value) => Some(value.into_parts()),
            None => None,
        }
    }
}

// Keep sorting's temporary mutable slice borrow inside a generic owned-vector
// boundary. Elements may themselves borrow the basket; no cloning is needed.
fn sorted_entries<T>(
    mut entries: Vec<T>,
    compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
) -> Vec<T> {
    entries.sort_by(compare);
    entries
}

impl<A, N> Basket<A, N> {
    pub fn new() -> Self {
        Self {
            quantities: IndexMap::new(),
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

    /// Iterates by asset order when an ontology-defined sorted view is needed.
    pub fn iter_sorted(&self) -> impl Iterator<Item = (&A, &Quantity<N>)>
    where
        A: Ord,
    {
        let entries = Vec::from_iter(self.iter());
        sorted_entries(entries, |(left, _), (right, _)| left.cmp(right)).into_iter()
    }
}

impl<A, N> Basket<A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    /// Builds a canonical basket, rejecting duplicate asset identifiers.
    pub fn try_from_entries(
        entries: impl IntoIterator<Item = (A, Quantity<N>)>,
    ) -> Result<Self, BasketError<A>> {
        construct_entries(entries)
    }

    /// Builds a canonical basket from asset-qualified quantities.
    ///
    /// This is the preferred construction boundary for typed authoring
    /// adapters because it preserves the asset identity paired with every
    /// quantity and rejects duplicate assets.
    pub fn try_from_amounts(
        amounts: impl IntoIterator<Item = AssetAmount<A, N>>,
    ) -> Result<Self, BasketError<A>> {
        Self::try_from_entries(AmountEntries(amounts.into_iter()))
    }

    pub fn get(&self, asset: &A) -> Option<&Quantity<N>> {
        self.quantities.get(asset)
    }

    pub fn quantity(&self, asset: &A) -> Quantity<N> {
        self.quantities.get(asset).cloned().unwrap_or_default()
    }

    pub fn insert(&mut self, asset: A, amount: Quantity<N>) -> Option<Quantity<N>> {
        if amount.is_zero() {
            self.quantities.shift_remove(&asset)
        } else {
            self.quantities.insert(asset, amount)
        }
    }

    pub fn insert_amount(&mut self, value: AssetAmount<A, N>) -> Option<Quantity<N>> {
        let (asset, quantity) = value.into_parts();
        self.insert(asset, quantity)
    }
}

impl<A, N> Basket<A, N>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    pub fn checked_add(&mut self, other: &Self) -> Result<(), A> {
        let mut updated = self.clone();

        for (asset, amount) in other.iter() {
            let Some(amount) = updated.quantity(asset).checked_add(amount) else {
                return Err(asset.clone());
            };
            updated.insert(asset.clone(), amount);
        }

        *self = updated;
        Ok(())
    }

    pub fn checked_scale(&self, units: &Quantity<N>) -> Result<Self, A> {
        let mut scaled = Self::new();

        for (asset, amount) in self.iter() {
            let Some(amount) = amount.checked_mul(units) else {
                return Err(asset.clone());
            };
            scaled.insert(asset.clone(), amount);
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
        if self.len() != other.len() {
            return false;
        }
        for (asset, amount) in self.iter() {
            if other.quantities.get(asset) == Some(amount) {
                continue;
            }
            return false;
        }
        true
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
        match Self::try_from_entries(entries) {
            Ok(basket) => basket,
            Err(_) => panic!("basket entries must have unique assets"),
        }
    }
}

impl<A, N, const LEN: usize> From<[(A, Quantity<N>); LEN]> for Basket<A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    fn from(entries: [(A, Quantity<N>); LEN]) -> Self {
        Self::from_iter(entries)
    }
}

impl<A, N> Serialize for Basket<A, N>
where
    A: Serialize,
    N: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.len()))?;
        for entry in self.iter() {
            sequence.serialize_element(&entry)?;
        }
        sequence.end()
    }
}

impl<'de, A, N> Deserialize<'de> for Basket<A, N>
where
    A: Deserialize<'de> + Eq + Hash,
    N: Deserialize<'de> + QuantityScalar,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(A, Quantity<N>)>::deserialize(deserializer)?;
        Self::try_from_entries(entries).map_err(serde::de::Error::custom)
    }
}

impl<A, N> JsonSchema for Basket<A, N>
where
    A: JsonSchema,
    N: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("Basket_{}_{}", A::schema_name(), N::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        generator.subschema_for::<Vec<(A, Quantity<N>)>>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BasketError<A> {
    #[error("basket contains a duplicate asset")]
    DuplicateAsset { asset: A },
}
