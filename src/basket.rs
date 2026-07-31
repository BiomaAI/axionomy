use crate::{AssetAmount, Quantity, QuantityScalar};
use indexmap::IndexMap;
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
    /// Builds a canonical basket, rejecting duplicate asset identifiers.
    pub fn try_from_entries(
        entries: impl IntoIterator<Item = (A, Quantity<N>)>,
    ) -> Result<Self, BasketError<A>> {
        let mut basket = Self::new();
        for (asset, quantity) in entries {
            if basket.quantities.contains_key(&asset) {
                return Err(BasketError::DuplicateAsset { asset });
            }
            basket.insert(asset, quantity);
        }
        Ok(basket)
    }

    /// Builds a canonical basket from asset-qualified quantities.
    ///
    /// This is the preferred construction boundary for typed authoring
    /// adapters because it preserves the asset identity paired with every
    /// quantity and rejects duplicate assets.
    pub fn try_from_amounts(
        amounts: impl IntoIterator<Item = AssetAmount<A, N>>,
    ) -> Result<Self, BasketError<A>> {
        Self::try_from_entries(amounts.into_iter().map(AssetAmount::into_parts))
    }

    pub fn get(&self, asset: &A) -> Option<&Quantity<N>> {
        self.quantities.get(asset)
    }

    pub fn quantity(&self, asset: &A) -> Quantity<N> {
        self.quantities.get(asset).cloned().unwrap_or_default()
    }

    pub fn insert(&mut self, asset: A, quantity: Quantity<N>) -> Option<Quantity<N>> {
        if quantity.is_zero() {
            self.quantities.shift_remove(&asset)
        } else {
            self.quantities.insert(asset, quantity)
        }
    }

    pub fn insert_amount(&mut self, amount: AssetAmount<A, N>) -> Option<Quantity<N>> {
        let (asset, quantity) = amount.into_parts();
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
        self.len() == other.len()
            && self
                .quantities
                .iter()
                .all(|(asset, quantity)| other.quantities.get(asset) == Some(quantity))
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
        entries.into_iter().collect()
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
