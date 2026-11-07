use crate::{
    Account, AccountDelta, AccountError, Basket, ComputedAmount, Exchange, LinearInvariant,
    Quantity, QuantityComparison, QuantityExpression, QuantityScalar, Rate, RateCondition, Receipt,
    Trace,
};
use indexmap::IndexMap;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use thiserror::Error;

// Keep the lazy borrowed account projection explicit for extraction.
struct AccountEntries<'a, AccountId, A, N>(indexmap::map::Iter<'a, AccountId, Arc<Account<A, N>>>);

impl<'a, AccountId, A, N> Iterator for AccountEntries<'a, AccountId, A, N> {
    type Item = (&'a AccountId, &'a Account<A, N>);

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some((id, account)) => Some((id, account.as_ref())),
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.0.len();
        (remaining, Some(remaining))
    }
}

fn collect_basket_assets<'a, A: Ord, N>(contents: &'a Basket<A, N>, assets: &mut BTreeSet<&'a A>) {
    for (asset, _) in contents.iter() {
        assets.insert(asset);
    }
}

fn collect_invariant_assets<'a, A: Eq + Hash + Ord>(
    invariant: &'a LinearInvariant<A>,
    assets: &mut BTreeSet<&'a A>,
) {
    for (asset, _) in invariant.weights() {
        assets.insert(asset);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "AccountId: Serialize + Ord, A: Serialize, N: Serialize",
    deserialize = "AccountId: Deserialize<'de> + Ord, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
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

impl<AccountId, A, N> Goal<AccountId, A, N>
where
    AccountId: Ord,
    A: Ord,
{
    /// Returns every asset identity referenced by this goal.
    // Shared map iteration uses the same reviewed Aeneas adapter as goal matching.
    #[allow(clippy::for_kv_map)]
    pub fn asset_keys(&self) -> BTreeSet<&A> {
        let mut assets = BTreeSet::new();
        for (_, required) in &self.required {
            Self::collect_assets(required, &mut assets);
        }
        assets
    }

    fn collect_assets<'a>(required: &'a Basket<A, N>, assets: &mut BTreeSet<&'a A>) {
        for (asset, _) in required.iter() {
            assets.insert(asset);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Economy<AccountId, A, RateId, Role, N = u64> {
    accounts: IndexMap<AccountId, Arc<Account<A, N>>>,
    rates: Arc<IndexMap<RateId, Rate<Role, A, N>>>,
    invariants: Arc<Vec<LinearInvariant<A>>>,
}

/// Compact logical-state identity for in-process search and transpositions.
///
/// The fingerprint is deterministic for a fixed ontology implementation and
/// executable, but it is not a durable serialization or collision-proof key.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct StateFingerprint(u64);

impl StateFingerprint {
    pub const fn get(self) -> u64 {
        self.0
    }
}

type StructuralValidationResult<'a, AccountId, A, RateId, Role, N> =
    Result<&'a Rate<Role, A, N>, Vec<ApplyError<RateId, Role, AccountId, A, N>>>;

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

pub type ModelBuildResult<AccountId, A, RateId, Role, N = u64> =
    Result<Economy<AccountId, A, RateId, Role, N>, ModelBuildError<AccountId, RateId>>;

/// The high-level result of assessing an exchange without mutating the economy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssessmentStatus {
    Applicable,
    Infeasible,
    Invalid,
}

/// One account's complete rate-derived requirements and effects.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "AccountId: Serialize, A: Serialize, N: Serialize",
    deserialize = "AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "AccountId: Serialize, A: Serialize, N: Serialize",
    deserialize = "AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
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
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(bound(
    serialize = "AccountId: Serialize, A: Serialize, RateId: Serialize, Role: Serialize, N: Serialize, N::SignedMeasure: Serialize"
))]
#[schemars(
    bound = "AccountId: JsonSchema, A: JsonSchema, RateId: JsonSchema, Role: JsonSchema, N: JsonSchema, N::SignedMeasure: JsonSchema"
)]
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

    #[allow(clippy::manual_find)] // Extract the primitive first-match loop; slice::find has unsupported lifetimes.
    pub fn account(&self, account: &AccountId) -> Option<&AccountAssessment<AccountId, A, N>>
    where
        AccountId: PartialEq,
    {
        for assessment in self.accounts() {
            if assessment.account() == account {
                return Some(assessment);
            }
        }
        None
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
        for shortfall in self.shortfalls() {
            if shortfall.account() == account {
                return Some(shortfall.missing());
            }
        }
        None
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
    accounts: IndexMap<AccountId, Account<A, N>>,
    rates: IndexMap<RateId, Rate<Role, A, N>>,
    invariants: Vec<LinearInvariant<A>>,
    issues: Vec<ModelIssue<AccountId, RateId>>,
}

impl<AccountId, A, RateId, Role, N> EconomyBuilder<AccountId, A, RateId, Role, N>
where
    AccountId: Eq + Hash,
    RateId: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            accounts: IndexMap::new(),
            rates: IndexMap::new(),
            invariants: Vec::new(),
            issues: Vec::new(),
        }
    }

    pub fn account(mut self, id: AccountId, account: Account<A, N>) -> Self {
        if self.accounts.contains_key(&id) {
            self.issues
                .push(ModelIssue::DuplicateAccount { account: id });
        } else {
            self.accounts.insert(id, account);
        }
        self
    }

    pub fn rate(mut self, id: RateId, rate: Rate<Role, A, N>) -> Self {
        if self.rates.contains_key(&id) {
            self.issues.push(ModelIssue::DuplicateRate { rate: id });
        } else {
            self.rates.insert(id, rate);
        }
        self
    }

    pub fn invariant(mut self, invariant: LinearInvariant<A>) -> Self {
        self.invariants.push(invariant);
        self
    }

    pub fn build(self) -> ModelBuildResult<AccountId, A, RateId, Role, N> {
        if !self.issues.is_empty() {
            return Err(ModelBuildError {
                issues: self.issues,
            });
        }
        let mut accounts = IndexMap::new();
        for (id, holder) in self.accounts {
            accounts.insert(id, Arc::new(holder));
        }
        Ok(Economy {
            accounts,
            rates: Arc::new(self.rates),
            invariants: Arc::new(self.invariants),
        })
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelIssue<AccountId, RateId> {
    DuplicateAccount { account: AccountId },
    DuplicateRate { rate: RateId },
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[error("model definition contains duplicate identifiers")]
pub struct ModelBuildError<AccountId, RateId> {
    issues: Vec<ModelIssue<AccountId, RateId>>,
}

impl<AccountId, RateId> ModelBuildError<AccountId, RateId> {
    pub fn issues(&self) -> &[ModelIssue<AccountId, RateId>] {
        &self.issues
    }
}

impl<AccountId, A, RateId, Role, N> Serialize for Economy<AccountId, A, RateId, Role, N>
where
    AccountId: Serialize,
    A: Serialize,
    RateId: Serialize,
    Role: Serialize + Ord,
    N: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let accounts = self
            .accounts
            .iter()
            .map(|(id, account)| (id, account.as_ref()))
            .collect::<Vec<_>>();
        let rates = self.rates.iter().collect::<Vec<_>>();
        let mut state = serializer.serialize_struct("Economy", 3)?;
        state.serialize_field("accounts", &accounts)?;
        state.serialize_field("rates", &rates)?;
        state.serialize_field("invariants", self.invariants.as_ref())?;
        state.end()
    }
}

