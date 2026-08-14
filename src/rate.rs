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

/// An exact, serializable quantity expression evaluated against the economy's
/// pre-exchange snapshot.
///
/// This keeps state-dependent exchange laws inside Axionomy authority without
/// embedding domain callbacks in the kernel. Expressions intentionally expose
/// only exact integer arithmetic; division is floor division.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    bound(
        serialize = "Role: Serialize, A: Serialize, N: Serialize",
        deserialize = "Role: Deserialize<'de>, A: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
    )
)]
pub enum QuantityExpression<Role, A, N = u64> {
    Constant {
        value: Quantity<N>,
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
    DivideFloor {
        numerator: Box<Self>,
        denominator: Box<Self>,
    },
    Minimum {
        left: Box<Self>,
        right: Box<Self>,
    },
    Maximum {
        left: Box<Self>,
        right: Box<Self>,
    },
}

impl<Role, A> QuantityExpression<Role, A> {
    pub const fn constant(value: u64) -> Self {
        Self::Constant {
            value: Quantity::new(value),
        }
    }
}

impl<Role, A, N> std::ops::Add for QuantityExpression<Role, A, N> {
    type Output = Self;

    fn add(self, right: Self) -> Self::Output {
        Self::Add {
            left: Box::new(self),
            right: Box::new(right),
        }
    }
}

impl<Role, A, N> QuantityExpression<Role, A, N> {
    pub const fn units() -> Self {
        Self::Units
    }

    pub fn parameter(name: impl Into<String>) -> Self {
        Self::Parameter { name: name.into() }
    }

    pub const fn balance(role: Role, asset: A) -> Self {
        Self::Balance { role, asset }
    }

    pub fn plus(left: Self, right: Self) -> Self {
        Self::Add {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn subtract(left: Self, right: Self) -> Self {
        Self::Subtract {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn multiply(left: Self, right: Self) -> Self {
        Self::Multiply {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn divide_floor(numerator: Self, denominator: Self) -> Self {
        Self::DivideFloor {
            numerator: Box::new(numerator),
            denominator: Box::new(denominator),
        }
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

    fn collect_roles<'a>(&'a self, roles: &mut BTreeSet<&'a Role>)
    where
        Role: Ord,
    {
        match self {
            Self::Balance { role, .. } => {
                roles.insert(role);
            }
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right } => {
                left.collect_roles(roles);
                right.collect_roles(roles);
            }
            Self::DivideFloor {
                numerator,
                denominator,
            } => {
                numerator.collect_roles(roles);
                denominator.collect_roles(roles);
            }
            Self::Constant { .. } | Self::Units | Self::Parameter { .. } => {}
        }
    }

    fn collect_assets<'a>(&'a self, assets: &mut Vec<&'a A>) {
        match self {
            Self::Balance { asset, .. } => assets.push(asset),
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right } => {
                left.collect_assets(assets);
                right.collect_assets(assets);
            }
            Self::DivideFloor {
                numerator,
                denominator,
            } => {
                numerator.collect_assets(assets);
                denominator.collect_assets(assets);
            }
            Self::Constant { .. } | Self::Units | Self::Parameter { .. } => {}
        }
    }

    fn collect_parameters<'a>(&'a self, parameters: &mut BTreeSet<&'a str>) {
        match self {
            Self::Parameter { name } => {
                parameters.insert(name);
            }
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right } => {
                left.collect_parameters(parameters);
                right.collect_parameters(parameters);
            }
            Self::DivideFloor {
                numerator,
                denominator,
            } => {
                numerator.collect_parameters(parameters);
                denominator.collect_parameters(parameters);
            }
            Self::Constant { .. } | Self::Units | Self::Balance { .. } => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "Role: Serialize, A: Serialize, N: Serialize",
    deserialize = "Role: Deserialize<'de>, A: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
))]
pub struct ComputedAmount<Role, A, N = u64> {
    role: Role,
    asset: A,
    quantity: QuantityExpression<Role, A, N>,
}

impl<Role, A, N> ComputedAmount<Role, A, N> {
    pub const fn role(&self) -> &Role {
        &self.role
    }

    pub const fn asset(&self) -> &A {
        &self.asset
    }

    pub const fn quantity(&self) -> &QuantityExpression<Role, A, N> {
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
#[serde(bound(
    serialize = "Role: Serialize, A: Serialize, N: Serialize",
    deserialize = "Role: Deserialize<'de>, A: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
))]
pub struct RateCondition<Role, A, N = u64> {
    name: String,
    left: QuantityExpression<Role, A, N>,
    comparison: QuantityComparison,
    right: QuantityExpression<Role, A, N>,
}

impl<Role, A, N> RateCondition<Role, A, N> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn left(&self) -> &QuantityExpression<Role, A, N> {
        &self.left
    }

    pub const fn comparison(&self) -> QuantityComparison {
        self.comparison
    }

