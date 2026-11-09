use crate::{Account, AccountDelta, Basket, Quantity, Rounding};
use indexmap::IndexMap;
use ruint::aliases::U384;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::hash::Hash;
use thiserror::Error;

/// An exact, serializable quantity expression evaluated against the economy's
/// pre-exchange snapshot.
///
/// This keeps state-dependent exchange laws inside Axionomy authority without
/// embedding domain callbacks in the kernel. Expressions intentionally expose
/// only exact integer arithmetic. `MulDiv` is the only division: it computes
/// `left × right ÷ denominator` from a full-width product and always names its
/// [`Rounding`] direction, so scaled ratios cannot overflow in the
/// intermediate product and nothing rounds implicitly. Round amounts a
/// participant pays up and amounts a participant receives down, so rounding
/// dust stays with the counterparty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuantityExpression<Role, A> {
    Constant {
        value: Quantity,
    },
    Units,
    Parameter {
        name: String,
    },
    Balance {
        role: Role,
        asset: A,
    },
    Add {
        left: Box<Self>,
        right: Box<Self>,
    },
    Subtract {
        left: Box<Self>,
        right: Box<Self>,
    },
    Multiply {
        left: Box<Self>,
        right: Box<Self>,
    },
    Minimum {
        left: Box<Self>,
        right: Box<Self>,
    },
    Maximum {
        left: Box<Self>,
        right: Box<Self>,
    },
    /// `left × right ÷ denominator` from a full-width intermediate product,
    /// with an inexact quotient rounded in the stated direction.
    MulDiv {
        left: Box<Self>,
        right: Box<Self>,
        denominator: Box<Self>,
        rounding: Rounding,
    },
}

impl<Role, A> QuantityExpression<Role, A> {
    pub const fn constant(value: u64) -> Self {
        Self::Constant {
            value: Quantity::new(value),
        }
    }

    pub const fn units() -> Self {
        Self::Units
    }

    pub fn parameter(name: impl Into<String>) -> Self {
        Self::Parameter { name: name.into() }
    }

    pub const fn balance(role: Role, asset: A) -> Self {
        Self::Balance { role, asset }
    }

    pub fn minimum(left: Self, right: Self) -> Self {
        Self::Minimum {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn maximum(left: Self, right: Self) -> Self {
        Self::Maximum {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// `left × right ÷ denominator` with a full-width product. Use
    /// [`Rounding::Ceil`] for amounts a participant pays and
    /// [`Rounding::Floor`] for amounts a participant receives.
    pub fn mul_div(left: Self, right: Self, denominator: Self, rounding: Rounding) -> Self {
        Self::MulDiv {
            left: Box::new(left),
            right: Box::new(right),
            denominator: Box::new(denominator),
            rounding,
        }
    }

    /// `numerator ÷ denominator`, rounded as stated: a mul-div by one.
    pub fn divide(numerator: Self, denominator: Self, rounding: Rounding) -> Self {
        Self::mul_div(numerator, Self::constant(1), denominator, rounding)
    }

    /// Every subexpression, depth first, starting with this one.
    fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Self)) {
        visit(self);
        match self {
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right } => {
                left.walk(visit);
                right.walk(visit);
            }
            Self::MulDiv {
                left,
                right,
                denominator,
                ..
            } => {
                left.walk(visit);
                right.walk(visit);
                denominator.walk(visit);
            }
            Self::Constant { .. } | Self::Units | Self::Parameter { .. } | Self::Balance { .. } => {
            }
        }
    }

    fn roles(&self) -> Vec<&Role> {
        let mut roles = Vec::new();
        self.walk(&mut |expression| {
            if let Self::Balance { role, .. } = expression {
                roles.push(role);
            }
        });
        roles
    }

    fn assets(&self) -> Vec<&A> {
        let mut assets = Vec::new();
        self.walk(&mut |expression| {
            if let Self::Balance { asset, .. } = expression {
                assets.push(asset);
            }
        });
        assets
    }

    fn parameters(&self) -> Vec<&str> {
        let mut parameters = Vec::new();
        self.walk(&mut |expression| {
            if let Self::Parameter { name } = expression {
                parameters.push(name.as_str());
            }
        });
        parameters
    }
}

/// `left + right`, `left - right` and `left * right` build checked arithmetic:
/// evaluation rejects a sum or product above `Quantity::MAX` and a difference
/// below zero.
macro_rules! expression_operator {
    ($trait:ident, $method:ident, $variant:ident) => {
        impl<Role, A> std::ops::$trait for QuantityExpression<Role, A> {
            type Output = Self;

            fn $method(self, right: Self) -> Self {
                Self::$variant {
                    left: Box::new(self),
                    right: Box::new(right),
                }
            }
        }
    };
}

expression_operator!(Add, add, Add);
expression_operator!(Sub, sub, Subtract);
expression_operator!(Mul, mul, Multiply);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComputedAmount<Role, A> {
    role: Role,
    asset: A,
    quantity: QuantityExpression<Role, A>,
}

impl<Role, A> ComputedAmount<Role, A> {
    pub const fn role(&self) -> &Role {
        &self.role
    }

