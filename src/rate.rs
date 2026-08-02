use crate::{Account, AccountDelta, Basket, Quantity, QuantityScalar};
use indexmap::IndexMap;
use num_traits::{CheckedAdd, Zero};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hash;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Rate<Role, A, N = u64> {
    consume: BTreeMap<Role, Basket<A, N>>,
    produce: BTreeMap<Role, Basket<A, N>>,
    preserve: BTreeMap<Role, Basket<A, N>>,
    roles: BTreeSet<Role>,
    distinct: BTreeSet<(Role, Role)>,
}

impl<Role, A, N> Rate<Role, A, N>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    pub fn new() -> Self {
        Self {
            consume: BTreeMap::new(),
            produce: BTreeMap::new(),
            preserve: BTreeMap::new(),
            roles: BTreeSet::new(),
            distinct: BTreeSet::new(),
        }
    }

    pub fn try_consume(
        mut self,
        role: Role,
        basket: Basket<A, N>,
    ) -> Result<Self, RateError<Role, A>> {
        self.roles.insert(role.clone());
        merge(&mut self.consume, role.clone(), basket)
            .map_err(|asset| RateError::BasketOverflow { role, asset })?;
        Ok(self)
    }

    pub fn consume(self, role: Role, basket: Basket<A, N>) -> Self {
        match self.try_consume(role, basket) {
            Ok(rate) => rate,
            Err(_) => panic!("rate consume basket quantity overflow"),
        }
    }

    pub fn try_produce(
        mut self,
        role: Role,
        basket: Basket<A, N>,
    ) -> Result<Self, RateError<Role, A>> {
        self.roles.insert(role.clone());
        merge(&mut self.produce, role.clone(), basket)
            .map_err(|asset| RateError::BasketOverflow { role, asset })?;
        Ok(self)
    }

    pub fn produce(self, role: Role, basket: Basket<A, N>) -> Self {
        match self.try_produce(role, basket) {
            Ok(rate) => rate,
            Err(_) => panic!("rate produce basket quantity overflow"),
        }
    }

    pub fn try_preserve(
        mut self,
        role: Role,
        basket: Basket<A, N>,
    ) -> Result<Self, RateError<Role, A>> {
        self.roles.insert(role.clone());
        merge(&mut self.preserve, role.clone(), basket)
            .map_err(|asset| RateError::BasketOverflow { role, asset })?;
        Ok(self)
    }

    pub fn preserve(self, role: Role, basket: Basket<A, N>) -> Self {
        match self.try_preserve(role, basket) {
            Ok(rate) => rate,
            Err(_) => panic!("rate preserve basket quantity overflow"),
        }
    }

    pub fn distinct(mut self, left: Role, right: Role) -> Self {
        self.roles.insert(left.clone());
        self.roles.insert(right.clone());
        let pair = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.distinct.insert(pair);
        self
    }

    pub fn roles(&self) -> impl Iterator<Item = &Role> {
        self.roles.iter()
    }

    pub fn consumed(&self, role: &Role) -> Option<&Basket<A, N>> {
        self.consume.get(role)
    }

    pub fn produced(&self, role: &Role) -> Option<&Basket<A, N>> {
        self.produce.get(role)
    }

    pub fn preserved(&self, role: &Role) -> Option<&Basket<A, N>> {
        self.preserve.get(role)
    }

    pub fn distinct_roles(&self) -> impl Iterator<Item = &(Role, Role)> {
        self.distinct.iter()
    }
}

impl<Role, A, N> Default for Rate<Role, A, N>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    fn default() -> Self {
        Self::new()
    }
}

fn merge<Role, A, N>(
    target: &mut BTreeMap<Role, Basket<A, N>>,
    role: Role,
    basket: Basket<A, N>,
) -> Result<(), A>
where
    Role: Ord,
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    target.entry(role).or_default().checked_add(&basket)
}

impl<Role, A, N> Serialize for Rate<Role, A, N>
where
    Role: Serialize + Ord,
    A: Serialize,
    N: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Rate", 4)?;
        state.serialize_field("consume", &self.consume.iter().collect::<Vec<_>>())?;
        state.serialize_field("produce", &self.produce.iter().collect::<Vec<_>>())?;
        state.serialize_field("preserve", &self.preserve.iter().collect::<Vec<_>>())?;
        state.serialize_field("distinct", &self.distinct.iter().collect::<Vec<_>>())?;
        state.end()
    }
}

#[derive(Deserialize)]
#[serde(bound(
    deserialize = "Role: Deserialize<'de> + Clone + Ord, A: Deserialize<'de> + Clone + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
struct RateData<Role, A, N> {
    consume: Vec<(Role, Basket<A, N>)>,
    produce: Vec<(Role, Basket<A, N>)>,
    preserve: Vec<(Role, Basket<A, N>)>,
    distinct: Vec<(Role, Role)>,
}

impl<'de, Role, A, N> Deserialize<'de> for Rate<Role, A, N>
where
    Role: Deserialize<'de> + Clone + Ord,
    A: Deserialize<'de> + Clone + Eq + Hash,
    N: Deserialize<'de> + QuantityScalar,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = RateData::<Role, A, N>::deserialize(deserializer)?;
        let mut rate = Self::new();
        for (role, basket) in data.consume {
            rate = rate
                .try_consume(role, basket)
                .map_err(serde::de::Error::custom)?;
        }
        for (role, basket) in data.produce {
            rate = rate
                .try_produce(role, basket)
                .map_err(serde::de::Error::custom)?;
        }
        for (role, basket) in data.preserve {
            rate = rate
                .try_preserve(role, basket)
                .map_err(serde::de::Error::custom)?;
        }
        for (left, right) in data.distinct {
            rate = rate.distinct(left, right);
        }
        Ok(rate)
    }
}