#[derive(Deserialize)]
#[serde(bound(
    deserialize = "AccountId: Deserialize<'de>, A: Deserialize<'de> + Clone + Eq + Hash, RateId: Deserialize<'de>, Role: Deserialize<'de> + Clone + Ord, N: Deserialize<'de> + QuantityScalar"
))]
struct EconomyData<AccountId, A, RateId, Role, N> {
    accounts: Vec<(AccountId, Account<A, N>)>,
    rates: Vec<(RateId, Rate<Role, A, N>)>,
    invariants: Vec<LinearInvariant<A>>,
}

impl<'de, AccountId, A, RateId, Role, N> Deserialize<'de> for Economy<AccountId, A, RateId, Role, N>
where
    AccountId: Deserialize<'de> + Eq + Hash,
    A: Deserialize<'de> + Clone + Eq + Hash,
    RateId: Deserialize<'de> + Eq + Hash,
    Role: Deserialize<'de> + Clone + Ord,
    N: Deserialize<'de> + QuantityScalar,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = EconomyData::<AccountId, A, RateId, Role, N>::deserialize(deserializer)?;
        let mut builder = EconomyBuilder::new();
        for (id, account) in data.accounts {
            builder = builder.account(id, account);
        }
        for (id, rate) in data.rates {
            builder = builder.rate(id, rate);
        }
        for invariant in data.invariants {
            builder = builder.invariant(invariant);
        }
        builder.build().map_err(serde::de::Error::custom)
    }
}

