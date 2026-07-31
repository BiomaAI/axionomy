use crate::{
    Account, AccountDelta, AccountError, Basket, Exchange, LinearInvariant, Quantity, Rate,
    Receipt, Trace,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct Goal<AccountId, A> {
    required: BTreeMap<AccountId, Basket<A>>,
}

impl<AccountId, A> Goal<AccountId, A>
where
    AccountId: Ord,
{
    pub fn new() -> Self {
        Self {
            required: BTreeMap::new(),
        }
    }

    pub fn require(mut self, account: AccountId, assets: Basket<A>) -> Self {
        self.required.insert(account, assets);
        self
    }

    pub fn requirements(&self) -> &BTreeMap<AccountId, Basket<A>> {
        &self.required
    }
}

impl<AccountId, A> Default for Goal<AccountId, A>
where
    AccountId: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Economy<AccountId, A, RateId, Role> {
    accounts: HashMap<AccountId, Account<A>>,
    rates: HashMap<RateId, Rate<Role, A>>,
    invariants: Vec<LinearInvariant<A>>,
}

pub type ApplyResult<AccountId, A, RateId, Role> =
    Result<Receipt<RateId, Role, AccountId, A>, ApplyError<RateId, Role, AccountId, A>>;

pub type ReplayResult<AccountId, A, RateId, Role> =
    Result<Vec<Receipt<RateId, Role, AccountId, A>>, ApplyError<RateId, Role, AccountId, A>>;

pub type SimulationResult<AccountId, A, RateId, Role> = Result<
    (
        Economy<AccountId, A, RateId, Role>,
        Receipt<RateId, Role, AccountId, A>,
    ),
    ApplyError<RateId, Role, AccountId, A>,
>;

#[derive(Debug)]
pub struct EconomyBuilder<AccountId, A, RateId, Role> {
    accounts: HashMap<AccountId, Account<A>>,
    rates: HashMap<RateId, Rate<Role, A>>,
    invariants: Vec<LinearInvariant<A>>,
}

impl<AccountId, A, RateId, Role> EconomyBuilder<AccountId, A, RateId, Role>
where
    AccountId: Eq + Hash,
    RateId: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            rates: HashMap::new(),
            invariants: Vec::new(),
        }
    }

    pub fn account(mut self, id: AccountId, account: Account<A>) -> Self {
        self.accounts.insert(id, account);
        self
    }

    pub fn rate(mut self, id: RateId, rate: Rate<Role, A>) -> Self {
        self.rates.insert(id, rate);
        self
    }

    pub fn invariant(mut self, invariant: LinearInvariant<A>) -> Self {
        self.invariants.push(invariant);
        self
    }

    pub fn build(self) -> Economy<AccountId, A, RateId, Role> {
        Economy {
            accounts: self.accounts,
            rates: self.rates,
            invariants: self.invariants,
        }
    }
}