    pub const fn asset(&self) -> &A {
        &self.asset
    }

    pub const fn quantity(&self) -> &QuantityExpression<Role, A> {
        &self.quantity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuantityComparison {
    Equal,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateCondition<Role, A> {
    name: String,
    left: QuantityExpression<Role, A>,
    comparison: QuantityComparison,
    right: QuantityExpression<Role, A>,
}

impl<Role, A> RateCondition<Role, A> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn left(&self) -> &QuantityExpression<Role, A> {
        &self.left
    }

    pub const fn comparison(&self) -> QuantityComparison {
        self.comparison
    }

    pub const fn right(&self) -> &QuantityExpression<Role, A> {
        &self.right
    }
}

#[derive(Debug, Clone)]
pub struct Rate<Role, A> {
    consume: BTreeMap<Role, Basket<A>>,
    produce: BTreeMap<Role, Basket<A>>,
    preserve: BTreeMap<Role, Basket<A>>,
    roles: BTreeSet<Role>,
    distinct: BTreeSet<(Role, Role)>,
    computed_consume: Vec<ComputedAmount<Role, A>>,
    computed_produce: Vec<ComputedAmount<Role, A>>,
    computed_preserve: Vec<ComputedAmount<Role, A>>,
    conditions: Vec<RateCondition<Role, A>>,
}

impl<Role, A> Rate<Role, A>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            consume: BTreeMap::new(),
            produce: BTreeMap::new(),
            preserve: BTreeMap::new(),
            roles: BTreeSet::new(),
            distinct: BTreeSet::new(),
            computed_consume: Vec::new(),
            computed_produce: Vec::new(),
            computed_preserve: Vec::new(),
            conditions: Vec::new(),
        }
    }

    /// Consumes `basket` per unit from the account bound to `role`.
    ///
    /// # Panics
    ///
    /// If repeated baskets for one role add up beyond `Quantity::MAX`.
    pub fn consume(self, role: Role, basket: Basket<A>) -> Self {
        self.with_static(Direction::Consume, role, basket)
            .unwrap_or_else(|_| panic!("rate consume basket quantity overflow"))
    }

    /// Produces `basket` per unit into the account bound to `role`.
    ///
    /// # Panics
    ///
    /// If repeated baskets for one role add up beyond `Quantity::MAX`.
    pub fn produce(self, role: Role, basket: Basket<A>) -> Self {
        self.with_static(Direction::Produce, role, basket)
            .unwrap_or_else(|_| panic!("rate produce basket quantity overflow"))
    }

    /// Requires `basket` in the account bound to `role` without consuming it.
    ///
    /// # Panics
    ///
    /// If repeated baskets for one role add up beyond `Quantity::MAX`.
    pub fn preserve(self, role: Role, basket: Basket<A>) -> Self {
        self.with_static(Direction::Preserve, role, basket)
            .unwrap_or_else(|_| panic!("rate preserve basket quantity overflow"))
    }

