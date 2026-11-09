use crate::{Basket, Quantity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::Hash;

/// A typed proposal keeps roles and account identities separate.
///
/// ```compile_fail,E0308
/// use axionomy::{Exchange, Quantity};
/// #[derive(PartialEq, Eq, PartialOrd, Ord)]
/// enum Role { Source }
/// enum AccountId { Source }
/// let proposal = Exchange::<u8, Role, AccountId>::new(0, Quantity::new(1));
/// let _ = proposal.bind(AccountId::Source, Role::Source);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>"
))]
pub struct Exchange<RateId, Role, AccountId> {
    rate: RateId,
    bindings: BTreeMap<Role, AccountId>,
    units: Quantity,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    parameters: BTreeMap<String, Quantity>,
}

impl<RateId, Role: Ord, AccountId> Exchange<RateId, Role, AccountId> {
    pub fn new(rate: RateId, units: Quantity) -> Self {
        Self {
            rate,
            bindings: BTreeMap::new(),
            units,
            parameters: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, role: Role, account: AccountId) -> Self {
        self.bindings.insert(role, account);
        self
    }

    /// Supplies a named exact quantity to a state-evaluated rate law.
    ///
    /// Parameters belong to the exchange proposal and are interpreted only by
    /// the bound rate. A common example is an AMM caller's `minimum_output`
    /// protection. Unknown and missing parameters are rejected by the economy.
    pub fn parameter(mut self, name: impl Into<String>, value: Quantity) -> Self {
        self.parameters.insert(name.into(), value);
        self
    }

    pub fn rate(&self) -> &RateId {
        &self.rate
    }

    pub fn bindings(&self) -> &BTreeMap<Role, AccountId> {
        &self.bindings
    }

    pub fn units(&self) -> Quantity {
        self.units
    }

    pub fn parameters(&self) -> &BTreeMap<String, Quantity> {
        &self.parameters
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "AccountId: Serialize, A: Serialize",
    deserialize = "AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash"
))]
pub struct AccountDelta<AccountId, A> {
    account: AccountId,
    consumed: Basket<A>,
    produced: Basket<A>,
    preserved: Basket<A>,
}

impl<AccountId, A> AccountDelta<AccountId, A> {
    pub(crate) fn new(
        account: AccountId,
        consumed: Basket<A>,
        produced: Basket<A>,
        preserved: Basket<A>,
    ) -> Self {
        Self {
            account,
            consumed,
            produced,
            preserved,
        }
    }

    pub fn account(&self) -> &AccountId {
        &self.account
    }

    pub fn consumed(&self) -> &Basket<A> {
        &self.consumed
    }

    pub fn produced(&self) -> &Basket<A> {
        &self.produced
    }

    pub fn preserved(&self) -> &Basket<A> {
        &self.preserved
    }
}

/// An applied exchange and the balance change it made to every account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize, A: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash"
))]
pub struct Receipt<RateId, Role, AccountId, A> {
    exchange: Exchange<RateId, Role, AccountId>,
    deltas: Vec<AccountDelta<AccountId, A>>,
    /// Accounts the exchange created, in role order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    opened: Vec<AccountId>,
    /// Accounts the exchange removed, in role order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    closed: Vec<AccountId>,
}

impl<RateId, Role, AccountId, A> Receipt<RateId, Role, AccountId, A> {
    pub(crate) fn new(
        exchange: Exchange<RateId, Role, AccountId>,
        deltas: Vec<AccountDelta<AccountId, A>>,
        opened: Vec<AccountId>,
        closed: Vec<AccountId>,
    ) -> Self {
        Self {
            exchange,
            deltas,
            opened,
            closed,
        }
    }

    /// Accounts the exchange created, in role order.
    pub fn opened(&self) -> &[AccountId] {
        &self.opened
    }

    /// Accounts the exchange removed, in role order.
    pub fn closed(&self) -> &[AccountId] {
        &self.closed
    }

    pub fn exchange(&self) -> &Exchange<RateId, Role, AccountId> {
        &self.exchange
    }

    pub fn deltas(&self) -> &[AccountDelta<AccountId, A>] {
        &self.deltas
    }
}

/// An ordered sequence of exchanges that replays from an initial economy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>"
))]
pub struct Trace<RateId, Role, AccountId> {
    exchanges: Vec<Exchange<RateId, Role, AccountId>>,
}

impl<RateId, Role, AccountId> Trace<RateId, Role, AccountId> {
    pub fn new() -> Self {
        Self {
            exchanges: Vec::new(),
        }
    }

    pub fn push(&mut self, exchange: Exchange<RateId, Role, AccountId>) {
        self.exchanges.push(exchange);
    }

    pub fn exchanges(&self) -> &[Exchange<RateId, Role, AccountId>] {
        &self.exchanges
    }

    pub fn into_exchanges(self) -> Vec<Exchange<RateId, Role, AccountId>> {
        self.exchanges
    }
}

impl<RateId, Role, AccountId> Default for Trace<RateId, Role, AccountId> {
    fn default() -> Self {
        Self::new()
    }
}

impl<RateId, Role, AccountId> Extend<Exchange<RateId, Role, AccountId>>
    for Trace<RateId, Role, AccountId>
{
    fn extend<T: IntoIterator<Item = Exchange<RateId, Role, AccountId>>>(&mut self, exchanges: T) {
        self.exchanges.extend(exchanges);
    }
}

impl<RateId, Role, AccountId> FromIterator<Exchange<RateId, Role, AccountId>>
    for Trace<RateId, Role, AccountId>
{
    fn from_iter<T: IntoIterator<Item = Exchange<RateId, Role, AccountId>>>(exchanges: T) -> Self {
        Self {
            exchanges: exchanges.into_iter().collect(),
        }
    }
}
