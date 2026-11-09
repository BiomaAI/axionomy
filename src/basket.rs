use crate::{AssetAmount, Quantity};
use indexmap::IndexMap;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::hash::Hash;
use thiserror::Error;

/// A sparse heterogeneous asset multiset with stable insertion order.
///
/// Zero quantities are never stored, so two baskets are equal exactly when
/// every asset has the same quantity.
#[derive(Debug, Clone)]
pub struct Basket<A> {
    quantities: IndexMap<A, Quantity>,
}

impl<A> Basket<A> {
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

    pub fn iter(&self) -> impl Iterator<Item = (&A, &Quantity)> {
        self.quantities.iter()
    }

    /// Iterates by asset order when an ontology-defined sorted view is needed.
    pub fn iter_sorted(&self) -> impl Iterator<Item = (&A, &Quantity)>
    where
        A: Ord,
    {
        let mut entries: Vec<_> = self.iter().collect();
        entries.sort_by_key(|(asset, _)| *asset);
        entries.into_iter()
    }
}

impl<A: Eq + Hash> Basket<A> {
    /// Builds a canonical basket, rejecting duplicate asset identifiers.
    ///
    /// Every identifier is checked, including those with zero quantities,
    /// before zeros are dropped.
    pub fn try_from_entries(
        entries: impl IntoIterator<Item = (A, Quantity)>,
    ) -> Result<Self, BasketError<A>> {
        let mut quantities = IndexMap::new();
        for (asset, quantity) in entries {
            if quantities.contains_key(&asset) {
                return Err(BasketError::DuplicateAsset { asset });
            }
            quantities.insert(asset, quantity);
        }
        quantities.retain(|_, quantity: &mut Quantity| !quantity.is_zero());
        Ok(Self { quantities })
    }

    /// Builds a canonical basket from asset-qualified quantities, rejecting
    /// duplicate assets. This is the construction boundary for typed
    /// authoring adapters.
    pub fn try_from_amounts(
        amounts: impl IntoIterator<Item = AssetAmount<A>>,
    ) -> Result<Self, BasketError<A>> {
        Self::try_from_entries(amounts.into_iter().map(AssetAmount::into_parts))
    }

    pub fn get(&self, asset: &A) -> Option<&Quantity> {
        self.quantities.get(asset)
    }

    /// The quantity of `asset`, zero when absent.
    pub fn quantity(&self, asset: &A) -> Quantity {
        self.get(asset).copied().unwrap_or_default()
    }

    /// Sets the quantity of `asset`; zero removes it.
    pub fn insert(&mut self, asset: A, amount: Quantity) -> Option<Quantity> {
        if amount.is_zero() {
            self.quantities.shift_remove(&asset)
        } else {
            self.quantities.insert(asset, amount)
        }
    }
}

impl<A: Clone + Eq + Hash> Basket<A> {
    /// Adds `other`, or changes nothing and returns the first overflowing asset.
    pub fn checked_add(&mut self, other: &Self) -> Result<(), A> {
        let mut updated = self.clone();
        for (asset, amount) in other.iter() {
            let total = updated
                .quantity(asset)
                .checked_add(*amount)
                .ok_or_else(|| asset.clone())?;
            updated.insert(asset.clone(), total);
        }
        *self = updated;
        Ok(())
    }

    /// Multiplies every quantity by `units`, or returns the first overflowing asset.
    pub fn checked_scale(&self, units: Quantity) -> Result<Self, A> {
        let mut scaled = Self::new();
        for (asset, amount) in self.iter() {
            let product = amount.checked_mul(units).ok_or_else(|| asset.clone())?;
            scaled.insert(asset.clone(), product);
        }
        Ok(scaled)
    }

    /// What this basket lacks to cover `required`.
    pub fn shortfall(&self, required: &Self) -> Self {
        let mut shortfall = Self::new();
        for (asset, needed) in required.iter() {
            if let Some(missing) = needed.checked_sub(self.quantity(asset)) {
                shortfall.insert(asset.clone(), missing);
            }
        }
        shortfall
    }

    pub fn contains(&self, required: &Self) -> bool {
        self.shortfall(required).is_empty()
    }
}

impl<A> Default for Basket<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Eq + Hash> PartialEq for Basket<A> {
    fn eq(&self, other: &Self) -> bool {
        self.quantities == other.quantities
    }
}

impl<A: Eq + Hash> Eq for Basket<A> {}

/// Collects entries into a basket.
///
/// # Panics
///
/// If an asset appears twice; use [`Basket::try_from_entries`] for untrusted input.
impl<A: Eq + Hash> FromIterator<(A, Quantity)> for Basket<A> {
    fn from_iter<T: IntoIterator<Item = (A, Quantity)>>(entries: T) -> Self {
        Self::try_from_entries(entries)
            .unwrap_or_else(|_| panic!("basket entries must have unique assets"))
    }
}

impl<A: Eq + Hash, const LEN: usize> From<[(A, Quantity); LEN]> for Basket<A> {
    fn from(entries: [(A, Quantity); LEN]) -> Self {
        entries.into_iter().collect()
    }
}

impl<A: Serialize> Serialize for Basket<A> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.len()))?;
        for entry in self.iter() {
            sequence.serialize_element(&entry)?;
        }
        sequence.end()
    }
}

impl<'de, A: Deserialize<'de> + Eq + Hash> Deserialize<'de> for Basket<A> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries = Vec::<(A, Quantity)>::deserialize(deserializer)?;
        Self::try_from_entries(entries).map_err(serde::de::Error::custom)
    }
}

impl<A: JsonSchema> JsonSchema for Basket<A> {
    fn schema_name() -> Cow<'static, str> {
        format!("Basket_{}", A::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        generator.subschema_for::<Vec<(A, Quantity)>>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BasketError<A> {
    #[error("basket contains a duplicate asset")]
    DuplicateAsset { asset: A },
}
