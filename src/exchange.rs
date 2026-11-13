use crate::Ontology;
use crate::{Basket, ParameterPath, Quantity};
use schemars::JsonSchema;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(bound(
    deserialize = "O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>, O::Account: Deserialize<'de>",
))]
#[schemars(
    bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema",
    description = "An exchange: some units of a rate, applied atomically between the accounts bound to its roles, such as {\"rate\": \"transfer\", \"bindings\": {\"giver\": \"source\", \"receiver\": \"sink\"}, \"units\": \"2\"}."
)]
pub struct Exchange<O: Ontology> {
    /// The rate to apply.
    rate: O::Rate,
    /// The account each role of the rate binds, by role; every role the rate
    /// lists needs one. A role the rate opens binds the account the exchange
    /// creates, which must not exist yet; a role it closes binds an account
    /// the exchange leaves empty.
    bindings: BTreeMap<O::Role, O::Account>,
    /// How many times the rate's amounts apply; greater than zero.
    units: Quantity,
    /// The values of the parameters the rate's computed amounts and
    /// conditions read, by name.
    #[serde(default)]
    parameters: BTreeMap<ParameterPath, Quantity>,
}

/// JSON leaves out an exchange's parameters when it has none; formats that are
/// not read by people write every field.
impl<O: Ontology> Serialize for Exchange<O>
where
    O::Rate: Serialize,
    O::Role: Serialize,
    O::Account: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let omit_parameters = serializer.is_human_readable() && self.parameters.is_empty();
        let mut state = serializer.serialize_struct("Exchange", 4)?;
        state.serialize_field("rate", &self.rate)?;
        state.serialize_field("bindings", &self.bindings)?;
        state.serialize_field("units", &self.units)?;
        if omit_parameters {
            state.skip_field("parameters")?;
        } else {
            state.serialize_field("parameters", &self.parameters)?;
        }
        state.end()
    }
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
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(bound(
    deserialize = "O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>, O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(
    bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema, O::Asset: JsonSchema"
)]
pub struct Receipt<O: Ontology> {
    exchange: Exchange<O>,
    deltas: Vec<AccountDelta<O>>,
    /// Accounts the exchange created, in role order.
    #[serde(default)]
    opened: Vec<O::Account>,
    /// Accounts the exchange removed, in role order.
    #[serde(default)]
    closed: Vec<O::Account>,
}

/// JSON leaves out the accounts a receipt neither opened nor closed; formats
/// that are not read by people write every field.
impl<O: Ontology> Serialize for Receipt<O>
where
    O::Rate: Serialize,
    O::Role: Serialize,
    O::Account: Serialize,
    O::Asset: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let readable = serializer.is_human_readable();
        let mut state = serializer.serialize_struct("Receipt", 4)?;
        state.serialize_field("exchange", &self.exchange)?;
        state.serialize_field("deltas", &self.deltas)?;
        for (name, accounts) in [("opened", &self.opened), ("closed", &self.closed)] {
            if readable && accounts.is_empty() {
                state.skip_field(name)?;
            } else {
                state.serialize_field(name, accounts)?;
            }
        }
        state.end()
    }
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