impl<AccountId, A, RateId, Role> Default for EconomyBuilder<AccountId, A, RateId, Role>
where
    AccountId: Eq + Hash,
    RateId: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<AccountId, A, RateId, Role> Economy<AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
{
    pub fn account(&self, id: &AccountId) -> Option<&Account<A>> {
        self.accounts.get(id)
    }

    pub fn balance(&self, account: &AccountId, asset: &A) -> Quantity {
        self.accounts
            .get(account)
            .map_or(Quantity::ZERO, |account| account.balance(asset))
    }

    pub fn accounts(&self) -> impl Iterator<Item = (&AccountId, &Account<A>)> {
        self.accounts.iter()
    }

    pub fn rate(&self, id: &RateId) -> Option<&Rate<Role, A>> {
        self.rates.get(id)
    }

    pub fn rate_ids(&self) -> impl Iterator<Item = &RateId> {
        self.rates.keys()
    }

    pub fn matches(&self, goal: &Goal<AccountId, A>) -> bool {
        goal.requirements().iter().all(|(account_id, required)| {
            self.accounts
                .get(account_id)
                .is_some_and(|account| account.balances().contains(required))
        })
    }

    pub fn view(
        &self,
        visible_accounts: impl IntoIterator<Item = AccountId>,
    ) -> EconomicView<'_, AccountId, A, RateId, Role> {
        EconomicView {
            economy: self,
            visible: visible_accounts.into_iter().collect(),
        }
    }

    pub fn state_key(&self) -> Vec<(AccountId, A, Quantity)> {
        let mut key = Vec::new();
        for (account_id, account) in &self.accounts {
            for (asset, quantity) in account.balances().iter() {
                key.push((account_id.clone(), asset.clone(), quantity));
            }
        }
        key.sort();
        key
    }

    /// Creates an isolated branch for search, simulation, or speculative work.
    pub fn fork(&self) -> Self {
        self.clone()
    }

    /// Applies one exchange to an isolated branch, leaving this economy intact.
    pub fn simulate(
        &self,
        exchange: Exchange<RateId, Role, AccountId>,
    ) -> SimulationResult<AccountId, A, RateId, Role> {
        let mut fork = self.fork();
        let receipt = fork.apply(exchange)?;
        Ok((fork, receipt))
    }

    pub fn can_apply(
        &self,
        exchange: &Exchange<RateId, Role, AccountId>,
    ) -> Result<(), ApplyError<RateId, Role, AccountId, A>> {
        self.prepare(exchange).map(|_| ())
    }

    pub fn applicable(
        &self,
        candidates: impl IntoIterator<Item = Exchange<RateId, Role, AccountId>>,
    ) -> Vec<Exchange<RateId, Role, AccountId>> {
        candidates
            .into_iter()
            .filter(|exchange| self.can_apply(exchange).is_ok())
            .collect()
    }

    pub fn apply(
        &mut self,
        exchange: Exchange<RateId, Role, AccountId>,
    ) -> ApplyResult<AccountId, A, RateId, Role> {
        let prepared = self.prepare(&exchange)?;
        self.accounts = prepared.accounts;
        Ok(Receipt::new(exchange, prepared.deltas))
    }

    pub fn replay(
        &mut self,
        trace: &Trace<RateId, Role, AccountId>,
    ) -> ReplayResult<AccountId, A, RateId, Role> {
        trace
            .exchanges()
            .iter()
            .cloned()
            .map(|exchange| self.apply(exchange))
            .collect()
    }

    /// Validates a complete trace on an isolated branch and returns its final state.
    pub fn replayed(
        &self,
        trace: &Trace<RateId, Role, AccountId>,
    ) -> Result<Self, ApplyError<RateId, Role, AccountId, A>> {
        let mut fork = self.fork();
        fork.replay(trace)?;
        Ok(fork)
    }

    fn prepare(
        &self,
        exchange: &Exchange<RateId, Role, AccountId>,
    ) -> Result<Prepared<AccountId, A>, ApplyError<RateId, Role, AccountId, A>> {
        if exchange.units().is_zero() {
            return Err(ApplyError::ZeroUnits);
        }

        let rate = self
            .rates
            .get(exchange.rate())
            .ok_or_else(|| ApplyError::MissingRate {
                rate: exchange.rate().clone(),
            })?;

        for role in rate.roles() {
            if !exchange.bindings().contains_key(role) {
                return Err(ApplyError::MissingBinding { role: role.clone() });
            }
        }
        for role in exchange.bindings().keys() {
            if !rate.roles().any(|known| known == role) {
                return Err(ApplyError::UnknownBinding { role: role.clone() });
            }
        }
        for (left, right) in rate.distinct_roles() {
            if exchange.bindings().get(left) == exchange.bindings().get(right) {
                return Err(ApplyError::RolesMustDiffer {
                    left: left.clone(),
                    right: right.clone(),
                });
            }
        }

        let mut effects: BTreeMap<AccountId, Effect<A>> = BTreeMap::new();
        for role in rate.roles() {
            let account_id = exchange
                .bindings()
                .get(role)
                .expect("all role bindings were checked")
                .clone();
            if !self.accounts.contains_key(&account_id) {
                return Err(ApplyError::MissingAccount {
                    account: account_id,
                });
            }
            let effect = effects.entry(account_id).or_default();
            if let Some(consume) = rate.consumed(role) {
                merge_scaled(&mut effect.consume, consume, exchange.units()).map_err(|asset| {
                    ApplyError::RateOverflow {
                        rate: exchange.rate().clone(),
                        asset,
                    }
                })?;
            }
            if let Some(produce) = rate.produced(role) {
                merge_scaled(&mut effect.produce, produce, exchange.units()).map_err(|asset| {
                    ApplyError::RateOverflow {
                        rate: exchange.rate().clone(),
                        asset,
                    }
                })?;
            }
            if let Some(preserve) = rate.preserved(role) {
                effect.preserve.checked_add(preserve).map_err(|asset| {
                    ApplyError::RateOverflow {
                        rate: exchange.rate().clone(),
                        asset,
                    }
                })?;
            }
        }

        let mut accounts = self.accounts.clone();
        let mut deltas = Vec::with_capacity(effects.len());
        for (account_id, effect) in effects {
            let account = accounts
                .get_mut(&account_id)
                .expect("bound accounts were checked");
            let mut required = effect.consume.clone();
            required
                .checked_add(&effect.preserve)
                .map_err(|asset| ApplyError::RateOverflow {
                    rate: exchange.rate().clone(),
                    asset,
                })?;
            let shortfall = account.balances().shortfall(&required);
            if !shortfall.is_empty() {
                return Err(ApplyError::InsufficientBalance {
                    account: account_id,
                    shortfall,
                });
            }
            account
                .withdraw(&effect.consume)
                .map_err(|error| map_account_error(account_id.clone(), error))?;
            account
                .deposit(&effect.produce)
                .map_err(|error| map_account_error(account_id.clone(), error))?;
            deltas.push(AccountDelta::new(
                account_id,
                effect.consume,
                effect.produce,
                effect.preserve,
            ));
        }

        for invariant in &self.invariants {
            let before =
                invariant
                    .measure(&self.accounts)
                    .ok_or_else(|| ApplyError::InvariantOverflow {
                        invariant: invariant.name().to_owned(),
                    })?;
            let after =
                invariant
                    .measure(&accounts)
                    .ok_or_else(|| ApplyError::InvariantOverflow {
                        invariant: invariant.name().to_owned(),
                    })?;
            if before != after {
                return Err(ApplyError::InvariantViolation {
                    invariant: invariant.name().to_owned(),
                    before,
                    after,
                });
            }
        }

        Ok(Prepared { accounts, deltas })
    }
}