    fn with_static(
        mut self,
        direction: Direction,
        role: Role,
        basket: Basket<A>,
    ) -> Result<Self, RateError<Role, A>> {
        self.roles.insert(role.clone());
        let target = match direction {
            Direction::Consume => &mut self.consume,
            Direction::Produce => &mut self.produce,
            Direction::Preserve => &mut self.preserve,
        };
        target
            .entry(role.clone())
            .or_default()
            .checked_add(&basket)
            .map_err(|asset| RateError::BasketOverflow { role, asset })?;
        Ok(self)
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

    pub fn consume_computed(
        self,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A>,
    ) -> Self {
        self.with_computed(Direction::Consume, role, asset, quantity)
    }

    pub fn produce_computed(
        self,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A>,
    ) -> Self {
        self.with_computed(Direction::Produce, role, asset, quantity)
    }

    pub fn preserve_computed(
        self,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A>,
    ) -> Self {
        self.with_computed(Direction::Preserve, role, asset, quantity)
    }

    fn with_computed(
        mut self,
        direction: Direction,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A>,
    ) -> Self {
        self.roles.insert(role.clone());
        self.roles.extend(quantity.roles().into_iter().cloned());
        let amount = ComputedAmount {
            role,
            asset,
            quantity,
        };
        match direction {
            Direction::Consume => self.computed_consume.push(amount),
            Direction::Produce => self.computed_produce.push(amount),
            Direction::Preserve => self.computed_preserve.push(amount),
        }
        self
    }

    pub fn condition(
        mut self,
        name: impl Into<String>,
        left: QuantityExpression<Role, A>,
        comparison: QuantityComparison,
        right: QuantityExpression<Role, A>,
    ) -> Self {
        self.roles.extend(left.roles().into_iter().cloned());
        self.roles.extend(right.roles().into_iter().cloned());
        self.conditions.push(RateCondition {
            name: name.into(),
            left,
            comparison,
            right,
        });
        self
    }

    pub fn roles(&self) -> impl Iterator<Item = &Role> {
        self.roles.iter()
    }

    pub(crate) fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    pub fn consumed(&self, role: &Role) -> Option<&Basket<A>> {
        self.consume.get(role)
    }

    pub fn produced(&self, role: &Role) -> Option<&Basket<A>> {
        self.produce.get(role)
    }

    pub fn preserved(&self, role: &Role) -> Option<&Basket<A>> {
        self.preserve.get(role)
    }

    pub fn distinct_roles(&self) -> impl Iterator<Item = &(Role, Role)> {
        self.distinct.iter()
    }

    pub fn computed_consumed(&self) -> &[ComputedAmount<Role, A>] {
        &self.computed_consume
    }

    pub fn computed_produced(&self) -> &[ComputedAmount<Role, A>] {
        &self.computed_produce
    }

    pub fn computed_preserved(&self) -> &[ComputedAmount<Role, A>] {
        &self.computed_preserve
    }

    pub fn conditions(&self) -> &[RateCondition<Role, A>] {
        &self.conditions
    }

    fn computed(&self) -> impl Iterator<Item = &ComputedAmount<Role, A>> {
        self.computed_consume
            .iter()
            .chain(&self.computed_produce)
            .chain(&self.computed_preserve)
    }

    fn expressions(&self) -> impl Iterator<Item = &QuantityExpression<Role, A>> {
        self.computed().map(ComputedAmount::quantity).chain(
            self.conditions
                .iter()
                .flat_map(|condition| [&condition.left, &condition.right]),
        )
    }

    /// Names of every parameter the rate's laws and conditions read.
    pub fn parameter_names(&self) -> BTreeSet<&str> {
        self.expressions()
            .flat_map(QuantityExpression::parameters)
            .collect()
    }

    /// Assets named by computed effects and the balances their laws read, in
    /// declaration order and with repeats.
    pub fn computed_asset_keys(&self) -> Vec<&A> {
        let mut assets = Vec::new();
        for amount in self.computed() {
            assets.push(&amount.asset);
            assets.extend(amount.quantity.assets());
        }
        for condition in &self.conditions {
            assets.extend(condition.left.assets());
            assets.extend(condition.right.assets());
        }
        assets
    }

    /// Every asset the rate touches or reads.
    pub(crate) fn asset_keys(&self) -> BTreeSet<&A>
    where
        A: Ord,
    {
        [&self.consume, &self.produce, &self.preserve]
            .into_iter()
            .flat_map(BTreeMap::values)
            .flat_map(|basket| basket.iter().map(|(asset, _)| asset))
            .chain(self.computed_asset_keys())
            .collect()
    }
}

impl<Role, A> Default for Rate<Role, A>
where
    Role: Clone + Ord,
    A: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Consume,
    Produce,
    Preserve,
}

