use crate::{
    Account, AccountDelta, AccountError, Basket, Exchange, LinearInvariant, Quantity,
    QuantityScalar, Rate, Receipt, Trace,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Goal<AccountId, A, N = u64> {
    required: BTreeMap<AccountId, Basket<A, N>>,
}

impl<AccountId, A, N> Goal<AccountId, A, N>
where
    AccountId: Ord,
{
    pub fn new() -> Self {
        Self {
            required: BTreeMap::new(),
        }
    }

    pub fn require(mut self, account: AccountId, assets: Basket<A, N>) -> Self {
        self.required.insert(account, assets);
        self
    }

    pub fn requirements(&self) -> &BTreeMap<AccountId, Basket<A, N>> {
        &self.required
    }
}

impl<AccountId, A, N> Default for Goal<AccountId, A, N>
where
    AccountId: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Economy<AccountId, A, RateId, Role, N = u64> {
    accounts: HashMap<AccountId, Arc<Account<A, N>>>,
    rates: Arc<HashMap<RateId, Rate<Role, A, N>>>,
    invariants: Arc<Vec<LinearInvariant<A>>>,
}

/// Compact logical-state identity for in-process search and transpositions.
///
/// The fingerprint is deterministic for a fixed ontology implementation and
/// executable, but it is not a durable serialization or collision-proof key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateFingerprint(u64);

impl StateFingerprint {
    pub const fn get(self) -> u64 {
        self.0
    }
}

pub type ApplyResult<AccountId, A, RateId, Role, N = u64> =
    Result<Receipt<RateId, Role, AccountId, A, N>, ApplyError<RateId, Role, AccountId, A, N>>;

pub type ReplayResult<AccountId, A, RateId, Role, N = u64> =
    Result<Vec<Receipt<RateId, Role, AccountId, A, N>>, ApplyError<RateId, Role, AccountId, A, N>>;

pub type SimulationResult<AccountId, A, RateId, Role, N = u64> = Result<
    (
        Economy<AccountId, A, RateId, Role, N>,
        Receipt<RateId, Role, AccountId, A, N>,
    ),
    ApplyError<RateId, Role, AccountId, A, N>,
>;

/// The high-level result of assessing an exchange without mutating the economy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentStatus {
    Applicable,
    Infeasible,
    Invalid,
}

/// One account's complete rate-derived requirements and effects.
#[derive(Debug, Clone)]
pub struct AccountAssessment<AccountId, A, N = u64> {
    account: AccountId,
    available: Basket<A, N>,
    required: Basket<A, N>,
    consumed: Basket<A, N>,
    produced: Basket<A, N>,
    preserved: Basket<A, N>,
}

impl<AccountId, A, N> AccountAssessment<AccountId, A, N> {
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    /// Balances relevant to this exchange's requirements.
    pub fn available(&self) -> &Basket<A, N> {
        &self.available
    }

    pub fn required(&self) -> &Basket<A, N> {
        &self.required
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

    fn new(
        account: AccountId,
        available: Basket<A, N>,
        required: Basket<A, N>,
        consumed: Basket<A, N>,
        produced: Basket<A, N>,
        preserved: Basket<A, N>,
    ) -> Self {
        Self {
            account,
            available,
            required,
            consumed,
            produced,
            preserved,
        }
    }
}

/// The exact assets one account lacks for a well-formed exchange.
#[derive(Debug, Clone)]
pub struct AccountShortfall<AccountId, A, N = u64> {
    account: AccountId,
    missing: Basket<A, N>,
}

impl<AccountId, A, N> AccountShortfall<AccountId, A, N> {
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    pub fn missing(&self) -> &Basket<A, N> {
        &self.missing
    }

