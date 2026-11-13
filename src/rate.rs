use crate::Ontology;
use crate::{
    Account, AccountDelta, Basket, ConditionPath, InvariantPath, ParameterPath, Quantity, Rounding,
};
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
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(
    bound = "O::Role: JsonSchema, O::Asset: JsonSchema",
    description = "An exact quantity computed from the economy before the exchange, as an object whose `kind` is `constant` (a `value`), `units` (the exchange's units), `parameter` (one of the exchange's parameters, by `name`), `balance` (a `role`'s balance of an `asset`), `add`, `subtract`, `multiply`, `minimum` or `maximum` (of a `left` and a `right` expression), or `mul_div`: `left` × `right` ÷ `denominator` from a full-width product, the only division, with an inexact quotient rounded as its `rounding` says, `floor` or `ceil`. Round what a participant pays up and what it receives down, so rounding dust stays with the counterparty."
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuantityExpression<O: Ontology> {
    Constant {
        value: Quantity,
    },
    Units,
    Parameter {
        name: ParameterPath,
    },
    Balance {
        role: O::Role,
        asset: O::Asset,
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

/// An expression is written tagged by `kind` where people read the format, as
/// JSON, and tagged by variant otherwise, as the store's binary records are. A
/// child expression writes itself, so each form holds at every depth.
impl<O: Ontology> Serialize for QuantityExpression<O>
where
    O::Role: Serialize,
    O::Asset: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            expression_forms::TaggedRef::from(self).serialize(serializer)
        } else {
            expression_forms::CompactRef::from(self).serialize(serializer)
        }
    }
}

impl<'de, O: Ontology> Deserialize<'de> for QuantityExpression<O>
where
    O::Role: Deserialize<'de>,
    O::Asset: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            expression_forms::Tagged::deserialize(deserializer).map(Self::from)
        } else {
            expression_forms::Compact::deserialize(deserializer).map(Self::from)
        }
    }
}

/// The two written forms of [`QuantityExpression`], each as a borrowed form to
/// write and an owned form to read.
mod expression_forms {
    use super::{Ontology, ParameterPath, Quantity, QuantityExpression, Rounding};
    use serde::{Deserialize, Serialize};

    macro_rules! form {
        (
            $by_ref:ident, $owned:ident, [$($attribute:meta),*],
            $($variant:ident $({ $($field:ident: $ty:ty),* $(,)? })?),* $(,)?
        ) => {
            #[derive(Serialize)]
            #[serde(bound(serialize = "O::Role: Serialize, O::Asset: Serialize"), $($attribute),*)]
            pub(super) enum $by_ref<'a, O: Ontology> {
                $($variant $({ $($field: &'a $ty),* })?),*
            }

            impl<'a, O: Ontology> From<&'a QuantityExpression<O>> for $by_ref<'a, O> {
                fn from(expression: &'a QuantityExpression<O>) -> Self {
                    match expression {
                        $(QuantityExpression::$variant $({ $($field),* })? => {
                            Self::$variant $({ $($field),* })?
                        })*
                    }
                }
            }

            #[derive(Deserialize)]
            #[serde(
                bound(deserialize = "O::Role: Deserialize<'de>, O::Asset: Deserialize<'de>"),
                $($attribute),*
            )]
            pub(super) enum $owned<O: Ontology> {
                $($variant $({ $($field: $ty),* })?),*
            }

            impl<O: Ontology> From<$owned<O>> for QuantityExpression<O> {
                fn from(form: $owned<O>) -> Self {
                    match form {
                        $($owned::$variant $({ $($field),* })? => {
                            Self::$variant $({ $($field),* })?
                        })*
                    }
                }
            }
        };
    }

    macro_rules! forms {
        ($($variants:tt)*) => {
            form!(TaggedRef, Tagged, [tag = "kind", rename_all = "snake_case"], $($variants)*);
            form!(CompactRef, Compact, [rename_all = "snake_case"], $($variants)*);
        };
    }