impl<AccountId, A, RateId, Role, N> JsonSchema for Economy<AccountId, A, RateId, Role, N>
where
    AccountId: JsonSchema,
    A: JsonSchema,
    RateId: JsonSchema,
    Role: JsonSchema,
    N: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!(
            "Economy_{}_{}_{}_{}_{}",
            AccountId::schema_name(),
            A::schema_name(),
            RateId::schema_name(),
            Role::schema_name(),
            N::schema_name()
        )
        .into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        #[allow(dead_code)]
        #[derive(JsonSchema)]
        struct EconomySchema<AccountId, A, RateId, Role, N> {
            accounts: Vec<(AccountId, Account<A, N>)>,
            rates: Vec<(RateId, Rate<Role, A, N>)>,
            invariants: Vec<LinearInvariant<A>>,
        }

        generator.subschema_for::<EconomySchema<AccountId, A, RateId, Role, N>>()
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
    /// Account inspection cannot bypass exchange validation by mutating balances.
    ///
    /// ```compile_fail,E0596
    /// use axionomy::{Economy, basket};
    /// fn unauthorized_write(world: &mut Economy<u8, u8, u8, u8>) {
    ///     let account = world.account(&0).unwrap();
    ///     account.deposit(&basket([(0, 1)])).unwrap();
    /// }
    /// ```
    pub fn account(&self, id: &AccountId) -> Option<&Account<A, N>> {
        match self.accounts.get(id) {
            Some(holder) => Some(holder.as_ref()),
            None => None,
        }
    }

    pub fn balance(&self, account_id: &AccountId, asset: &A) -> Quantity<N> {
        match self.accounts.get(account_id) {
            Some(holder) => holder.balance(asset),
            None => Quantity::default(),
        }
    }

    pub fn accounts(&self) -> impl Iterator<Item = (&AccountId, &Account<A, N>)> {
        AccountEntries(self.accounts.iter())
    }

    pub fn rate(&self, id: &RateId) -> Option<&Rate<Role, A, N>> {
        self.rates.get(id)
    }

    pub fn rate_ids(&self) -> impl Iterator<Item = &RateId> {
        self.rates.keys()
    }

    /// Returns the model's declared conservation laws.
    ///
    /// Invariants remain immutable engine authority. This read-only iterator is
    /// useful to explain and inspect a closed model without duplicating its
    /// definition in presentation or transport layers.
    pub fn invariants(&self) -> impl Iterator<Item = &LinearInvariant<A>> {
        self.invariants.iter()
    }

    /// Returns every asset identity referenced by the closed model.
    ///
    /// The set covers initial balances, every rate effect and requirement,
    /// and declared invariant weights. It supports construction-time schema
    /// validation without giving validators mutation authority.
    pub fn asset_keys(&self) -> BTreeSet<&A> {
        let mut assets = BTreeSet::new();

        for holder in self.accounts.values() {
            collect_basket_assets(holder.balances(), &mut assets);
        }
        for rule in self.rates.values() {
            rule.collect_assets(&mut assets);
        }
        for invariant in self.invariants.iter() {
            collect_invariant_assets(invariant, &mut assets);
        }

        assets
    }

    pub fn matches(&self, goal: &Goal<AccountId, A, N>) -> bool {
        for (account_id, required) in goal.requirements() {
            let Some(holder) = self.accounts.get(account_id) else {
                return false;
            };
            if !holder.balances().contains(required) {
                return false;
            }
        }
        true
    }

    pub fn view(
        &self,
        visible_accounts: impl IntoIterator<Item = AccountId>,
    ) -> EconomicView<'_, AccountId, A, RateId, Role, N> {
        let mut visible = BTreeSet::new();
        for account_id in visible_accounts {
            visible.insert(account_id);
        }
        EconomicView {
            economy: self,
            visible,
        }
    }

    /// Canonical nonzero balances, ignoring account and asset insertion order.
    /// Empty and absent accounts have the same key; compare keys within the
    /// same account domain when account existence matters.
    pub fn state_key(&self) -> Vec<(AccountId, A, Quantity<N>)> {
        let mut key = Vec::new();
        for (account_id, holder) in &self.accounts {
            for (asset, value) in holder.balances().iter() {
                key.push((account_id.clone(), asset.clone(), value.clone()));
            }
        }
        key.sort();
        key
    }

    fn observation_key_for(
        &self,
        visible: &BTreeSet<AccountId>,
    ) -> ObservationKey<AccountId, A, N> {
        let mut balances = Vec::new();
        let mut visible_accounts = Vec::new();
        let mut existing_accounts = Vec::new();
        for account_id in visible {
            visible_accounts.push(account_id.clone());
            let Some(holder) = self.account(account_id) else {
                continue;
            };
            existing_accounts.push(account_id.clone());
            for (asset, value) in holder.balances().iter() {
                balances.push((account_id.clone(), asset.clone(), value.clone()));
            }
        }
        balances.sort();
        ObservationKey {
            visible_accounts,
            existing_accounts,
            balances,
        }
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
        proposal: Exchange<RateId, Role, AccountId, N>,
    ) -> SimulationResult<AccountId, A, RateId, Role, N> {
        let mut fork = self.fork();
        let receipt = fork.apply(proposal)?;
        Ok((fork, receipt))
    }

    /// Explains one exchange without mutating the economy.
    pub fn assess(
        &self,
        proposal: &Exchange<RateId, Role, AccountId, N>,
    ) -> ExchangeAssessment<AccountId, A, RateId, Role, N> {
        self.analyze(proposal).assessment
    }

    /// Returns whether one exchange is applicable to the current snapshot.
    #[must_use]
    pub fn is_applicable(&self, proposal: &Exchange<RateId, Role, AccountId, N>) -> bool {
        self.assess(proposal).is_applicable()
    }

    /// Retains only candidates applicable to the current snapshot.
    #[must_use]
    pub fn applicable(
        &self,
        candidates: impl IntoIterator<Item = Exchange<RateId, Role, AccountId, N>>,
    ) -> Vec<Exchange<RateId, Role, AccountId, N>> {
        let mut applicable = Vec::new();
        for proposal in candidates {
            if self.is_applicable(&proposal) {
                applicable.push(proposal);
            }
        }
        applicable
    }

    pub fn apply(
        &mut self,
        proposal: Exchange<RateId, Role, AccountId, N>,
    ) -> ApplyResult<AccountId, A, RateId, Role, N> {
        let Analysis {
            assessment,
            prepared_accounts,
        } = self.analyze(&proposal);

        match assessment {
            ExchangeAssessment::Applicable {
                projected_deltas, ..
            } => {
                for (account_id, holder) in
                    prepared_accounts.expect("applicable assessments have prepared accounts")
                {
                    self.accounts[&account_id] = holder;
                }
                Ok(Receipt::new(proposal, projected_deltas))
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

    /// Applies proposals in order and stops at the first error, retaining the
    /// changes from earlier successful proposals. Returns ordered receipts only
    /// when every proposal succeeds.
    pub fn replay(
        &mut self,
        trace: &Trace<RateId, Role, AccountId, N>,
    ) -> ReplayResult<AccountId, A, RateId, Role, N> {
        let mut receipts = Vec::new();
        for proposal in trace.exchanges() {
            receipts.push(self.apply(proposal.clone())?);
        }
        Ok(receipts)
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

    pub(crate) fn validate_structure(
        &self,
        proposal: &Exchange<RateId, Role, AccountId, N>,
    ) -> StructuralValidationResult<'_, AccountId, A, RateId, Role, N> {
        let mut issues = Vec::new();
        if proposal.units().is_zero() {
            issues.push(ApplyError::ZeroUnits);
        }

        let Some(rule) = self.rates.get(proposal.rate()) else {
            issues.push(ApplyError::MissingRate {
                rate: proposal.rate().clone(),
            });
            return Err(issues);
        };

        for role in rule.roles() {
            if !proposal.bindings().contains_key(role) {
                issues.push(ApplyError::MissingBinding { role: role.clone() });
            }
        }
        for role in proposal.bindings().keys() {
            if !rule.has_role(role) {
                issues.push(ApplyError::UnknownBinding { role: role.clone() });
            }
        }

        let parameter_names = rule.parameter_names();
        for name in parameter_names.clone() {
            if !proposal.parameters().contains_key(name) {
                issues.push(ApplyError::MissingParameter {
                    name: name.to_owned(),
                });
            }
        }
        for name in proposal.parameters().keys() {
            if !parameter_names.contains(name.as_str()) {
                issues.push(ApplyError::UnknownParameter { name: name.clone() });
            }
        }
        for (left, right) in rule.distinct_roles() {
            if let (Some(left_account), Some(right_account)) = (
                proposal.bindings().get(left),
                proposal.bindings().get(right),
            ) && left_account == right_account
            {
                issues.push(ApplyError::RolesMustDiffer {
                    left: left.clone(),
                    right: right.clone(),
                });
            }
        }

        let mut missing_accounts = BTreeSet::new();
        for role in rule.roles() {
            if let Some(holder) = proposal.bindings().get(role)
                && !self.accounts.contains_key(holder)
            {
                missing_accounts.insert(holder.clone());
            }
        }
        for holder in missing_accounts {
            issues.push(ApplyError::MissingAccount { account: holder });
        }

        if issues.is_empty() {
            Ok(rule)
        } else {
            Err(issues)
        }
    }

    fn merge_computed_effects(
        &self,
        amounts: &[ComputedAmount<Role, A, N>],
        direction: ComputedDirection,
        proposal: &Exchange<RateId, Role, AccountId, N>,
        effects: &mut BTreeMap<AccountId, Effect<A, N>>,
    ) -> Result<(), ApplyError<RateId, Role, AccountId, A, N>> {
        for amount in amounts {
            let value = match evaluate_expression(amount.quantity(), proposal, &self.accounts) {
                Ok(value) => value,
                Err(error) => {
                    return Err(ApplyError::RateLawEvaluation {
                        rate: proposal.rate().clone(),
                        message: error.into_message(),
                    });
                }
            };
            let account_id = proposal
                .bindings()
                .get(amount.role())
                .expect("computed rate roles were checked")
                .clone();
            let effect = effects.entry(account_id).or_default();
            let merged = effect.merge_computed(direction, amount.asset().clone(), value);
            if merged.is_err() {
                return Err(ApplyError::RateOverflow {
                    rate: proposal.rate().clone(),
                    asset: amount.asset().clone(),
                });
            }
        }
        Ok(())
    }

    fn accounts_with_prepared(
        &self,
        prepared_accounts: &PreparedAccounts<AccountId, A, N>,
    ) -> IndexMap<AccountId, Arc<Account<A, N>>> {
        let mut after_accounts = self.accounts.clone();
        for (account_id, holder) in prepared_accounts {
            after_accounts[account_id] = holder.clone();
        }
        after_accounts
    }

    fn check_conditions(
        &self,
        rule: &Rate<Role, A, N>,
        proposal: &Exchange<RateId, Role, AccountId, N>,
    ) -> Result<(), ApplyError<RateId, Role, AccountId, A, N>> {
        for condition in rule.conditions() {
            let satisfied = match evaluate_condition(condition, proposal, &self.accounts) {
                Ok(satisfied) => satisfied,
                Err(error) => {
                    return Err(ApplyError::RateLawEvaluation {
                        rate: proposal.rate().clone(),
                        message: error.into_message(),
                    });
                }
            };
            if !satisfied {
                return Err(ApplyError::RateConditionFailed {
                    rate: proposal.rate().clone(),
                    condition: condition.name().to_owned(),
                });
            }
        }

        Ok(())
    }

    fn aggregate_effects(
        &self,
        rule: &Rate<Role, A, N>,
        proposal: &Exchange<RateId, Role, AccountId, N>,
    ) -> AggregationResult<AccountId, A, RateId, Role, N> {
        let mut effects: BTreeMap<AccountId, Effect<A, N>> = BTreeMap::new();
        for role in rule.roles() {
            let account_id = proposal
                .bindings()
                .get(role)
                .expect("all role bindings were checked")
                .clone();
            let effect = effects.entry(account_id).or_default();
            if let Some(consume) = rule.consumed(role)
                && let Err(asset) = merge_scaled(&mut effect.consume, consume, proposal.units())
            {
                return Err(ApplyError::RateOverflow {
                    rate: proposal.rate().clone(),
                    asset,
                });
            }
            if let Some(produce) = rule.produced(role)
                && let Err(asset) = merge_scaled(&mut effect.produce, produce, proposal.units())
            {
                return Err(ApplyError::RateOverflow {
                    rate: proposal.rate().clone(),
                    asset,
                });
            }
            if let Some(preserve) = rule.preserved(role)
                && let Err(asset) = effect.preserve.checked_add(preserve)
            {
                return Err(ApplyError::RateOverflow {
                    rate: proposal.rate().clone(),
                    asset,
                });
            }
        }

        self.merge_computed_effects(
            rule.computed_consumed(),
            ComputedDirection::Consume,
            proposal,
            &mut effects,
        )?;
        self.merge_computed_effects(
            rule.computed_produced(),
            ComputedDirection::Produce,
            proposal,
            &mut effects,
        )?;
        self.merge_computed_effects(
            rule.computed_preserved(),
            ComputedDirection::Preserve,
            proposal,
            &mut effects,
        )?;

        Ok(effects)
    }

    fn preflight_effects(
        &self,
        proposal: &Exchange<RateId, Role, AccountId, N>,
        effects: &BTreeMap<AccountId, Effect<A, N>>,
    ) -> AvailabilityResult<AccountId, A, RateId, Role, N> {
        let mut account_assessments = Vec::with_capacity(effects.len());
        let mut shortfalls = Vec::new();
        for (account_id, effect) in effects {
            let holder = self
                .accounts
                .get(account_id)
                .expect("bound accounts were checked");
            let mut required = effect.consume.clone();
            if let Err(asset) = required.checked_add(&effect.preserve) {
                return Err(ApplyError::RateOverflow {
                    rate: proposal.rate().clone(),
                    asset,
                });
            }
            let available = relevant_balances(holder, &required);
            let missing = holder.balances().shortfall(&required);
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

        Ok(Availability {
            accounts: account_assessments,
            shortfalls,
        })
    }

    fn prepare_effects(
        &self,
        effects: BTreeMap<AccountId, Effect<A, N>>,
    ) -> PreparationResult<AccountId, A, RateId, Role, N> {
        let mut prepared_accounts = Vec::with_capacity(effects.len());
        let mut deltas = Vec::with_capacity(effects.len());
        for (account_id, effect) in effects {
            let holder = match self
                .accounts
                .get(&account_id)
                .expect("bound accounts were checked")
                .prepare_exchange(&effect.consume, &effect.produce)
            {
                Ok(holder) => holder,
                Err(error) => {
                    return Err(map_account_error(account_id.clone(), error));
                }
            };
            deltas.push(AccountDelta::new(
                account_id.clone(),
                effect.consume,
                effect.produce,
                effect.preserve,
            ));
            prepared_accounts.push((account_id, Arc::new(holder)));
        }

        Ok(Preparation {
            accounts: prepared_accounts,
            deltas,
        })
    }

    fn check_invariants(
        &self,
        deltas: &[AccountDelta<AccountId, A, N>],
        prepared_accounts: &PreparedAccounts<AccountId, A, N>,
    ) -> Result<(), ApplyError<RateId, Role, AccountId, A, N>> {
        for invariant in self.invariants.iter() {
            let Some(conserved) = invariant.conserves(deltas) else {
                return Err(ApplyError::InvariantOverflow {
                    invariant: invariant.name().to_owned(),
                });
            };
            if !conserved {
                let Some(before) = invariant.measure(&self.accounts) else {
                    return Err(ApplyError::InvariantOverflow {
                        invariant: invariant.name().to_owned(),
                    });
                };
                let after_accounts = self.accounts_with_prepared(prepared_accounts);
                let Some(after) = invariant.measure(&after_accounts) else {
                    return Err(ApplyError::InvariantOverflow {
                        invariant: invariant.name().to_owned(),
                    });
                };
                return Err(ApplyError::InvariantViolation {
                    invariant: invariant.name().to_owned(),
                    before,
                    after,
                });
            }
        }

        Ok(())
    }

    fn analyze(
        &self,
        proposal: &Exchange<RateId, Role, AccountId, N>,
    ) -> Analysis<AccountId, A, RateId, Role, N> {
        let rule = match self.validate_structure(proposal) {
            Ok(rule) => rule,
            Err(issues) => return invalid_analysis(issues),
        };
        if let Err(error) = self.check_conditions(rule, proposal) {
            return invalid_analysis(vec![error]);
        }
        let effects = match self.aggregate_effects(rule, proposal) {
            Ok(effects) => effects,
            Err(error) => return invalid_analysis(vec![error]),
        };
        let availability = match self.preflight_effects(proposal, &effects) {
            Ok(availability) => availability,
            Err(error) => return invalid_analysis(vec![error]),
        };
        if !availability.shortfalls.is_empty() {
            return Analysis {
                assessment: ExchangeAssessment::Infeasible {
                    accounts: availability.accounts,
                    shortfalls: availability.shortfalls,
                },
                prepared_accounts: None,
            };
        }
        let prepared = match self.prepare_effects(effects) {
            Ok(prepared) => prepared,
            Err(error) => return invalid_analysis(vec![error]),
        };
        if let Err(error) = self.check_invariants(&prepared.deltas, &prepared.accounts) {
            return invalid_analysis(vec![error]);
        }
        Analysis {
            assessment: ExchangeAssessment::Applicable {
                accounts: availability.accounts,
                projected_deltas: prepared.deltas,
            },
            prepared_accounts: Some(prepared.accounts),
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

/// Canonical actor-visible balances, account existence, and visibility boundary.
/// Produced keys ignore storage insertion order. Serialized keys must include
/// `existing_accounts`; older keys cannot recover this information from balances.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "AccountId: Serialize, A: Serialize, N: Serialize",
    deserialize = "AccountId: Deserialize<'de>, A: Deserialize<'de> + Eq + Hash, N: Deserialize<'de> + QuantityScalar"
))]
pub struct ObservationKey<AccountId, A, N = u64> {
    visible_accounts: Vec<AccountId>,
    existing_accounts: Vec<AccountId>,
    balances: Vec<(AccountId, A, Quantity<N>)>,
}

impl<AccountId, A, N> ObservationKey<AccountId, A, N> {
    pub fn visible_accounts(&self) -> &[AccountId] {
        &self.visible_accounts
    }

    /// Visible accounts present in the economy, including empty accounts.
    pub fn existing_accounts(&self) -> &[AccountId] {
        &self.existing_accounts
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
        if self.visible.contains(id) {
            self.economy.account(id)
        } else {
            None
        }
    }

    pub fn balance(&self, account_id: &AccountId, asset: &A) -> Option<Quantity<N>> {
        match self.account(account_id) {
            Some(holder) => Some(holder.balance(asset)),
            None => None,
        }
    }

    /// Returns a canonical identity containing only visible economic state.
    ///
    /// Equal produced keys have the same visibility boundary, visible account
    /// existence, and balance-query results, including missing versus zero.
    /// They ignore storage insertion order and all hidden accounts.
    pub fn observation_key(&self) -> ObservationKey<AccountId, A, N> {
        self.economy.observation_key_for(&self.visible)
    }
}

#[derive(Debug, Clone, Error, Serialize, JsonSchema)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize, AccountId: Serialize, A: Serialize, N: Serialize, N::SignedMeasure: Serialize"
))]
#[schemars(
    bound = "RateId: JsonSchema, Role: JsonSchema, AccountId: JsonSchema, A: JsonSchema, N: JsonSchema, N::SignedMeasure: JsonSchema"
)]
pub enum ApplyError<RateId, Role, AccountId, A, N = u64>
where
    N: QuantityScalar,
{
    #[error("rate does not exist")]
    MissingRate { rate: RateId },
    #[error("exchange is missing a role binding")]
    MissingBinding { role: Role },
    #[error("exchange contains an unknown role")]
    UnknownBinding { role: Role },
    #[error("rate roles must bind to different accounts")]
    RolesMustDiffer { left: Role, right: Role },
    #[error("bound account does not exist")]
    MissingAccount { account: AccountId },
    #[error("exchange units must be greater than zero")]
    ZeroUnits,
    #[error("exchange is missing a required rate parameter")]
    MissingParameter { name: String },
    #[error("exchange contains an unknown rate parameter")]
    UnknownParameter { name: String },
    #[error("scaled rate overflow")]
    RateOverflow { rate: RateId, asset: A },
    #[error("state-evaluated rate law could not be evaluated: {message}")]
    RateLawEvaluation { rate: RateId, message: String },
    #[error("state-evaluated rate condition failed")]
    RateConditionFailed { rate: RateId, condition: String },
    #[error("exchange is infeasible")]
    Infeasible {
        shortfalls: Vec<AccountShortfall<AccountId, A, N>>,
    },
    #[error("account balance overflow")]
    BalanceOverflow { account: AccountId, asset: A },
    #[error("invariant arithmetic overflow")]
    InvariantOverflow { invariant: String },
    #[error("declared invariant violation")]
    InvariantViolation {
        invariant: String,
        before: N::SignedMeasure,
        after: N::SignedMeasure,
    },
}