    fn new(account: AccountId, missing: Basket<A, N>) -> Self {
        Self { account, missing }
    }
}

/// A non-mutating explanation of one proposed exchange.
#[must_use]
#[derive(Debug, Clone)]
pub enum ExchangeAssessment<AccountId, A, RateId, Role, N = u64>
where
    N: QuantityScalar,
{
    /// Every check succeeded; projected deltas match a subsequent receipt if
    /// the economy remains unchanged.
    Applicable {
        accounts: Vec<AccountAssessment<AccountId, A, N>>,
        projected_deltas: Vec<AccountDelta<AccountId, A, N>>,
    },
    /// The proposal is structurally valid but one or more accounts lack assets.
    Infeasible {
        accounts: Vec<AccountAssessment<AccountId, A, N>>,
        shortfalls: Vec<AccountShortfall<AccountId, A, N>>,
    },
    /// The proposal is malformed, overflows, or violates an invariant.
    Invalid {
        issues: Vec<ApplyError<RateId, Role, AccountId, A, N>>,
    },
}

impl<AccountId, A, RateId, Role, N> ExchangeAssessment<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    pub fn status(&self) -> AssessmentStatus {
        match self {
            Self::Applicable { .. } => AssessmentStatus::Applicable,
            Self::Infeasible { .. } => AssessmentStatus::Infeasible,
            Self::Invalid { .. } => AssessmentStatus::Invalid,
        }
    }

    pub fn is_applicable(&self) -> bool {
        matches!(self, Self::Applicable { .. })
    }

    pub fn accounts(&self) -> &[AccountAssessment<AccountId, A, N>] {
        match self {
            Self::Applicable { accounts, .. } | Self::Infeasible { accounts, .. } => accounts,
            Self::Invalid { .. } => &[],
        }
    }

    pub fn account(&self, account: &AccountId) -> Option<&AccountAssessment<AccountId, A, N>>
    where
        AccountId: PartialEq,
    {
        self.accounts()
            .iter()
            .find(|assessment| assessment.account() == account)
    }

    pub fn shortfalls(&self) -> &[AccountShortfall<AccountId, A, N>] {
        match self {
            Self::Infeasible { shortfalls, .. } => shortfalls,
            Self::Applicable { .. } | Self::Invalid { .. } => &[],
        }
    }

    pub fn shortfall(&self, account: &AccountId) -> Option<&Basket<A, N>>
    where
        AccountId: PartialEq,
    {
        self.shortfalls()
            .iter()
            .find(|shortfall| shortfall.account() == account)
            .map(AccountShortfall::missing)
    }

    pub fn projected_deltas(&self) -> Option<&[AccountDelta<AccountId, A, N>]> {
        match self {
            Self::Applicable {
                projected_deltas, ..
            } => Some(projected_deltas),
            Self::Infeasible { .. } | Self::Invalid { .. } => None,
        }
    }

    pub fn issues(&self) -> &[ApplyError<RateId, Role, AccountId, A, N>] {
        match self {
            Self::Invalid { issues } => issues,
            Self::Applicable { .. } | Self::Infeasible { .. } => &[],
        }
    }
}

#[derive(Debug)]
pub struct EconomyBuilder<AccountId, A, RateId, Role, N = u64> {
    accounts: HashMap<AccountId, Account<A, N>>,
    rates: HashMap<RateId, Rate<Role, A, N>>,
    invariants: Vec<LinearInvariant<A>>,
}

impl<AccountId, A, RateId, Role, N> EconomyBuilder<AccountId, A, RateId, Role, N>
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

    pub fn account(mut self, id: AccountId, account: Account<A, N>) -> Self {
        self.accounts.insert(id, account);
        self
    }

    pub fn rate(mut self, id: RateId, rate: Rate<Role, A, N>) -> Self {
        self.rates.insert(id, rate);
        self
    }

    pub fn invariant(mut self, invariant: LinearInvariant<A>) -> Self {
        self.invariants.push(invariant);
        self
    }

    pub fn build(self) -> Economy<AccountId, A, RateId, Role, N> {
        Economy {
            accounts: self
                .accounts
                .into_iter()
                .map(|(id, account)| (id, Arc::new(account)))
                .collect(),
            rates: Arc::new(self.rates),
            invariants: Arc::new(self.invariants),
        }
    }
}

