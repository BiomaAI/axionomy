use crate::{Basket, Quantity, QuantityScalar};
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
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize, N: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>, N: Deserialize<'de> + QuantityScalar"
))]
pub struct Exchange<RateId, Role, AccountId, N = u64> {
    rate: RateId,
    bindings: BTreeMap<Role, AccountId>,
    units: Quantity<N>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    parameters: BTreeMap<String, Quantity<N>>,
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
    pub fn parameter(mut self, name: impl Into<String>, value: Quantity<N>) -> Self {
        self.parameters.insert(name.into(), value);
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

    pub fn parameters(&self) -> &BTreeMap<String, Quantity<N>> {
        &self.parameters
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize, A: Serialize, N: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
pub struct Receipt<RateId, Role, AccountId, A, N = u64> {
    #[serde(rename = "exchange")]
    proposal: Exchange<RateId, Role, AccountId, N>,
    deltas: Vec<AccountDelta<AccountId, A, N>>,
}

impl<RateId, Role, AccountId, A, N> std::fmt::Debug for Receipt<RateId, Role, AccountId, A, N>
where
    RateId: std::fmt::Debug,
    Role: std::fmt::Debug,
    AccountId: std::fmt::Debug,
    A: std::fmt::Debug,
    N: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Receipt")
            .field("exchange", &self.proposal)
            .field("deltas", &self.deltas)
            .finish()
    }
}

impl<RateId, Role, AccountId, A, N> Receipt<RateId, Role, AccountId, A, N> {
    pub fn exchange(&self) -> &Exchange<RateId, Role, AccountId, N> {
        &self.proposal
    }

    pub fn deltas(&self) -> &[AccountDelta<AccountId, A, N>] {
        &self.deltas
    }

    pub(crate) fn new(
        proposal: Exchange<RateId, Role, AccountId, N>,
        deltas: Vec<AccountDelta<AccountId, A, N>>,
    ) -> Self {
        Self { proposal, deltas }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

    pub fn push(&mut self, proposal: Exchange<RateId, Role, AccountId, N>) {
        self.exchanges.push(proposal);
    }

    pub fn extend(
        &mut self,
        exchanges: impl IntoIterator<Item = Exchange<RateId, Role, AccountId, N>>,
    ) {
        for proposal in exchanges {
            self.exchanges.push(proposal);
        }
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