#[derive(Debug, Clone)]
struct Effect<A, N> {
    consume: Basket<A, N>,
    produce: Basket<A, N>,
    preserve: Basket<A, N>,
}

#[derive(Debug, Clone, Copy)]
enum ComputedDirection {
    Consume,
    Produce,
    Preserve,
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

type PreparedAccounts<AccountId, A, N> = Vec<(AccountId, Arc<Account<A, N>>)>;

type AggregationResult<AccountId, A, RateId, Role, N> =
    Result<BTreeMap<AccountId, Effect<A, N>>, ApplyError<RateId, Role, AccountId, A, N>>;

type AvailabilityResult<AccountId, A, RateId, Role, N> =
    Result<Availability<AccountId, A, N>, ApplyError<RateId, Role, AccountId, A, N>>;

type PreparationResult<AccountId, A, RateId, Role, N> =
    Result<Preparation<AccountId, A, N>, ApplyError<RateId, Role, AccountId, A, N>>;

struct Availability<AccountId, A, N> {
    accounts: Vec<AccountAssessment<AccountId, A, N>>,
    shortfalls: Vec<AccountShortfall<AccountId, A, N>>,
}

struct Preparation<AccountId, A, N> {
    accounts: PreparedAccounts<AccountId, A, N>,
    deltas: Vec<AccountDelta<AccountId, A, N>>,
}

impl<A: Clone + Eq + Hash, N: QuantityScalar> Effect<A, N> {
    fn merge_computed(
        &mut self,
        direction: ComputedDirection,
        asset: A,
        value: Quantity<N>,
    ) -> Result<(), A> {
        match direction {
            ComputedDirection::Consume => merge_quantity(&mut self.consume, asset, value),
            ComputedDirection::Produce => merge_quantity(&mut self.produce, asset, value),
            ComputedDirection::Preserve => merge_quantity(&mut self.preserve, asset, value),
        }
    }
}

struct Analysis<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    assessment: ExchangeAssessment<AccountId, A, RateId, Role, N>,
    prepared_accounts: Option<PreparedAccounts<AccountId, A, N>>,
}