impl<AccountId, A, RateId, Role, N> Default for EconomyBuilder<AccountId, A, RateId, Role, N>
where
    AccountId: Eq + Hash,
    RateId: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<AccountId, A, RateId, Role, N> Economy<AccountId, A, RateId, Role, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
{
    pub fn account(&self, id: &AccountId) -> Option<&Account<A, N>> {
        self.accounts.get(id).map(Arc::as_ref)
    }

    pub fn balance(&self, account: &AccountId, asset: &A) -> Quantity<N> {
        self.accounts
            .get(account)
            .map_or_else(Quantity::default, |account| account.balance(asset))
    }

    pub fn accounts(&self) -> impl Iterator<Item = (&AccountId, &Account<A, N>)> {
        self.accounts
            .iter()
            .map(|(id, account)| (id, account.as_ref()))
    }

    pub fn rate(&self, id: &RateId) -> Option<&Rate<Role, A, N>> {
        self.rates.get(id)
    }

    pub fn rate_ids(&self) -> impl Iterator<Item = &RateId> {
        self.rates.keys()
    }

    pub fn matches(&self, goal: &Goal<AccountId, A, N>) -> bool {
        goal.requirements().iter().all(|(account_id, required)| {
            self.accounts
                .get(account_id)
                .is_some_and(|account| account.balances().contains(required))
        })
    }

    pub fn view(
        &self,
        visible_accounts: impl IntoIterator<Item = AccountId>,
    ) -> EconomicView<'_, AccountId, A, RateId, Role, N> {
        EconomicView {
            economy: self,
            visible: visible_accounts.into_iter().collect(),
        }
    }

    pub fn state_key(&self) -> Vec<(AccountId, A, Quantity<N>)> {
        let mut key = Vec::new();
        for (account_id, account) in &self.accounts {
            for (asset, quantity) in account.balances().iter() {
                key.push((account_id.clone(), asset.clone(), quantity.clone()));
            }
        }
        key.sort();
        key
    }

    /// Returns a compact fingerprint for caches scoped to this economy model.
    pub fn state_fingerprint(&self) -> StateFingerprint {
        let mut hasher = FingerprintHasher::new();
        self.state_key().hash(&mut hasher);
        StateFingerprint(hasher.finish())
    }

    /// Creates an isolated branch for search, simulation, or speculative work.
    pub fn fork(&self) -> Self {
        self.clone()
    }

    /// Applies one exchange to an isolated branch, leaving this economy intact.
    pub fn simulate(
        &self,
        exchange: Exchange<RateId, Role, AccountId, N>,
    ) -> SimulationResult<AccountId, A, RateId, Role, N> {
        let mut fork = self.fork();
        let receipt = fork.apply(exchange)?;
        Ok((fork, receipt))
    }

    /// Explains one exchange without mutating the economy.
    pub fn assess(
        &self,
        exchange: &Exchange<RateId, Role, AccountId, N>,
    ) -> ExchangeAssessment<AccountId, A, RateId, Role, N> {
        self.analyze(exchange).assessment
    }

    /// Returns whether one exchange is applicable to the current snapshot.
    #[must_use]
    pub fn is_applicable(&self, exchange: &Exchange<RateId, Role, AccountId, N>) -> bool {
        self.assess(exchange).is_applicable()
    }

    /// Retains only candidates applicable to the current snapshot.
    #[must_use]
    pub fn applicable(
        &self,
        candidates: impl IntoIterator<Item = Exchange<RateId, Role, AccountId, N>>,
    ) -> Vec<Exchange<RateId, Role, AccountId, N>> {
        candidates
            .into_iter()
            .filter(|exchange| self.is_applicable(exchange))
            .collect()
    }

    pub fn apply(
        &mut self,
        exchange: Exchange<RateId, Role, AccountId, N>,
    ) -> ApplyResult<AccountId, A, RateId, Role, N> {
        let Analysis {
            assessment,
            prepared_accounts,
        } = self.analyze(&exchange);

        match assessment {
            ExchangeAssessment::Applicable {
                projected_deltas, ..
            } => {
                self.accounts =
                    prepared_accounts.expect("applicable assessments have prepared accounts");
                Ok(Receipt::new(exchange, projected_deltas))
            }
            ExchangeAssessment::Infeasible { shortfalls, .. } => {
                Err(ApplyError::Infeasible { shortfalls })
            }
            ExchangeAssessment::Invalid { issues } => Err(issues
                .into_iter()
                .next()
                .expect("invalid assessments contain at least one issue")),
        }
    }

    pub fn replay(
        &mut self,
        trace: &Trace<RateId, Role, AccountId, N>,
    ) -> ReplayResult<AccountId, A, RateId, Role, N> {
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
        trace: &Trace<RateId, Role, AccountId, N>,
    ) -> Result<Self, ApplyError<RateId, Role, AccountId, A, N>> {
        let mut fork = self.fork();
        fork.replay(trace)?;
        Ok(fork)
    }

    fn analyze(
        &self,
        exchange: &Exchange<RateId, Role, AccountId, N>,
    ) -> Analysis<AccountId, A, RateId, Role, N> {
        let mut issues = Vec::new();
        if exchange.units().is_zero() {
            issues.push(ApplyError::ZeroUnits);
        }

        let Some(rate) = self.rates.get(exchange.rate()) else {
            issues.push(ApplyError::MissingRate {
                rate: exchange.rate().clone(),
            });
            return invalid_analysis(issues);
        };

        for role in rate.roles() {
            if !exchange.bindings().contains_key(role) {
                issues.push(ApplyError::MissingBinding { role: role.clone() });
            }
        }
        for role in exchange.bindings().keys() {
            if !rate.roles().any(|known| known == role) {
                issues.push(ApplyError::UnknownBinding { role: role.clone() });
            }
        }
        for (left, right) in rate.distinct_roles() {
            if let (Some(left_account), Some(right_account)) = (
                exchange.bindings().get(left),
                exchange.bindings().get(right),
            ) {
                if left_account == right_account {
                    issues.push(ApplyError::RolesMustDiffer {
                        left: left.clone(),
                        right: right.clone(),
                    });
                }
            }
        }

        let missing_accounts = rate
            .roles()
            .filter_map(|role| exchange.bindings().get(role))
            .filter(|account| !self.accounts.contains_key(*account))
            .cloned()
            .collect::<BTreeSet<_>>();
        for account in missing_accounts {
            issues.push(ApplyError::MissingAccount { account });
        }

        if !issues.is_empty() {
            return invalid_analysis(issues);
        }

        let mut effects: BTreeMap<AccountId, Effect<A, N>> = BTreeMap::new();
        for role in rate.roles() {
            let account_id = exchange
                .bindings()
                .get(role)
                .expect("all role bindings were checked")
                .clone();
            let effect = effects.entry(account_id).or_default();
            if let Some(consume) = rate.consumed(role) {
                if let Err(asset) = merge_scaled(&mut effect.consume, consume, exchange.units()) {
                    return invalid_analysis(vec![ApplyError::RateOverflow {
                        rate: exchange.rate().clone(),
                        asset,
                    }]);
                }
            }
            if let Some(produce) = rate.produced(role) {
                if let Err(asset) = merge_scaled(&mut effect.produce, produce, exchange.units()) {
                    return invalid_analysis(vec![ApplyError::RateOverflow {
                        rate: exchange.rate().clone(),
                        asset,
                    }]);
                }
            }
            if let Some(preserve) = rate.preserved(role) {
                if let Err(asset) = effect.preserve.checked_add(preserve) {
                    return invalid_analysis(vec![ApplyError::RateOverflow {
                        rate: exchange.rate().clone(),
                        asset,
                    }]);
                }
            }
        }

        let mut account_assessments = Vec::with_capacity(effects.len());
        let mut shortfalls = Vec::new();
        for (account_id, effect) in &effects {
            let account = self
                .accounts
                .get(account_id)
                .expect("bound accounts were checked");
            let mut required = effect.consume.clone();
            if let Err(asset) = required.checked_add(&effect.preserve) {
                return invalid_analysis(vec![ApplyError::RateOverflow {
                    rate: exchange.rate().clone(),
                    asset,
                }]);
            }
            let available = relevant_balances(account, &required);
            let missing = account.balances().shortfall(&required);
            account_assessments.push(AccountAssessment::new(
                account_id.clone(),
                available,
                required,
                effect.consume.clone(),
                effect.produce.clone(),
                effect.preserve.clone(),
            ));
            if !missing.is_empty() {
                shortfalls.push(AccountShortfall::new(account_id.clone(), missing));
            }
        }

        if !shortfalls.is_empty() {
            return Analysis {
                assessment: ExchangeAssessment::Infeasible {
                    accounts: account_assessments,
                    shortfalls,
                },
                prepared_accounts: None,
            };
        }

        let mut accounts = self.accounts.clone();
        let mut deltas = Vec::with_capacity(effects.len());
        for (account_id, effect) in effects {
            let account = Arc::make_mut(
                accounts
                    .get_mut(&account_id)
                    .expect("bound accounts were checked"),
            );
            if let Err(error) = account.withdraw(&effect.consume) {
                return invalid_analysis(vec![map_account_error(account_id.clone(), error)]);
            }
            if let Err(error) = account.deposit(&effect.produce) {
                return invalid_analysis(vec![map_account_error(account_id.clone(), error)]);
            }
            deltas.push(AccountDelta::new(
                account_id,
                effect.consume,
                effect.produce,
                effect.preserve,
            ));
        }

        for invariant in self.invariants.iter() {
            let Some(before) = invariant.measure(&self.accounts) else {
                return invalid_analysis(vec![ApplyError::InvariantOverflow {
                    invariant: invariant.name().to_owned(),
                }]);
            };
            let Some(after) = invariant.measure(&accounts) else {
                return invalid_analysis(vec![ApplyError::InvariantOverflow {
                    invariant: invariant.name().to_owned(),
                }]);
            };
            if before != after {
                return invalid_analysis(vec![ApplyError::InvariantViolation {
                    invariant: invariant.name().to_owned(),
                    before,
                    after,
                }]);
            }
        }

        Analysis {
            assessment: ExchangeAssessment::Applicable {
                accounts: account_assessments,
                projected_deltas: deltas,
            },
            prepared_accounts: Some(accounts),
        }
    }
}

