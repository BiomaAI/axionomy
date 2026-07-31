use crate::{Basket, Quantity, QuantityScalar};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize, N: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
))]
pub struct Exchange<RateId, Role, AccountId, N = u64> {
    rate: RateId,
    bindings: BTreeMap<Role, AccountId>,
    units: Quantity<N>,
}

impl<RateId, Role, AccountId, N> Exchange<RateId, Role, AccountId, N>
where
    Role: Ord,
{
    pub fn new(rate: RateId, units: Quantity<N>) -> Self {
        Self {
            rate,
            bindings: BTreeMap::new(),
            units,
        }
    }

    pub fn bind(mut self, role: Role, account: AccountId) -> Self {
        self.bindings.insert(role, account);
        self
    }

    pub fn rate(&self) -> &RateId {
        &self.rate
    }

    pub fn bindings(&self) -> &BTreeMap<Role, AccountId> {
        &self.bindings
    }

    pub fn units(&self) -> &Quantity<N> {
        &self.units
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "AccountId: Serialize, A: Serialize, N: Serialize",
    deserialize = "AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
pub struct AccountDelta<AccountId, A, N = u64> {
    account: AccountId,
    consumed: Basket<A, N>,
    produced: Basket<A, N>,
    preserved: Basket<A, N>,
}

impl<AccountId, A, N> AccountDelta<AccountId, A, N> {
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    pub fn consumed(&self) -> &Basket<A, N> {
        &self.consumed
    }

    pub fn produced(&self) -> &Basket<A, N> {
        &self.produced
    }

    pub fn preserved(&self) -> &Basket<A, N> {
        &self.preserved
    }

    pub(crate) fn new(
        account: AccountId,
        consumed: Basket<A, N>,
        produced: Basket<A, N>,
        preserved: Basket<A, N>,
    ) -> Self {
        Self {
            account,
            consumed,
            produced,
            preserved,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize, A: Serialize, N: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
pub struct Receipt<RateId, Role, AccountId, A, N = u64> {
    exchange: Exchange<RateId, Role, AccountId, N>,
    deltas: Vec<AccountDelta<AccountId, A, N>>,
}

impl<RateId, Role, AccountId, A, N> Receipt<RateId, Role, AccountId, A, N> {
    pub fn exchange(&self) -> &Exchange<RateId, Role, AccountId, N> {
        &self.exchange
    }

    pub fn deltas(&self) -> &[AccountDelta<AccountId, A, N>] {
        &self.deltas
    }

    pub(crate) fn new(
        exchange: Exchange<RateId, Role, AccountId, N>,
        deltas: Vec<AccountDelta<AccountId, A, N>>,
    ) -> Self {
        Self { exchange, deltas }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize, N: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
))]
pub struct Trace<RateId, Role, AccountId, N = u64> {
    exchanges: Vec<Exchange<RateId, Role, AccountId, N>>,
}

impl<RateId, Role, AccountId, N> Trace<RateId, Role, AccountId, N> {
    pub fn new() -> Self {
        Self {
            exchanges: Vec::new(),
        }
    }

    pub fn push(&mut self, exchange: Exchange<RateId, Role, AccountId, N>) {
        self.exchanges.push(exchange);
    }

    pub fn extend(
        &mut self,
        exchanges: impl IntoIterator<Item = Exchange<RateId, Role, AccountId, N>>,
    ) {
        self.exchanges.extend(exchanges);
    }

    pub fn exchanges(&self) -> &[Exchange<RateId, Role, AccountId, N>] {
        &self.exchanges
    }

    pub fn into_exchanges(self) -> Vec<Exchange<RateId, Role, AccountId, N>> {
        self.exchanges
    }
}

impl<RateId, Role, AccountId, N> Default for Trace<RateId, Role, AccountId, N> {
    fn default() -> Self {
        Self::new()
    }
}