pub(crate) fn merge_scaled<A, N>(
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

pub(crate) fn merge_quantity<A, N>(
    target: &mut Basket<A, N>,
    asset: A,
    amount: Quantity<N>,
) -> Result<(), A>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    let Some(total) = target.quantity(&asset).checked_add(&amount) else {
        return Err(asset);
    };
    target.insert(asset, total);
    Ok(())
}

/// Typed failures inside the evaluator; text is produced only for public diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExpressionError {
    MissingParameter(String),
    UnboundRole,
    MissingAccount,
    AdditionOverflow,
    SubtractionUnderflow,
    MultiplicationOverflow,
    DivisionByZero,
}

impl ExpressionError {
    pub(crate) fn into_message(self) -> String {
        match self {
            Self::MissingParameter(name) => {
                let mut message = String::from("missing parameter `");
                message.push_str(&name);
                // Both pieces use the same proved UTF-8 append operation.
                #[allow(clippy::single_char_add_str)]
                message.push_str("`");
                message
            }
            Self::UnboundRole => "expression references an unbound role".to_owned(),
            Self::MissingAccount => "expression references a missing account".to_owned(),
            Self::AdditionOverflow => "addition overflow".to_owned(),
            Self::SubtractionUnderflow => "subtraction underflow".to_owned(),
            Self::MultiplicationOverflow => "multiplication overflow".to_owned(),
            Self::DivisionByZero => "division by zero".to_owned(),
        }
    }
}