struct FingerprintHasher {
    state: u64,
}

impl FingerprintHasher {
    const fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }
}

impl Hasher for FingerprintHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

pub struct EconomicView<'a, AccountId, A, RateId, Role, N = u64> {
    economy: &'a Economy<AccountId, A, RateId, Role, N>,
    visible: BTreeSet<AccountId>,
}

/// Canonical actor-visible state, including the view's account boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObservationKey<AccountId, A, N = u64> {
    visible_accounts: Vec<AccountId>,
    balances: Vec<(AccountId, A, Quantity<N>)>,
}

impl<AccountId, A, N> ObservationKey<AccountId, A, N> {
    pub fn visible_accounts(&self) -> &[AccountId] {
        &self.visible_accounts
    }

    pub fn balances(&self) -> &[(AccountId, A, Quantity<N>)] {
        &self.balances
    }
}

impl<AccountId, A, RateId, Role, N> EconomicView<'_, AccountId, A, RateId, Role, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
{
    pub fn account(&self, id: &AccountId) -> Option<&Account<A, N>> {
        self.visible
            .contains(id)
            .then(|| self.economy.account(id))
            .flatten()
    }

    pub fn balance(&self, account: &AccountId, asset: &A) -> Option<Quantity<N>> {
        self.account(account).map(|visible| visible.balance(asset))
    }