pub struct EconomicView<'a, AccountId, A, RateId, Role> {
    economy: &'a Economy<AccountId, A, RateId, Role>,
    visible: BTreeSet<AccountId>,
}

impl<AccountId, A, RateId, Role> EconomicView<'_, AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
{
    pub fn account(&self, id: &AccountId) -> Option<&Account<A>> {
        self.visible
            .contains(id)
            .then(|| self.economy.account(id))
            .flatten()
    }

    pub fn balance(&self, account: &AccountId, asset: &A) -> Option<Quantity> {
        self.account(account).map(|visible| visible.balance(asset))
    }
}

#[derive(Debug, Clone)]
pub enum ApplyError<RateId, Role, AccountId, A> {
    MissingRate {
        rate: RateId,
    },
    MissingBinding {
        role: Role,
    },
    UnknownBinding {
        role: Role,
    },
    RolesMustDiffer {
        left: Role,
        right: Role,
    },
    MissingAccount {
        account: AccountId,
    },
    ZeroUnits,
    RateOverflow {
        rate: RateId,
        asset: A,
    },
    InsufficientBalance {
        account: AccountId,
        shortfall: Basket<A>,
    },
    BalanceOverflow {
        account: AccountId,
        asset: A,
    },
    InvariantOverflow {
        invariant: String,
    },
    InvariantViolation {
        invariant: String,
        before: i128,
        after: i128,
    },
}

impl<RateId, Role, AccountId, A> fmt::Display for ApplyError<RateId, Role, AccountId, A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRate { .. } => formatter.write_str("rate does not exist"),
            Self::MissingBinding { .. } => {
                formatter.write_str("exchange is missing a role binding")
            }
            Self::UnknownBinding { .. } => formatter.write_str("exchange contains an unknown role"),
            Self::RolesMustDiffer { .. } => {
                formatter.write_str("rate roles must bind to different accounts")
            }
            Self::MissingAccount { .. } => formatter.write_str("bound account does not exist"),
            Self::ZeroUnits => formatter.write_str("exchange units must be greater than zero"),
            Self::RateOverflow { .. } => formatter.write_str("scaled rate overflow"),
            Self::InsufficientBalance { .. } => formatter.write_str("insufficient balance"),
            Self::BalanceOverflow { .. } => formatter.write_str("account balance overflow"),
            Self::InvariantOverflow { .. } => formatter.write_str("invariant arithmetic overflow"),
            Self::InvariantViolation { .. } => formatter.write_str("declared invariant violation"),
        }
    }
}

impl<RateId, Role, AccountId, A> Error for ApplyError<RateId, Role, AccountId, A>
where
    RateId: fmt::Debug,
    Role: fmt::Debug,
    AccountId: fmt::Debug,
    A: fmt::Debug,
{
}

#[derive(Debug, Clone)]
struct Effect<A> {
    consume: Basket<A>,
    produce: Basket<A>,
    preserve: Basket<A>,
}

impl<A> Default for Effect<A> {
    fn default() -> Self {
        Self {
            consume: Basket::new(),
            produce: Basket::new(),
            preserve: Basket::new(),
        }
    }
}

struct Prepared<AccountId, A> {
    accounts: HashMap<AccountId, Account<A>>,
    deltas: Vec<AccountDelta<AccountId, A>>,
}

fn merge_scaled<A>(target: &mut Basket<A>, source: &Basket<A>, units: Quantity) -> Result<(), A>
where
    A: Clone + Eq + Hash,
{
    let scaled = source.checked_scale(units)?;
    target.checked_add(&scaled)
}

fn map_account_error<RateId, Role, AccountId, A>(
    account: AccountId,
    error: AccountError<A>,
) -> ApplyError<RateId, Role, AccountId, A> {
    match error {
        AccountError::InsufficientBalance { shortfall } => {
            ApplyError::InsufficientBalance { account, shortfall }
        }
        AccountError::Overflow { asset } => ApplyError::BalanceOverflow { account, asset },
    }
}
