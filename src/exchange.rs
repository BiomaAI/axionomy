use crate::Ontology;
use crate::{Basket, ParameterPath, Quantity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A typed proposal keeps roles and account identities separate.
///
/// ```compile_fail,E0308
/// use axionomy::{AccountPath, Quantity, RolePath, paths::Exchange};
/// let source: AccountPath = "source".parse().unwrap();
/// let payer: RolePath = "payer".parse().unwrap();
/// let proposal = Exchange::new("pay".parse().unwrap(), Quantity::new(1));
/// let _ = proposal.bind(source, payer);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Rate: Serialize, O::Role: Serialize, O::Account: Serialize",
    deserialize = "O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>, O::Account: Deserialize<'de>",
))]
#[schemars(bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema")]
pub struct Exchange<O: Ontology> {
    rate: O::Rate,
    bindings: BTreeMap<O::Role, O::Account>,
    units: Quantity,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    parameters: BTreeMap<ParameterPath, Quantity>,
}

impl<O: Ontology> Exchange<O> {
    pub fn new(rate: O::Rate, units: Quantity) -> Self {
        Self {
            rate,
            bindings: BTreeMap::new(),
            units,
            parameters: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, role: O::Role, account: O::Account) -> Self {
        self.bindings.insert(role, account);
        self
    }

    /// Supplies a named exact quantity to a state-evaluated rate law.
    ///
    /// Parameters belong to the exchange proposal and are interpreted only by
    /// the bound rate. A common example is an AMM caller's `minimum-output`
    /// protection. Unknown and missing parameters are rejected by the economy.
    pub fn parameter(mut self, name: ParameterPath, value: Quantity) -> Self {
        self.parameters.insert(name, value);
        self
    }

    pub fn rate(&self) -> &O::Rate {
        &self.rate
    }

    pub fn bindings(&self) -> &BTreeMap<O::Role, O::Account> {
        &self.bindings
    }

    pub fn units(&self) -> Quantity {
        self.units
    }

    pub fn parameters(&self) -> &BTreeMap<ParameterPath, Quantity> {
        &self.parameters
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Account: Serialize, O::Asset: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Account: JsonSchema, O::Asset: JsonSchema")]
pub struct AccountDelta<O: Ontology> {
    account: O::Account,
    consumed: Basket<O::Asset>,
    produced: Basket<O::Asset>,
    preserved: Basket<O::Asset>,
}

impl<O: Ontology> AccountDelta<O> {
    pub(crate) fn new(
        account: O::Account,
        consumed: Basket<O::Asset>,
        produced: Basket<O::Asset>,
        preserved: Basket<O::Asset>,
    ) -> Self {
        Self {
            account,
            consumed,
            produced,
            preserved,
        }
    }

    pub fn account(&self) -> &O::Account {
        &self.account
    }

    pub fn consumed(&self) -> &Basket<O::Asset> {
        &self.consumed
    }

    pub fn produced(&self) -> &Basket<O::Asset> {
        &self.produced
    }

    pub fn preserved(&self) -> &Basket<O::Asset> {
        &self.preserved
    }
}

/// An applied exchange and the balance change it made to every account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Rate: Serialize, O::Role: Serialize, O::Account: Serialize, O::Asset: Serialize",
    deserialize = "O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>, O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(
    bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema, O::Asset: JsonSchema"
)]
pub struct Receipt<O: Ontology> {
    exchange: Exchange<O>,
    deltas: Vec<AccountDelta<O>>,
    /// Accounts the exchange created, in role order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    opened: Vec<O::Account>,
    /// Accounts the exchange removed, in role order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    closed: Vec<O::Account>,
}

impl<O: Ontology> Receipt<O> {
    pub(crate) fn new(
        exchange: Exchange<O>,
        deltas: Vec<AccountDelta<O>>,
        opened: Vec<O::Account>,
        closed: Vec<O::Account>,
    ) -> Self {
        Self {
            exchange,
            deltas,
            opened,
            closed,
        }
    }

    /// Accounts the exchange created, in role order.
    pub fn opened(&self) -> &[O::Account] {
        &self.opened
    }

    /// Accounts the exchange removed, in role order.
    pub fn closed(&self) -> &[O::Account] {
        &self.closed
    }

    pub fn exchange(&self) -> &Exchange<O> {
        &self.exchange
    }

    pub fn deltas(&self) -> &[AccountDelta<O>] {
        &self.deltas
    }
}

/// An ordered sequence of exchanges that replays from an initial economy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Rate: Serialize, O::Role: Serialize, O::Account: Serialize",
    deserialize = "O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>, O::Account: Deserialize<'de>",
))]
#[schemars(bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema")]
pub struct Trace<O: Ontology> {
    exchanges: Vec<Exchange<O>>,
}

impl<O: Ontology> Trace<O> {
    pub fn new() -> Self {
        Self {
            exchanges: Vec::new(),
        }
    }

    pub fn push(&mut self, exchange: Exchange<O>) {
        self.exchanges.push(exchange);
    }

    pub fn exchanges(&self) -> &[Exchange<O>] {
        &self.exchanges
    }

    pub fn into_exchanges(self) -> Vec<Exchange<O>> {
        self.exchanges
    }
}

impl<O: Ontology> Default for Trace<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Ontology> Extend<Exchange<O>> for Trace<O> {
    fn extend<T: IntoIterator<Item = Exchange<O>>>(&mut self, exchanges: T) {
        self.exchanges.extend(exchanges);
    }
}

impl<O: Ontology> FromIterator<Exchange<O>> for Trace<O> {
    fn from_iter<T: IntoIterator<Item = Exchange<O>>>(exchanges: T) -> Self {
        Self {
            exchanges: exchanges.into_iter().collect(),
        }
    }
}