impl<Role, A> Serialize for Rate<Role, A>
where
    Role: Serialize + Ord,
    A: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Rate", 8)?;
        state.serialize_field("consume", &self.consume.iter().collect::<Vec<_>>())?;
        state.serialize_field("produce", &self.produce.iter().collect::<Vec<_>>())?;
        state.serialize_field("preserve", &self.preserve.iter().collect::<Vec<_>>())?;
        state.serialize_field("distinct", &self.distinct.iter().collect::<Vec<_>>())?;
        state.serialize_field("computed_consume", &self.computed_consume)?;
        state.serialize_field("computed_produce", &self.computed_produce)?;
        state.serialize_field("computed_preserve", &self.computed_preserve)?;
        state.serialize_field("conditions", &self.conditions)?;
        state.end()
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(bound(deserialize = "Role: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash"))]
struct RateData<Role, A> {
    consume: Vec<(Role, Basket<A>)>,
    produce: Vec<(Role, Basket<A>)>,
    preserve: Vec<(Role, Basket<A>)>,
    distinct: Vec<(Role, Role)>,
    #[serde(default)]
    computed_consume: Vec<ComputedAmount<Role, A>>,
    #[serde(default)]
    computed_produce: Vec<ComputedAmount<Role, A>>,
    #[serde(default)]
    computed_preserve: Vec<ComputedAmount<Role, A>>,
    #[serde(default)]
    conditions: Vec<RateCondition<Role, A>>,
}

impl<'de, Role, A> Deserialize<'de> for Rate<Role, A>
where
    Role: Deserialize<'de> + Clone + Ord,
    A: Deserialize<'de> + Clone + Eq + Hash,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = RateData::<Role, A>::deserialize(deserializer)?;
        let mut rate = Self::new();
        let statics = [
            (Direction::Consume, data.consume),
            (Direction::Produce, data.produce),
            (Direction::Preserve, data.preserve),
        ];
        for (direction, baskets) in statics {
            for (role, basket) in baskets {
                rate = rate
                    .with_static(direction, role, basket)
                    .map_err(serde::de::Error::custom)?;
            }
        }
        for (left, right) in data.distinct {
            rate = rate.distinct(left, right);
        }
        let computed = [
            (Direction::Consume, data.computed_consume),
            (Direction::Produce, data.computed_produce),
            (Direction::Preserve, data.computed_preserve),
        ];
        for (direction, amounts) in computed {
            for amount in amounts {
                rate = rate.with_computed(direction, amount.role, amount.asset, amount.quantity);
            }
        }
        for condition in data.conditions {
            rate = rate.condition(
                condition.name,
                condition.left,
                condition.comparison,
                condition.right,
            );
        }
        Ok(rate)
    }
}