    pub const fn right(&self) -> &QuantityExpression<Role, A, N> {
        &self.right
    }
}

#[derive(Debug, Clone)]
pub struct Rate<Role, A, N = u64> {
    consume: BTreeMap<Role, Basket<A, N>>,
    produce: BTreeMap<Role, Basket<A, N>>,
    preserve: BTreeMap<Role, Basket<A, N>>,
    roles: BTreeSet<Role>,
    distinct: BTreeSet<(Role, Role)>,
    computed_consume: Vec<ComputedAmount<Role, A, N>>,
    computed_produce: Vec<ComputedAmount<Role, A, N>>,
    computed_preserve: Vec<ComputedAmount<Role, A, N>>,
    conditions: Vec<RateCondition<Role, A, N>>,
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
            computed_consume: Vec::new(),
            computed_produce: Vec::new(),
            computed_preserve: Vec::new(),
            conditions: Vec::new(),
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

    pub fn consume_computed(
        mut self,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A, N>,
    ) -> Self {
        self.register_expression_roles(&role, &quantity);
        self.computed_consume.push(ComputedAmount {
            role,
            asset,
            quantity,
        });
        self
    }

    pub fn produce_computed(
        mut self,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A, N>,
    ) -> Self {
        self.register_expression_roles(&role, &quantity);
        self.computed_produce.push(ComputedAmount {
            role,
            asset,
            quantity,
        });
        self
    }

    pub fn preserve_computed(
        mut self,
        role: Role,
        asset: A,
        quantity: QuantityExpression<Role, A, N>,
    ) -> Self {
        self.register_expression_roles(&role, &quantity);
        self.computed_preserve.push(ComputedAmount {
            role,
            asset,
            quantity,
        });
        self
    }

    pub fn condition(
        mut self,
        name: impl Into<String>,
        left: QuantityExpression<Role, A, N>,
        comparison: QuantityComparison,
        right: QuantityExpression<Role, A, N>,
    ) -> Self {
        let mut expression_roles = BTreeSet::new();
        left.collect_roles(&mut expression_roles);
        right.collect_roles(&mut expression_roles);
        self.roles.extend(expression_roles.into_iter().cloned());
        self.conditions.push(RateCondition {
            name: name.into(),
            left,
            comparison,
            right,
        });
        self
    }

    fn register_expression_roles(
        &mut self,
        target: &Role,
        expression: &QuantityExpression<Role, A, N>,
    ) {
        self.roles.insert(target.clone());
        let mut expression_roles = BTreeSet::new();
        expression.collect_roles(&mut expression_roles);
        self.roles.extend(expression_roles.into_iter().cloned());
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

    pub fn computed_consumed(&self) -> &[ComputedAmount<Role, A, N>] {
        &self.computed_consume
    }

    pub fn computed_produced(&self) -> &[ComputedAmount<Role, A, N>] {
        &self.computed_produce
    }

    pub fn computed_preserved(&self) -> &[ComputedAmount<Role, A, N>] {
        &self.computed_preserve
    }

    pub fn conditions(&self) -> &[RateCondition<Role, A, N>] {
        &self.conditions
    }

    pub fn parameter_names(&self) -> BTreeSet<&str> {
        let mut parameters = BTreeSet::new();
        for amount in self
            .computed_consume
            .iter()
            .chain(&self.computed_produce)
            .chain(&self.computed_preserve)
        {
            amount.quantity.collect_parameters(&mut parameters);
        }
        for condition in &self.conditions {
            condition.left.collect_parameters(&mut parameters);
            condition.right.collect_parameters(&mut parameters);
        }
        parameters
    }

    pub fn computed_asset_keys(&self) -> Vec<&A> {
        let mut assets = Vec::new();
        for amount in self
            .computed_consume
            .iter()
            .chain(&self.computed_produce)
            .chain(&self.computed_preserve)
        {
            assets.push(&amount.asset);
            amount.quantity.collect_assets(&mut assets);
        }
        for condition in &self.conditions {
            condition.left.collect_assets(&mut assets);
            condition.right.collect_assets(&mut assets);
        }
        assets
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

#[derive(Deserialize)]
#[serde(bound(
    deserialize = "Role: Deserialize<'de> + Clone + Ord, A: Deserialize<'de> + Clone + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
struct RateData<Role, A, N> {
    consume: Vec<(Role, Basket<A, N>)>,
    produce: Vec<(Role, Basket<A, N>)>,
    preserve: Vec<(Role, Basket<A, N>)>,
    distinct: Vec<(Role, Role)>,
    #[serde(default)]
    computed_consume: Vec<ComputedAmount<Role, A, N>>,
    #[serde(default)]
    computed_produce: Vec<ComputedAmount<Role, A, N>>,
    #[serde(default)]
    computed_preserve: Vec<ComputedAmount<Role, A, N>>,
    #[serde(default)]
    conditions: Vec<RateCondition<Role, A, N>>,
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
        for amount in data.computed_consume {
            rate = rate.consume_computed(amount.role, amount.asset, amount.quantity);
        }
        for amount in data.computed_produce {
            rate = rate.produce_computed(amount.role, amount.asset, amount.quantity);
        }
        for amount in data.computed_preserve {
            rate = rate.preserve_computed(amount.role, amount.asset, amount.quantity);
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
            computed_consume: Vec<ComputedAmount<Role, A, N>>,
            computed_produce: Vec<ComputedAmount<Role, A, N>>,
            computed_preserve: Vec<ComputedAmount<Role, A, N>>,
            conditions: Vec<RateCondition<Role, A, N>>,
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