    /// Returns a canonical identity containing only visible economic state.
    ///
    /// Equal observation keys mean that this view cannot distinguish the two
    /// snapshots. Search code can therefore key information sets without
    /// copying hidden accounts into a parallel state representation.
    pub fn observation_key(&self) -> ObservationKey<AccountId, A, N> {
        let mut balances = Vec::new();
        for account_id in &self.visible {
            let Some(account) = self.economy.account(account_id) else {
                continue;
            };
            for (asset, quantity) in account.balances().iter() {
                balances.push((account_id.clone(), asset.clone(), quantity.clone()));
            }
        }
        balances.sort();
        ObservationKey {
            visible_accounts: self.visible.iter().cloned().collect(),
            balances,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ApplyError<RateId, Role, AccountId, A, N = u64>
where
    N: QuantityScalar,
{
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
    Infeasible {
        shortfalls: Vec<AccountShortfall<AccountId, A, N>>,
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
        before: N::SignedMeasure,
        after: N::SignedMeasure,
    },
}

impl<RateId, Role, AccountId, A, N> fmt::Display for ApplyError<RateId, Role, AccountId, A, N>
where
    N: QuantityScalar,
{
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
            Self::Infeasible { .. } => formatter.write_str("exchange is infeasible"),
            Self::BalanceOverflow { .. } => formatter.write_str("account balance overflow"),
            Self::InvariantOverflow { .. } => formatter.write_str("invariant arithmetic overflow"),
            Self::InvariantViolation { .. } => formatter.write_str("declared invariant violation"),
        }
    }
}

impl<RateId, Role, AccountId, A, N> Error for ApplyError<RateId, Role, AccountId, A, N>
where
    RateId: fmt::Debug,
    Role: fmt::Debug,
    AccountId: fmt::Debug,
    A: fmt::Debug,
    N: QuantityScalar,
{
}

#[derive(Debug, Clone)]
struct Effect<A, N> {
    consume: Basket<A, N>,
    produce: Basket<A, N>,
    preserve: Basket<A, N>,
}

impl<A, N> Default for Effect<A, N> {
    fn default() -> Self {
        Self {
            consume: Basket::new(),
            produce: Basket::new(),
            preserve: Basket::new(),
        }
    }
}

struct Analysis<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    assessment: ExchangeAssessment<AccountId, A, RateId, Role, N>,
    prepared_accounts: Option<HashMap<AccountId, Arc<Account<A, N>>>>,
}