impl<Role: JsonSchema, A: JsonSchema> JsonSchema for Rate<Role, A> {
    fn schema_name() -> Cow<'static, str> {
        format!("Rate_{}_{}", Role::schema_name(), A::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        generator.subschema_for::<RateData<Role, A>>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
enum RateError<Role, A> {
    #[error("rate basket quantity overflow")]
    BasketOverflow { role: Role, asset: A },
}

/// The exact value of a linear invariant over some holdings: the sum of each
/// quantity times its asset's signed weight.
///
/// A quantity is below 2^256 and a weight's magnitude at most 2^63, so every
/// term is below 2^319; fewer than 2^64 terms fit in memory, so every sum is
/// below 2^383 and measuring never overflows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Measure {
    negative: bool,
    magnitude: U384,
}

impl Measure {
    fn from_parts(positive: U384, negative: U384) -> Self {
        if positive >= negative {
            Self {
                negative: false,
                magnitude: positive - negative,
            }
        } else {
            Self {
                negative: true,
                magnitude: negative - positive,
            }
        }
    }
}

impl fmt::Display for Measure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            write!(formatter, "-{}", self.magnitude)
        } else {
            write!(formatter, "{}", self.magnitude)
        }
    }
}

impl Serialize for Measure {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl JsonSchema for Measure {
    fn schema_name() -> Cow<'static, str> {
        "Measure".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "An exact signed integer, as decimal digits.",
            "type": "string",
            "pattern": "^-?[0-9]+$"
        })
    }
}

#[derive(Debug, Clone)]
pub struct LinearInvariant<A> {
    name: String,
    weights: IndexMap<A, i64>,
}

impl<A: Eq + Hash> LinearInvariant<A> {
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

    // Keep zero weights until all input identifiers have been checked. Removing
    // them earlier would let a repeated asset evade duplicate rejection.
    fn from_serialized_weights(name: String, entries: Vec<(A, i64)>) -> Result<Self, A> {
        let mut weights = IndexMap::new();
        for (asset, weight) in entries {
            if weights.contains_key(&asset) {
                return Err(asset);
            }
            weights.insert(asset, weight);
        }
        weights.retain(|_, weight| *weight != 0);
        Ok(Self { name, weights })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn weights(&self) -> impl Iterator<Item = (&A, i64)> {
        self.weights.iter().map(|(asset, weight)| (asset, *weight))
    }

    /// The invariant's value over every account.
    pub fn measure<'a>(&self, accounts: impl IntoIterator<Item = &'a Account<A>>) -> Measure
    where
        A: 'a,
    {
        self.measure_baskets(accounts.into_iter().map(Account::balances))
    }

    /// Whether an exchange preserves this invariant, from only the accounts it
    /// changed: a linear invariant is a location-independent sum, so it holds
    /// exactly when everything consumed measures the same as everything produced.
    pub(crate) fn conserves<AccountId>(&self, deltas: &[AccountDelta<AccountId, A>]) -> bool {
        self.measure_baskets(deltas.iter().map(AccountDelta::consumed))
            == self.measure_baskets(deltas.iter().map(AccountDelta::produced))
    }

    fn measure_baskets<'a>(&self, baskets: impl IntoIterator<Item = &'a Basket<A>>) -> Measure
    where
        A: 'a,
    {
        let (mut positive, mut negative) = (U384::ZERO, U384::ZERO);
        for (asset, amount) in baskets.into_iter().flat_map(Basket::iter) {
            let Some(&weight) = self.weights.get(asset) else {
                continue;
            };
            let term = U384::from(amount.value()) * U384::from(weight.unsigned_abs());
            if weight > 0 {
                positive += term;
            } else {
                negative += term;
            }
        }
        Measure::from_parts(positive, negative)
    }
}

impl<A: Serialize> Serialize for LinearInvariant<A> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("LinearInvariant", 2)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("weights", &self.weights.iter().collect::<Vec<_>>())?;
        state.end()
    }
}

#[derive(Deserialize, JsonSchema)]
struct InvariantData<A> {
    name: String,
    weights: Vec<(A, i64)>,
}