    forms! {
        Constant { value: Quantity },
        Units,
        Parameter { name: ParameterPath },
        Balance { role: O::Role, asset: O::Asset },
        Add { left: Box<QuantityExpression<O>>, right: Box<QuantityExpression<O>> },
        Subtract { left: Box<QuantityExpression<O>>, right: Box<QuantityExpression<O>> },
        Multiply { left: Box<QuantityExpression<O>>, right: Box<QuantityExpression<O>> },
        Minimum { left: Box<QuantityExpression<O>>, right: Box<QuantityExpression<O>> },
        Maximum { left: Box<QuantityExpression<O>>, right: Box<QuantityExpression<O>> },
        MulDiv {
            left: Box<QuantityExpression<O>>,
            right: Box<QuantityExpression<O>>,
            denominator: Box<QuantityExpression<O>>,
            rounding: Rounding,
        },
    }
}

impl<O: Ontology> QuantityExpression<O> {
    pub const fn constant(value: u64) -> Self {
        Self::Constant {
            value: Quantity::new(value),
        }
    }

    pub const fn units() -> Self {
        Self::Units
    }

    pub fn parameter(name: ParameterPath) -> Self {
        Self::Parameter { name }
    }

    pub const fn balance(role: O::Role, asset: O::Asset) -> Self {
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

    fn roles(&self) -> Vec<&O::Role> {
        let mut roles = Vec::new();
        self.walk(&mut |expression| {
            if let Self::Balance { role, .. } = expression {
                roles.push(role);
            }
        });
        roles
    }

    fn assets(&self) -> Vec<&O::Asset> {
        let mut assets = Vec::new();
        self.walk(&mut |expression| {
            if let Self::Balance { asset, .. } = expression {
                assets.push(asset);
            }
        });
        assets
    }

    fn parameters(&self) -> Vec<&ParameterPath> {
        let mut parameters = Vec::new();
        self.walk(&mut |expression| {
            if let Self::Parameter { name } = expression {
                parameters.push(name);
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
        impl<O: Ontology> std::ops::$trait for QuantityExpression<O> {
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
#[serde(bound(
    serialize = "O::Role: Serialize, O::Asset: Serialize",
    deserialize = "O::Role: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Role: JsonSchema, O::Asset: JsonSchema")]
pub struct ComputedAmount<O: Ontology> {
    role: O::Role,
    asset: O::Asset,
    quantity: QuantityExpression<O>,
}

impl<O: Ontology> ComputedAmount<O> {
    pub const fn role(&self) -> &O::Role {
        &self.role
    }

    pub const fn asset(&self) -> &O::Asset {
        &self.asset
    }

    pub const fn quantity(&self) -> &QuantityExpression<O> {
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
    serialize = "O::Role: Serialize, O::Asset: Serialize",
    deserialize = "O::Role: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Role: JsonSchema, O::Asset: JsonSchema")]
pub struct RateCondition<O: Ontology> {
    name: ConditionPath,
    left: QuantityExpression<O>,
    comparison: QuantityComparison,
    right: QuantityExpression<O>,
}

impl<O: Ontology> RateCondition<O> {
    pub fn name(&self) -> &ConditionPath {
        &self.name
    }

    pub const fn left(&self) -> &QuantityExpression<O> {
        &self.left
    }

    pub const fn comparison(&self) -> QuantityComparison {
        self.comparison
    }

    pub const fn right(&self) -> &QuantityExpression<O> {
        &self.right
    }
}

#[derive(Debug, Clone)]
pub struct Rate<O: Ontology> {
    consume: BTreeMap<O::Role, Basket<O::Asset>>,
    produce: BTreeMap<O::Role, Basket<O::Asset>>,
    preserve: BTreeMap<O::Role, Basket<O::Asset>>,
    roles: BTreeSet<O::Role>,
    distinct: BTreeSet<(O::Role, O::Role)>,
    computed_consume: Vec<ComputedAmount<O>>,
    computed_produce: Vec<ComputedAmount<O>>,
    computed_preserve: Vec<ComputedAmount<O>>,
    conditions: Vec<RateCondition<O>>,
    opens: BTreeSet<O::Role>,
    closes: BTreeSet<O::Role>,
}

impl<O: Ontology> Rate<O> {
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
            opens: BTreeSet::new(),
            closes: BTreeSet::new(),
        }
    }

    /// Creates the account bound to `role`, empty, before the exchange's
    /// effects. The proposal names it; it must not exist, and no other role
    /// may bind it.
    pub fn opens(mut self, role: O::Role) -> Self {
        self.roles.insert(role.clone());
        self.opens.insert(role);
        self
    }

    /// Removes the account bound to `role` after the exchange's effects, which
    /// must leave it empty, so closing never destroys anything.
    pub fn closes(mut self, role: O::Role) -> Self {
        self.roles.insert(role.clone());
        self.closes.insert(role);
        self
    }

    /// Consumes `basket` per unit from the account bound to `role`.
    ///
    /// # Panics
    ///
    /// If repeated baskets for one role add up beyond `Quantity::MAX`.
    pub fn consume(self, role: O::Role, basket: Basket<O::Asset>) -> Self {
        self.with_static(Direction::Consume, role, basket)
            .unwrap_or_else(|_| panic!("rate consume basket quantity overflow"))
    }

    /// Produces `basket` per unit into the account bound to `role`.
    ///
    /// # Panics
    ///
    /// If repeated baskets for one role add up beyond `Quantity::MAX`.
    pub fn produce(self, role: O::Role, basket: Basket<O::Asset>) -> Self {
        self.with_static(Direction::Produce, role, basket)
            .unwrap_or_else(|_| panic!("rate produce basket quantity overflow"))
    }

    /// Requires `basket` in the account bound to `role` without consuming it.
    ///
    /// # Panics
    ///
    /// If repeated baskets for one role add up beyond `Quantity::MAX`.
    pub fn preserve(self, role: O::Role, basket: Basket<O::Asset>) -> Self {
        self.with_static(Direction::Preserve, role, basket)
            .unwrap_or_else(|_| panic!("rate preserve basket quantity overflow"))
    }

    fn with_static(
        mut self,
        direction: Direction,
        role: O::Role,
        basket: Basket<O::Asset>,
    ) -> Result<Self, RateError<O>> {
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

    pub fn distinct(mut self, left: O::Role, right: O::Role) -> Self {
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
        role: O::Role,
        asset: O::Asset,
        quantity: QuantityExpression<O>,
    ) -> Self {
        self.with_computed(Direction::Consume, role, asset, quantity)
    }

    pub fn produce_computed(
        self,
        role: O::Role,
        asset: O::Asset,
        quantity: QuantityExpression<O>,
    ) -> Self {
        self.with_computed(Direction::Produce, role, asset, quantity)
    }

    pub fn preserve_computed(
        self,
        role: O::Role,
        asset: O::Asset,
        quantity: QuantityExpression<O>,
    ) -> Self {
        self.with_computed(Direction::Preserve, role, asset, quantity)
    }

    fn with_computed(
        mut self,
        direction: Direction,
        role: O::Role,
        asset: O::Asset,
        quantity: QuantityExpression<O>,
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
        name: ConditionPath,
        left: QuantityExpression<O>,
        comparison: QuantityComparison,
        right: QuantityExpression<O>,
    ) -> Self {
        self.roles.extend(left.roles().into_iter().cloned());
        self.roles.extend(right.roles().into_iter().cloned());
        self.conditions.push(RateCondition {
            name,
            left,
            comparison,
            right,
        });
        self
    }

    pub fn roles(&self) -> impl Iterator<Item = &O::Role> {
        self.roles.iter()
    }

    pub(crate) fn has_role(&self, role: &O::Role) -> bool {
        self.roles.contains(role)
    }

    pub fn consumed(&self, role: &O::Role) -> Option<&Basket<O::Asset>> {
        self.consume.get(role)
    }

    pub fn produced(&self, role: &O::Role) -> Option<&Basket<O::Asset>> {
        self.produce.get(role)
    }

    pub fn preserved(&self, role: &O::Role) -> Option<&Basket<O::Asset>> {
        self.preserve.get(role)
    }

    pub fn distinct_roles(&self) -> impl Iterator<Item = &(O::Role, O::Role)> {
        self.distinct.iter()
    }

    pub fn computed_consumed(&self) -> &[ComputedAmount<O>] {
        &self.computed_consume
    }

    pub fn computed_produced(&self) -> &[ComputedAmount<O>] {
        &self.computed_produce
    }

    pub fn computed_preserved(&self) -> &[ComputedAmount<O>] {
        &self.computed_preserve
    }

    pub fn conditions(&self) -> &[RateCondition<O>] {
        &self.conditions
    }

    /// Roles whose bound account the exchange opens, in role order.
    pub fn opened_roles(&self) -> impl Iterator<Item = &O::Role> {
        self.opens.iter()
    }

    /// Roles whose bound account the exchange closes, in role order.
    pub fn closed_roles(&self) -> impl Iterator<Item = &O::Role> {
        self.closes.iter()
    }

    fn computed(&self) -> impl Iterator<Item = &ComputedAmount<O>> {
        self.computed_consume
            .iter()
            .chain(&self.computed_produce)
            .chain(&self.computed_preserve)
    }

    fn expressions(&self) -> impl Iterator<Item = &QuantityExpression<O>> {
        self.computed().map(ComputedAmount::quantity).chain(
            self.conditions
                .iter()
                .flat_map(|condition| [&condition.left, &condition.right]),
        )
    }

    /// Names of every parameter the rate's laws and conditions read.
    pub fn parameter_names(&self) -> BTreeSet<&ParameterPath> {
        self.expressions()
            .flat_map(QuantityExpression::parameters)
            .collect()
    }

    /// Assets named by computed effects and the balances their laws read, in
    /// declaration order and with repeats.
    pub fn computed_asset_keys(&self) -> Vec<&O::Asset> {
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
    pub(crate) fn asset_keys(&self) -> BTreeSet<&O::Asset> {
        [&self.consume, &self.produce, &self.preserve]
            .into_iter()
            .flat_map(BTreeMap::values)
            .flat_map(|basket| basket.iter().map(|(asset, _)| asset))
            .chain(self.computed_asset_keys())
            .collect()
    }
}

impl<O: Ontology> Default for Rate<O> {
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

impl<O: Ontology> Serialize for Rate<O>
where
    O::Role: Serialize,
    O::Asset: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let readable = serializer.is_human_readable();
        let mut state = serializer.serialize_struct("Rate", 10)?;
        state.serialize_field("consume", &self.consume.iter().collect::<Vec<_>>())?;
        state.serialize_field("produce", &self.produce.iter().collect::<Vec<_>>())?;
        state.serialize_field("preserve", &self.preserve.iter().collect::<Vec<_>>())?;
        state.serialize_field("distinct", &self.distinct.iter().collect::<Vec<_>>())?;
        state.serialize_field("computed_consume", &self.computed_consume)?;
        state.serialize_field("computed_produce", &self.computed_produce)?;
        state.serialize_field("computed_preserve", &self.computed_preserve)?;
        state.serialize_field("conditions", &self.conditions)?;
        // JSON leaves out roles a rate neither opens nor closes; formats
        // that are not read by people write every field.
        if readable && self.opens.is_empty() {
            state.skip_field("opens")?;
        } else {
            state.serialize_field("opens", &self.opens)?;
        }
        if readable && self.closes.is_empty() {
            state.skip_field("closes")?;
        } else {
            state.serialize_field("closes", &self.closes)?;
        }
        state.end()
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(bound(deserialize = "O::Role: Deserialize<'de>, O::Asset: Deserialize<'de>",))]
#[schemars(bound = "O::Role: JsonSchema, O::Asset: JsonSchema")]
struct RateData<O: Ontology> {
    consume: Vec<(O::Role, Basket<O::Asset>)>,
    produce: Vec<(O::Role, Basket<O::Asset>)>,
    preserve: Vec<(O::Role, Basket<O::Asset>)>,
    distinct: Vec<(O::Role, O::Role)>,
    #[serde(default)]
    computed_consume: Vec<ComputedAmount<O>>,
    #[serde(default)]
    computed_produce: Vec<ComputedAmount<O>>,
    #[serde(default)]
    computed_preserve: Vec<ComputedAmount<O>>,
    #[serde(default)]
    conditions: Vec<RateCondition<O>>,
    #[serde(default)]
    opens: Vec<O::Role>,
    #[serde(default)]
    closes: Vec<O::Role>,
}

impl<'de, O: Ontology> Deserialize<'de> for Rate<O>
where
    O::Role: Deserialize<'de>,
    O::Asset: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = RateData::<O>::deserialize(deserializer)?;
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
        for role in data.opens {
            rate = rate.opens(role);
        }
        for role in data.closes {
            rate = rate.closes(role);
        }
        Ok(rate)
    }
}

impl<O: Ontology> JsonSchema for Rate<O>
where
    O::Role: JsonSchema,
    O::Asset: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!(
            "Rate_{}_{}",
            <O::Role as JsonSchema>::schema_name(),
            <O::Asset as JsonSchema>::schema_name()
        )
        .into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        generator.subschema_for::<RateData<O>>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
enum RateError<O: Ontology> {
    #[error("rate basket quantity overflow")]
    BasketOverflow { role: O::Role, asset: O::Asset },
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
    name: InvariantPath,
    weights: IndexMap<A, i64>,
}

impl<A: Eq + Hash> LinearInvariant<A> {
    pub fn new(name: InvariantPath) -> Self {
        Self {
            name,
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
    fn from_serialized_weights(name: InvariantPath, entries: Vec<(A, i64)>) -> Result<Self, A> {
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

    pub fn name(&self) -> &InvariantPath {
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
    pub(crate) fn conserves<O: Ontology<Asset = A>>(&self, deltas: &[AccountDelta<O>]) -> bool {
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
    name: InvariantPath,
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Numbers {}

    impl Ontology for Numbers {
        const NAME: &'static str = "numbers";
        type Account = u64;
        type Asset = u64;
        type Rate = ();
        type Role = ();
    }

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
        assert!(!fixtures.conservation.is_empty());
        let mut outcomes = [0; 2];
        for case in fixtures.conservation {
            let mut invariant = LinearInvariant::new(crate::path!("fixture"));
            for (asset, coefficient) in &case.weights {
                invariant = invariant.weight(*asset, coefficient.parse().unwrap());
            }
            let deltas: Vec<AccountDelta<Numbers>> = case
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
        assert!(!fixtures.measurements.is_empty());
        for case in fixtures.measurements {
            let mut invariant = LinearInvariant::new(crate::path!("fixture"));
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    struct Role(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    struct Asset(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Test {}

    impl Ontology for Test {
        const NAME: &'static str = "test";
        type Account = ();
        type Asset = Asset;
        type Rate = ();
        type Role = Role;
    }

    type Expression = QuantityExpression<Test>;
    type AmountData = (Role, (Asset, Expression));
    type ConditionData = (ConditionPath, (Expression, (u8, Expression)));

    #[derive(Debug, Deserialize)]
    struct Step {
        kind: u8,
        role: Role,
        asset: Asset,
        expression: Expression,
        right: Expression,
        label: ConditionPath,
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
        parameters: Vec<ParameterPath>,
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

    fn check(rule: &Rate<Test>, expected: &Snapshot) {
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
            expected.parameters.iter().collect::<Vec<_>>()
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
        assert!(!fixtures.metadata.is_empty());
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
                            step.label.clone(),
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