fn merge_scaled<A, N>(
    target: &mut Basket<A, N>,
    source: &Basket<A, N>,
    units: &Quantity<N>,
) -> Result<(), A>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    let scaled = source.checked_scale(units)?;
    target.checked_add(&scaled)
}

fn invalid_analysis<AccountId, A, RateId, Role, N>(
    issues: Vec<ApplyError<RateId, Role, AccountId, A, N>>,
) -> Analysis<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    debug_assert!(!issues.is_empty());
    Analysis {
        assessment: ExchangeAssessment::Invalid { issues },
        prepared_accounts: None,
    }
}

fn relevant_balances<A, N>(account: &Account<A, N>, required: &Basket<A, N>) -> Basket<A, N>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    required
        .iter()
        .map(|(asset, _)| (asset.clone(), account.balance(asset)))
        .collect()
}

fn map_account_error<RateId, Role, AccountId, A, N>(
    account: AccountId,
    error: AccountError<A, N>,
) -> ApplyError<RateId, Role, AccountId, A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    match error {
        AccountError::InsufficientBalance { shortfall } => ApplyError::Infeasible {
            shortfalls: vec![AccountShortfall::new(account, shortfall)],
        },
        AccountError::Overflow { asset } => ApplyError::BalanceOverflow { account, asset },
    }
}