impl<'de, A: Deserialize<'de> + Eq + Hash> Deserialize<'de> for LinearInvariant<A> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = InvariantData::<A>::deserialize(deserializer)?;
        Self::from_serialized_weights(data.name, data.weights).map_err(|_| {
            serde::de::Error::custom("linear invariant contains a duplicate asset weight")
        })
    }
}

impl<A: JsonSchema> JsonSchema for LinearInvariant<A> {
    fn schema_name() -> Cow<'static, str> {
        format!("LinearInvariant_{}", A::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        generator.subschema_for::<InvariantData<A>>()
    }
}

/// A basket from literal `u64` quantities.
///
/// # Panics
///
/// If an asset appears twice.
pub fn basket<A: Eq + Hash, const LEN: usize>(entries: [(A, u64); LEN]) -> Basket<A> {
    entries
        .into_iter()
        .map(|(asset, value)| (asset, Quantity::new(value)))
        .collect()
}
#[cfg(test)]
mod conservation_conformance {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct DeltaCase {
        account: u64,
        consumed: Vec<(u64, String)>,
        produced: Vec<(u64, String)>,
        preserved: Vec<(u64, String)>,
    }

    #[derive(Debug, Deserialize)]
    struct Case {
        weights: Vec<(u64, String)>,
        deltas: Vec<DeltaCase>,
        conserved: Option<bool>,
    }

    #[derive(Deserialize)]
    struct Fixtures {
        conservation: Vec<Case>,
        measurements: Vec<Measurement>,
    }

    #[derive(Debug, Deserialize)]
    struct Measurement {
        weights: Vec<(u64, String)>,
        accounts: Vec<(u64, Vec<(u64, String)>)>,
        measured: Option<String>,
    }

    #[test]
    fn runtime_signed_conservation_matches_lean() {
        fn amounts(entries: &[(u64, String)]) -> Basket<u64> {
            Basket::try_from_entries(
                entries
                    .iter()
                    .map(|(asset, amount)| (*asset, amount.parse::<Quantity>().unwrap())),
            )
            .unwrap()
        }
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert_eq!(fixtures.conservation.len(), 464);
        let mut outcomes = [0; 2];
        for case in fixtures.conservation {
            let mut invariant = LinearInvariant::new("fixture");
            for (asset, coefficient) in &case.weights {
                invariant = invariant.weight(*asset, coefficient.parse().unwrap());
            }
            let deltas: Vec<_> = case
                .deltas
                .iter()
                .map(|d| {
                    AccountDelta::new(
                        d.account,
                        amounts(&d.consumed),
                        amounts(&d.produced),
                        amounts(&d.preserved),
                    )
                })
                .collect();
            // Measures are exact, so every case decides.
            let actual = invariant.conserves(&deltas);
            assert_eq!(Some(actual), case.conserved, "{case:?}");
            outcomes[usize::from(actual)] += 1;
        }
        assert!(outcomes.into_iter().all(|count| count > 0));
    }

    #[test]
    fn runtime_state_measurement_matches_lean() {
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert_eq!(fixtures.measurements.len(), 463);
        for case in fixtures.measurements {
            let mut invariant = LinearInvariant::new("fixture");
            for (asset, coefficient) in &case.weights {
                invariant = invariant.weight(*asset, coefficient.parse().unwrap());
            }
            let accounts: IndexMap<_, _> =
                case.accounts
                    .iter()
                    .map(|(id, entries)| {
                        let balances: Basket<u64> =
                            Basket::try_from_entries(entries.iter().map(|(asset, amount)| {
                                (*asset, amount.parse::<Quantity>().unwrap())
                            }))
                            .unwrap();
                        (*id, std::sync::Arc::new(Account::new(balances)))
                    })
                    .collect();
            let before = accounts.clone();
            let actual = invariant
                .measure(accounts.values().map(AsRef::as_ref))
                .to_string();
            assert_eq!(Some(actual), case.measured, "{case:?}");
            assert_eq!(accounts, before);
        }
    }
}

