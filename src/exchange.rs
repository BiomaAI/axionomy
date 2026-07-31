use crate::{Basket, Quantity};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange<RateId, Role, AccountId> {
    rate: RateId,
    bindings: BTreeMap<Role, AccountId>,
    units: Quantity,
}

impl<RateId, Role, AccountId> Exchange<RateId, Role, AccountId>
where
    Role: Ord,
{
    pub fn new(rate: RateId, units: Quantity) -> Self {
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

    pub fn units(&self) -> Quantity {
        self.units
    }
}

#[derive(Debug, Clone)]
pub struct AccountDelta<AccountId, A> {
    account: AccountId,
    consumed: Basket<A>,
    produced: Basket<A>,
    preserved: Basket<A>,
}

impl<AccountId, A> AccountDelta<AccountId, A> {
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
}

#[derive(Debug, Clone)]
pub struct Receipt<RateId, Role, AccountId, A> {
    exchange: Exchange<RateId, Role, AccountId>,
    deltas: Vec<AccountDelta<AccountId, A>>,
}

impl<RateId, Role, AccountId, A> Receipt<RateId, Role, AccountId, A> {
    pub fn exchange(&self) -> &Exchange<RateId, Role, AccountId> {
        &self.exchange
    }

    pub fn deltas(&self) -> &[AccountDelta<AccountId, A>] {
        &self.deltas
    }

    pub(crate) fn new(
        exchange: Exchange<RateId, Role, AccountId>,
        deltas: Vec<AccountDelta<AccountId, A>>,
    ) -> Self {
        Self { exchange, deltas }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn extend(
        &mut self,
        exchanges: impl IntoIterator<Item = Exchange<RateId, Role, AccountId>>,
    ) {
        self.exchanges.extend(exchanges);
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