pub(crate) fn evaluate_expression<AccountId, A, RateId, Role, N>(
    expression: &QuantityExpression<Role, A, N>,
    proposal: &Exchange<RateId, Role, AccountId, N>,
    accounts: &IndexMap<AccountId, Arc<Account<A, N>>>,
) -> Result<Quantity<N>, ExpressionError>
where
    AccountId: Eq + Hash,
    A: Eq + Hash,
    Role: Ord,
    N: QuantityScalar,
{
    match expression {
        QuantityExpression::Constant { value } => Ok(value.clone()),
        QuantityExpression::Units => Ok(proposal.units().clone()),
        QuantityExpression::Parameter { name } => proposal
            .parameters()
            .get(name)
            .cloned()
            .ok_or_else(|| ExpressionError::MissingParameter(name.clone())),
        QuantityExpression::Balance { role, asset } => {
            let account_id = proposal
                .bindings()
                .get(role)
                .ok_or(ExpressionError::UnboundRole)?;
            let account = accounts
                .get(account_id)
                .ok_or(ExpressionError::MissingAccount)?;
            Ok(account.balance(asset))
        }
        QuantityExpression::Add { left, right } => evaluate_binary(
            left,
            right,
            proposal,
            accounts,
            ExpressionError::AdditionOverflow,
            |left, right| left.checked_add(right),
        ),
        QuantityExpression::Subtract { left, right } => evaluate_binary(
            left,
            right,
            proposal,
            accounts,
            ExpressionError::SubtractionUnderflow,
            |left, right| left.checked_sub(right),
        ),
        QuantityExpression::Multiply { left, right } => evaluate_binary(
            left,
            right,
            proposal,
            accounts,
            ExpressionError::MultiplicationOverflow,
            |left, right| left.checked_mul(right),
        ),
        QuantityExpression::DivideFloor {
            numerator,
            denominator,
        } => evaluate_binary(
            numerator,
            denominator,
            proposal,
            accounts,
            ExpressionError::DivisionByZero,
            |left, right| left.checked_div(right),
        ),
        QuantityExpression::Minimum { left, right } => {
            let left = evaluate_expression(left, proposal, accounts)?;
            let right = evaluate_expression(right, proposal, accounts)?;
            Ok(std::cmp::min(left, right))
        }
        QuantityExpression::Maximum { left, right } => {
            let left = evaluate_expression(left, proposal, accounts)?;
            let right = evaluate_expression(right, proposal, accounts)?;
            Ok(std::cmp::max(left, right))
        }
    }
}

pub(crate) fn evaluate_condition<AccountId, A, RateId, Role, N>(
    condition: &RateCondition<Role, A, N>,
    proposal: &Exchange<RateId, Role, AccountId, N>,
    accounts: &IndexMap<AccountId, Arc<Account<A, N>>>,
) -> Result<bool, ExpressionError>
where
    AccountId: Eq + Hash,
    A: Eq + Hash,
    Role: Ord,
    N: QuantityScalar,
{
    let left = evaluate_expression(condition.left(), proposal, accounts)?;
    let right = evaluate_expression(condition.right(), proposal, accounts)?;
    Ok(match condition.comparison() {
        QuantityComparison::Equal => left == right,
        QuantityComparison::GreaterThanOrEqual => left >= right,
        QuantityComparison::LessThanOrEqual => left <= right,
    })
}