impl<Role, A, N> JsonSchema for Rate<Role, A, N>
where
    Role: JsonSchema,
    A: JsonSchema,
    N: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!(
            "Rate_{}_{}_{}",
            Role::schema_name(),
            A::schema_name(),
            N::schema_name()
        )
        .into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        #[allow(dead_code)]
        #[derive(JsonSchema)]
        struct RateSchema<Role, A, N> {
            consume: Vec<(Role, Basket<A, N>)>,
            produce: Vec<(Role, Basket<A, N>)>,
            preserve: Vec<(Role, Basket<A, N>)>,
            distinct: Vec<(Role, Role)>,
        }

        generator.subschema_for::<RateSchema<Role, A, N>>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RateError<Role, A> {
    #[error("rate basket quantity overflow")]
    BasketOverflow { role: Role, asset: A },
}

#[derive(Debug, Clone)]
pub struct LinearInvariant<A> {
    name: String,
    weights: IndexMap<A, i64>,
}

impl<A> LinearInvariant<A>
where
    A: Eq + Hash,
{
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            weights: IndexMap::new(),
        }
    }

    pub fn weight(mut self, asset: A, weight: i64) -> Self {
        if weight == 0 {
            self.weights.shift_remove(&asset);
        } else {
            self.weights.insert(asset, weight);
        }
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn weights(&self) -> impl Iterator<Item = (&A, i64)> {
        self.weights.iter().map(|(asset, weight)| (asset, *weight))
    }

    pub fn measure<AccountId, Holder, N>(
        &self,
        accounts: &IndexMap<AccountId, Holder>,
    ) -> Option<N::SignedMeasure>
    where
        A: Clone,
        Holder: AsRef<Account<A, N>>,
        N: QuantityScalar,
    {
        let mut total = N::SignedMeasure::zero();
        for account in accounts.values() {
            for (asset, quantity) in account.as_ref().balances().iter() {
                let coefficient = *self.weights.get(asset).unwrap_or(&0);
                let weighted = quantity.as_scalar().checked_weighted(coefficient)?;
                total = CheckedAdd::checked_add(&total, &weighted)?;
            }
        }
        Some(total)
    }

    /// Checks conservation from only the accounts changed by an exchange.
    ///
    /// Linear invariants are location-independent sums, so an exchange
    /// preserves one exactly when the weighted consumed and produced baskets
    /// have equal measures. This avoids rescanning untouched accounts on the
    /// successful application path.
    pub(crate) fn conserves<AccountId, N>(
        &self,
        deltas: &[AccountDelta<AccountId, A, N>],
    ) -> Option<bool>
    where
        N: QuantityScalar,
    {
        let mut consumed = N::SignedMeasure::zero();
        let mut produced = N::SignedMeasure::zero();
        for delta in deltas {
            consumed = self.checked_add_basket_measure(consumed, delta.consumed())?;
            produced = self.checked_add_basket_measure(produced, delta.produced())?;
        }
        Some(consumed == produced)
    }

    fn checked_add_basket_measure<N>(
        &self,
        mut total: N::SignedMeasure,
        basket: &Basket<A, N>,
    ) -> Option<N::SignedMeasure>
    where
        N: QuantityScalar,
    {
        for (asset, quantity) in basket.iter() {
            let coefficient = *self.weights.get(asset).unwrap_or(&0);
            let weighted = quantity.as_scalar().checked_weighted(coefficient)?;
            total = CheckedAdd::checked_add(&total, &weighted)?;
        }
        Some(total)
    }
}

impl<A> Serialize for LinearInvariant<A>
where
    A: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("LinearInvariant", 2)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("weights", &self.weights.iter().collect::<Vec<_>>())?;
        state.end()
    }
}

#[derive(Deserialize)]
struct InvariantData<A> {
    name: String,
    weights: Vec<(A, i64)>,
}

impl<'de, A> Deserialize<'de> for LinearInvariant<A>
where
    A: Deserialize<'de> + Eq + Hash,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = InvariantData::<A>::deserialize(deserializer)?;
        let mut invariant = Self::new(data.name);
        for (asset, weight) in data.weights {
            if invariant.weights.contains_key(&asset) {
                return Err(serde::de::Error::custom(
                    "linear invariant contains a duplicate asset weight",
                ));
            }
            invariant = invariant.weight(asset, weight);
        }
        Ok(invariant)
    }
}

impl<A> JsonSchema for LinearInvariant<A>
where
    A: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("LinearInvariant_{}", A::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        #[allow(dead_code)]
        #[derive(JsonSchema)]
        struct LinearInvariantSchema<A> {
            name: String,
            weights: Vec<(A, i64)>,
        }

        generator.subschema_for::<LinearInvariantSchema<A>>()
    }
}

pub fn basket<A, const LEN: usize>(entries: [(A, u64); LEN]) -> Basket<A>
where
    A: Eq + Hash,
{
    entries
        .map(|(asset, quantity)| (asset, Quantity::new(quantity)))
        .into()
}