#[cfg(test)]
mod metadata_conformance {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
    struct Role(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
    struct Asset(u64);
    type Expression = QuantityExpression<Role, Asset>;
    type AmountData = (Role, (Asset, Expression));
    type ConditionData = (String, (Expression, (u8, Expression)));

    #[derive(Debug, Deserialize)]
    struct Step {
        kind: u8,
        role: Role,
        asset: Asset,
        expression: Expression,
        right: Expression,
        label: String,
        comparison: u8,
    }

    #[derive(Debug, Deserialize)]
    struct Snapshot {
        roles: Vec<Role>,
        distinct: Vec<(Role, Role)>,
        consumed: Vec<AmountData>,
        produced: Vec<AmountData>,
        preserved: Vec<AmountData>,
        conditions: Vec<ConditionData>,
        parameters: Vec<String>,
        assets: Vec<Asset>,
    }

    #[derive(Debug, Deserialize)]
    struct Case {
        steps: Vec<Step>,
        snapshots: Vec<Snapshot>,
    }

    #[derive(Deserialize)]
    struct Fixtures {
        metadata: Vec<Case>,
    }

    fn comparison(code: u8) -> QuantityComparison {
        match code {
            0 => QuantityComparison::Equal,
            1 => QuantityComparison::GreaterThanOrEqual,
            2 => QuantityComparison::LessThanOrEqual,
            _ => panic!("unknown comparison"),
        }
    }

    fn check(rule: &Rate<Role, Asset>, expected: &Snapshot) {
        assert_eq!(rule.roles().copied().collect::<Vec<_>>(), expected.roles);
        assert_eq!(
            rule.distinct_roles().copied().collect::<Vec<_>>(),
            expected.distinct
        );
        for (actual, expected) in [
            (rule.computed_consumed(), &expected.consumed),
            (rule.computed_produced(), &expected.produced),
            (rule.computed_preserved(), &expected.preserved),
        ] {
            assert_eq!(
                actual
                    .iter()
                    .map(|a| (a.role, (a.asset, a.quantity.clone())))
                    .collect::<Vec<_>>(),
                *expected
            );
        }
        assert_eq!(rule.conditions.len(), expected.conditions.len());
        for (actual, (label, (left, (code, right)))) in
            rule.conditions.iter().zip(&expected.conditions)
        {
            assert_eq!(&actual.name, label);
            assert_eq!(&actual.left, left);
            assert_eq!(actual.comparison, comparison(*code));
            assert_eq!(&actual.right, right);
        }
        assert_eq!(
            rule.parameter_names().into_iter().collect::<Vec<_>>(),
            expected.parameters
        );
        assert_eq!(
            rule.computed_asset_keys()
                .into_iter()
                .copied()
                .collect::<Vec<_>>(),
            expected.assets
        );
        assert!(rule.consume.is_empty() && rule.produce.is_empty() && rule.preserve.is_empty());
    }

    #[test]
    fn runtime_rate_metadata_matches_lean() {
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert_eq!(fixtures.metadata.len(), 85);
        for case in fixtures.metadata {
            assert_eq!(case.snapshots.len(), case.steps.len() + 1);
            for default in [false, true] {
                let mut rule = if default {
                    Rate::default()
                } else {
                    Rate::new()
                };
                check(&rule, &case.snapshots[0]);
                for (step, expected) in case.steps.iter().zip(&case.snapshots[1..]) {
                    rule = match step.kind {
                        0 => rule.consume_computed(step.role, step.asset, step.expression.clone()),
                        1 => rule.produce_computed(step.role, step.asset, step.expression.clone()),
                        2 => rule.preserve_computed(step.role, step.asset, step.expression.clone()),
                        3 => rule.condition(
                            &step.label,
                            step.expression.clone(),
                            comparison(step.comparison),
                            step.right.clone(),
                        ),
                        4 => rule.distinct(step.role, Role(step.asset.0)),
                        _ => panic!("unknown rate construction step"),
                    };
                    check(&rule, expected);
                }
            }
        }
    }
}