fn evaluate_binary<AccountId, A, RateId, Role, N>(
    left: &QuantityExpression<Role, A, N>,
    right: &QuantityExpression<Role, A, N>,
    proposal: &Exchange<RateId, Role, AccountId, N>,
    accounts: &IndexMap<AccountId, Arc<Account<A, N>>>,
    error: ExpressionError,
    operation: impl FnOnce(&Quantity<N>, &Quantity<N>) -> Option<Quantity<N>>,
) -> Result<Quantity<N>, ExpressionError>
where
    AccountId: Eq + Hash,
    A: Eq + Hash,
    Role: Ord,
    N: QuantityScalar,
{
    let left = evaluate_expression(left, proposal, accounts)?;
    let right = evaluate_expression(right, proposal, accounts)?;
    operation(&left, &right).ok_or(error)
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

fn relevant_balances<A, N>(holder: &Account<A, N>, required: &Basket<A, N>) -> Basket<A, N>
where
    A: Clone + Eq + Hash,
    N: QuantityScalar,
{
    let mut relevant = Basket::new();
    for (asset, _) in required.iter() {
        relevant.insert(asset.clone(), holder.balance(asset));
    }
    relevant
}

fn map_account_error<RateId, Role, AccountId, A, N>(
    holder: AccountId,
    error: AccountError<A, N>,
) -> ApplyError<RateId, Role, AccountId, A, N>
where
    A: Eq + Hash,
    N: QuantityScalar,
{
    match error {
        AccountError::InsufficientBalance { shortfall } => ApplyError::Infeasible {
            shortfalls: vec![AccountShortfall::new(holder, shortfall)],
        },
        AccountError::Overflow { asset } => ApplyError::BalanceOverflow {
            account: holder,
            asset,
        },
    }
}

#[cfg(test)]
mod evaluation_conformance {
    use super::*;

    // Exercise distinct semantic domains even in the fixture adapter.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    struct Holder(u64);
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    struct Asset(u64);
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    struct Role(u64);
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Rule;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct EvaluationCase {
        units: String,
        parameters: Vec<(String, String)>,
        bindings: Vec<(Role, Holder)>,
        accounts: Vec<(Holder, Vec<(Asset, String)>)>,
        expression: QuantityExpression<Role, Asset>,
        result: Option<String>,
        error: Option<String>,
        condition_right: QuantityExpression<Role, Asset>,
        conditions: Vec<(Option<bool>, Option<String>)>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct EffectsCase {
        target: Vec<(Asset, String)>,
        source: Vec<(Asset, String)>,
        units: String,
        scaled: Vec<(Asset, String)>,
        scale_error: Option<Asset>,
        asset: Asset,
        amount: String,
        computed: Vec<(Asset, String)>,
        computed_error: Option<Asset>,
    }

    #[derive(Deserialize)]
    struct Fixtures {
        evaluation: Vec<EvaluationCase>,
        effects: Vec<EffectsCase>,
    }

    fn amounts(entries: &[(Asset, String)]) -> Basket<Asset> {
        Basket::try_from_entries(
            entries
                .iter()
                .map(|(asset, amount)| (asset.clone(), Quantity::new(amount.parse().unwrap()))),
        )
        .unwrap()
    }

    fn entries(basket: &Basket<Asset>) -> Vec<(Asset, String)> {
        basket
            .iter()
            .map(|(asset, amount)| (asset.clone(), amount.as_scalar().to_string()))
            .collect()
    }

    #[test]
    fn runtime_evaluation_and_effects_match_lean() {
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert_eq!(fixtures.evaluation.len(), 2210);
        let mut condition_outcomes = [0; 3];
        for case in fixtures.evaluation {
            let mut proposal = Exchange::new(Rule, Quantity::new(case.units.parse().unwrap()));
            for (role, holder) in &case.bindings {
                proposal = proposal.bind(role.clone(), holder.clone());
            }
            for (parameter, amount) in &case.parameters {
                proposal = proposal.parameter(parameter, Quantity::new(amount.parse().unwrap()));
            }
            let accounts: IndexMap<_, _> = case
                .accounts
                .iter()
                .map(|(id, balances)| (id.clone(), Arc::new(Account::new(amounts(balances)))))
                .collect();
            let before = accounts.clone();
            let proposal_before = proposal.clone();
            let actual = evaluate_expression(&case.expression, &proposal, &accounts)
                .map(|value| value.into_scalar().to_string())
                .map_err(ExpressionError::into_message);
            let expected = match (&case.result, &case.error) {
                (Some(value), None) => Ok(value.clone()),
                (None, Some(error)) => Err(error.clone()),
                _ => panic!("invalid evaluation fixture: {case:?}"),
            };
            assert_eq!(actual, expected, "{case:?}");
            for (comparison, expected) in [
                QuantityComparison::Equal,
                QuantityComparison::GreaterThanOrEqual,
                QuantityComparison::LessThanOrEqual,
            ]
            .into_iter()
            .zip(&case.conditions)
            {
                let rule = Rate::new().condition(
                    "fixture",
                    case.expression.clone(),
                    comparison,
                    case.condition_right.clone(),
                );
                let actual = evaluate_condition(&rule.conditions()[0], &proposal, &accounts)
                    .map_err(ExpressionError::into_message);
                let expected = match expected {
                    (Some(value), None) => Ok(*value),
                    (None, Some(error)) => Err(error.clone()),
                    _ => panic!("invalid condition fixture: {case:?}"),
                };
                assert_eq!(actual, expected, "{case:?}, {comparison:?}");
                condition_outcomes[match actual {
                    Ok(false) => 0,
                    Ok(true) => 1,
                    Err(_) => 2,
                }] += 1;
            }
            assert_eq!(accounts, before);
            assert_eq!(proposal, proposal_before);
        }
        assert!(condition_outcomes.into_iter().all(|count| count > 0));

        assert_eq!(fixtures.effects.len(), 73);
        for case in fixtures.effects {
            let source = amounts(&case.source);
            let source_before = source.clone();
            let mut target = amounts(&case.target);
            let actual = merge_scaled(
                &mut target,
                &source,
                &Quantity::new(case.units.parse().unwrap()),
            );
            assert_eq!(actual.err(), case.scale_error, "{case:?}");
            assert_eq!(entries(&target), case.scaled, "{case:?}");
            assert_eq!(source, source_before);
            let mut target = amounts(&case.target);
            let actual = merge_quantity(
                &mut target,
                case.asset.clone(),
                Quantity::new(case.amount.parse().unwrap()),
            );
            assert_eq!(actual.err(), case.computed_error, "{case:?}");
            assert_eq!(entries(&target), case.computed, "{case:?}");
        }
    }
}

#[cfg(test)]
mod construction_conformance {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Holder(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Asset(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Rule(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Role(u64);
    type Builder = EconomyBuilder<Holder, Asset, Rule, Role>;

    #[derive(Debug, Deserialize)]
    struct Snapshot {
        accounts: Vec<(u64, u64)>,
        rates: Vec<(u64, u64)>,
        invariants: Vec<u64>,
        issues: Vec<(u64, u64)>,
        accepted: bool,
    }

    #[derive(Debug, Deserialize)]
    struct Case {
        steps: Vec<(u64, (u64, u64))>,
        snapshots: Vec<Snapshot>,
    }

    #[derive(Deserialize)]
    struct Fixtures {
        models: Vec<Case>,
    }

    fn balances(value: u64) -> Basket<Asset> {
        Basket::try_from_entries([(Asset(0), Quantity::new(value))]).unwrap()
    }

    fn expected_issues(snapshot: &Snapshot) -> Vec<ModelIssue<Holder, Rule>> {
        snapshot
            .issues
            .iter()
            .map(|&(kind, id)| match kind {
                0 => ModelIssue::DuplicateAccount {
                    account: Holder(id),
                },
                1 => ModelIssue::DuplicateRate { rate: Rule(id) },
                _ => panic!("unknown model issue"),
            })
            .collect()
    }

    #[test]
    fn runtime_model_construction_matches_lean() {
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert_eq!(fixtures.models.len(), 345);
        let mut outcomes = [0; 2];
        for case in fixtures.models {
            assert_eq!(case.snapshots.len(), case.steps.len() + 1);
            for default in [false, true] {
                // Build consumes its builder. Recreate each prefix to check every intermediate
                // accepted/rejected model as well as retained data after duplicate inputs.
                for (count, snapshot) in case.snapshots.iter().enumerate() {
                    let mut builder = if default {
                        Builder::default()
                    } else {
                        Builder::new()
                    };
                    for &(kind, (key, value)) in &case.steps[..count] {
                        builder = match kind {
                            0 => builder.account(Holder(key), Account::new(balances(value))),
                            1 => builder
                                .rate(Rule(key), Rate::new().produce(Role(0), balances(value))),
                            2 => builder.invariant(
                                LinearInvariant::new(value.to_string())
                                    .weight(Asset(0), -(value as i64)),
                            ),
                            _ => panic!("unknown model construction step"),
                        };
                    }
                    assert_eq!(
                        builder
                            .accounts
                            .iter()
                            .map(|(id, holder)| (
                                id.0,
                                *holder.balances().quantity(&Asset(0)).as_scalar()
                            ))
                            .collect::<Vec<_>>(),
                        snapshot.accounts
                    );
                    assert_eq!(
                        builder
                            .rates
                            .iter()
                            .map(|(id, rule)| (
                                id.0,
                                *rule
                                    .produced(&Role(0))
                                    .unwrap()
                                    .quantity(&Asset(0))
                                    .as_scalar()
                            ))
                            .collect::<Vec<_>>(),
                        snapshot.rates
                    );
                    assert_eq!(
                        builder
                            .invariants
                            .iter()
                            .map(|i| i.name().parse::<u64>().unwrap())
                            .collect::<Vec<_>>(),
                        snapshot.invariants
                    );
                    for (invariant, value) in builder.invariants.iter().zip(&snapshot.invariants) {
                        let expected = if *value == 0 {
                            vec![]
                        } else {
                            vec![(&Asset(0), -(*value as i64))]
                        };
                        assert_eq!(invariant.weights().collect::<Vec<_>>(), expected);
                    }
                    assert_eq!(builder.issues, expected_issues(snapshot));
                    let actual = builder.build();
                    assert_eq!(actual.is_ok(), snapshot.accepted);
                    outcomes[usize::from(actual.is_ok())] += 1;
                    match actual {
                        Ok(model) => {
                            assert_eq!(
                                model
                                    .accounts
                                    .iter()
                                    .map(|(id, holder)| (
                                        id.0,
                                        *holder.balances().quantity(&Asset(0)).as_scalar()
                                    ))
                                    .collect::<Vec<_>>(),
                                snapshot.accounts
                            );
                            assert_eq!(
                                model
                                    .rates
                                    .iter()
                                    .map(|(id, rule)| (
                                        id.0,
                                        *rule
                                            .produced(&Role(0))
                                            .unwrap()
                                            .quantity(&Asset(0))
                                            .as_scalar()
                                    ))
                                    .collect::<Vec<_>>(),
                                snapshot.rates
                            );
                            assert_eq!(
                                model
                                    .invariants
                                    .iter()
                                    .map(|i| i.name().parse::<u64>().unwrap())
                                    .collect::<Vec<_>>(),
                                snapshot.invariants
                            );
                        }
                        Err(error) => assert_eq!(error.issues(), expected_issues(snapshot)),
                    }
                }
            }
        }
        assert!(outcomes.into_iter().all(|count| count > 0));
    }
}

#[cfg(test)]
mod validation_conformance {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
    struct Holder(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
    struct Asset(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
    struct Role(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
    struct Rule(u64);
    type Error = ApplyError<Rule, Role, Holder, Asset>;

    #[derive(Debug, Deserialize)]
    struct Issue {
        kind: String,
        ids: Vec<u64>,
        parameter: String,
    }

    impl Issue {
        fn error(&self) -> Error {
            match self.kind.as_str() {
                "zeroUnits" => Error::ZeroUnits,
                "missingRate" => Error::MissingRate {
                    rate: Rule(self.ids[0]),
                },
                "missingBinding" => Error::MissingBinding {
                    role: Role(self.ids[0]),
                },
                "unknownBinding" => Error::UnknownBinding {
                    role: Role(self.ids[0]),
                },
                "missingParameter" => Error::MissingParameter {
                    name: self.parameter.clone(),
                },
                "unknownParameter" => Error::UnknownParameter {
                    name: self.parameter.clone(),
                },
                "rolesMustDiffer" => Error::RolesMustDiffer {
                    left: Role(self.ids[0]),
                    right: Role(self.ids[1]),
                },
                "missingAccount" => Error::MissingAccount {
                    account: Holder(self.ids[0]),
                },
                _ => panic!("unknown structural issue"),
            }
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        roles: Vec<Role>,
        required_parameters: Vec<String>,
        distinct: Vec<(Role, Role)>,
        units: String,
        rate: Rule,
        bindings: Vec<(Role, Holder)>,
        parameters: Vec<(String, String)>,
        accounts: Vec<Holder>,
        accepted: bool,
        issues: Vec<Issue>,
    }

    #[derive(Deserialize)]
    struct Fixtures {
        validation: Vec<Case>,
    }

    #[test]
    fn runtime_structural_validation_matches_lean() {
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert_eq!(fixtures.validation.len(), 765);
        let mut outcomes = [0; 2];
        let mut kinds = BTreeSet::new();
        for case in fixtures.validation {
            for trap in [false, true] {
                let mut rule = Rate::new();
                for role in &case.roles {
                    rule = rule.consume(*role, Basket::<Asset>::new());
                }
                for parameter in &case.required_parameters {
                    rule = rule.condition(
                        parameter.clone(),
                        QuantityExpression::parameter(parameter),
                        QuantityComparison::Equal,
                        QuantityExpression::parameter(parameter),
                    );
                }
                for (left, right) in &case.distinct {
                    rule = rule.distinct(*left, *right);
                }
                let fail_condition = |rule: Rate<Role, Asset>| {
                    if trap {
                        rule.condition(
                            "after validation",
                            QuantityExpression::constant(0),
                            QuantityComparison::Equal,
                            QuantityExpression::constant(1),
                        )
                    } else {
                        rule
                    }
                };
                let mut builder = EconomyBuilder::new()
                    .rate(Rule(0), fail_condition(rule))
                    .rate(Rule(2), fail_condition(Rate::new()));
                for id in &case.accounts {
                    builder = builder.account(
                        *id,
                        Account::new(
                            Basket::try_from_entries([(Asset(0), Quantity::new(5))]).unwrap(),
                        ),
                    );
                }
                let mut model = builder.build().unwrap();
                let mut proposal =
                    Exchange::new(case.rate, Quantity::new(case.units.parse().unwrap()));
                for (role, id) in &case.bindings {
                    proposal = proposal.bind(*role, *id);
                }
                for (parameter, value) in &case.parameters {
                    proposal = proposal.parameter(parameter, Quantity::new(value.parse().unwrap()));
                }
                let expected: Vec<Error> = case.issues.iter().map(Issue::error).collect();
                for issue in &case.issues {
                    kinds.insert(issue.kind.clone());
                }
                let before = serde_json::to_value(&model).unwrap();
                match model.validate_structure(&proposal) {
                    Ok(selected) => {
                        assert!(case.accepted, "{case:?}");
                        assert!(expected.is_empty());
                        assert!(std::ptr::eq(selected, model.rates.get(&case.rate).unwrap()));
                        outcomes[0] += 1;
                    }
                    Err(errors) => {
                        assert!(!case.accepted, "{case:?}");
                        assert!(!errors.is_empty());
                        assert_eq!(
                            serde_json::to_value(errors).unwrap(),
                            serde_json::to_value(&expected).unwrap(),
                            "{case:?}"
                        );
                        outcomes[1] += 1;
                    }
                }
                let assessment = model.assess(&proposal);
                let expected = if case.accepted && trap {
                    vec![Error::RateConditionFailed {
                        rate: case.rate,
                        condition: "after validation".into(),
                    }]
                } else {
                    expected
                };
                assert_eq!(
                    serde_json::to_value(assessment.issues()).unwrap(),
                    serde_json::to_value(&expected).unwrap(),
                    "{case:?}"
                );
                assert_eq!(serde_json::to_value(&model).unwrap(), before);
                let applied = model.apply(proposal);
                match expected.first() {
                    None => assert!(applied.is_ok(), "{case:?}"),
                    Some(first) => assert_eq!(
                        serde_json::to_value(applied.unwrap_err()).unwrap(),
                        serde_json::to_value(first).unwrap(),
                        "{case:?}"
                    ),
                }
                assert_eq!(serde_json::to_value(&model).unwrap(), before);
            }
        }
        assert!(outcomes.into_iter().all(|count| count > 0));
        assert_eq!(kinds.len(), 8);
    }
}
