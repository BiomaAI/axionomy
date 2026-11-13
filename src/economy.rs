use crate::Ontology;
use crate::accounts::Accounts;
use crate::rate::Measure;
use crate::{
    Account, AccountDelta, AccountError, Basket, ComputedAmount, ConditionPath, Exchange,
    InvariantPath, LinearInvariant, ParameterPath, Quantity, QuantityComparison,
    QuantityExpression, Rate, RateCondition, Receipt, Rounding, Trace,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Account: Serialize, O::Asset: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Account: JsonSchema, O::Asset: JsonSchema")]
pub struct Goal<O: Ontology> {
    required: BTreeMap<O::Account, Basket<O::Asset>>,
}

impl<O: Ontology> Goal<O> {
    pub fn new() -> Self {
        Self {
            required: BTreeMap::new(),
        }
    }

    pub fn require(mut self, account: O::Account, assets: Basket<O::Asset>) -> Self {
        self.required.insert(account, assets);
        self
    }

    pub fn requirements(&self) -> &BTreeMap<O::Account, Basket<O::Asset>> {
        &self.required
    }
}

impl<O: Ontology> Default for Goal<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Ontology> Goal<O> {
    /// Returns every asset identity referenced by this goal.
    pub fn asset_keys(&self) -> BTreeSet<&O::Asset> {
        self.required
            .values()
            .flat_map(|required| required.iter().map(|(asset, _)| asset))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Economy<O: Ontology> {
    accounts: Accounts<O::Account, O::Asset>,
    rates: Arc<IndexMap<O::Rate, Rate<O>>>,
    invariants: Arc<Vec<LinearInvariant<O::Asset>>>,
}

/// The logical state of an economy's accounts, independent of creation order:
/// which accounts exist and their nonzero balances.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateKey<O: Ontology> {
    accounts: Vec<O::Account>,
    balances: Vec<(O::Account, O::Asset, Quantity)>,
}

impl<O: Ontology> StateKey<O> {
    pub fn accounts(&self) -> &[O::Account] {
        &self.accounts
    }

    pub fn balances(&self) -> &[(O::Account, O::Asset, Quantity)] {
        &self.balances
    }
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

type StructuralValidationResult<'a, O> = Result<&'a Rate<O>, Vec<ApplyError<O>>>;

pub type ApplyResult<O> = Result<Receipt<O>, ApplyError<O>>;

pub type ReplayResult<O> = Result<Vec<Receipt<O>>, ReplayError<O>>;

pub type ModelBuildResult<O> = Result<Economy<O>, ModelBuildError<O>>;

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
    serialize = "O::Account: Serialize, O::Asset: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Account: JsonSchema, O::Asset: JsonSchema")]
pub struct AccountAssessment<O: Ontology> {
    account: O::Account,
    available: Basket<O::Asset>,
    required: Basket<O::Asset>,
    consumed: Basket<O::Asset>,
    produced: Basket<O::Asset>,
    preserved: Basket<O::Asset>,
}

impl<O: Ontology> AccountAssessment<O> {
    pub fn account(&self) -> &O::Account {
        &self.account
    }

    /// Balances relevant to this exchange's requirements.
    pub fn available(&self) -> &Basket<O::Asset> {
        &self.available
    }

    pub fn required(&self) -> &Basket<O::Asset> {
        &self.required
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

    fn new(
        account: O::Account,
        available: Basket<O::Asset>,
        required: Basket<O::Asset>,
        consumed: Basket<O::Asset>,
        produced: Basket<O::Asset>,
        preserved: Basket<O::Asset>,
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
    serialize = "O::Account: Serialize, O::Asset: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Account: JsonSchema, O::Asset: JsonSchema")]
pub struct AccountShortfall<O: Ontology> {
    account: O::Account,
    missing: Basket<O::Asset>,
}

impl<O: Ontology> AccountShortfall<O> {
    pub fn account(&self) -> &O::Account {
        &self.account
    }

    pub fn missing(&self) -> &Basket<O::Asset> {
        &self.missing
    }

    fn new(account: O::Account, missing: Basket<O::Asset>) -> Self {
        Self { account, missing }
    }
}

/// A non-mutating explanation of one proposed exchange.
#[must_use]
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Account: Serialize, O::Asset: Serialize, O::Rate: Serialize, O::Role: Serialize",
))]
#[schemars(
    bound = "O::Account: JsonSchema, O::Asset: JsonSchema, O::Rate: JsonSchema, O::Role: JsonSchema"
)]
pub enum ExchangeAssessment<O: Ontology> {
    /// Every check succeeded; projected deltas match a subsequent receipt if
    /// the economy remains unchanged.
    Applicable {
        accounts: Vec<AccountAssessment<O>>,
        projected_deltas: Vec<AccountDelta<O>>,
    },
    /// The proposal is structurally valid but one or more accounts lack assets.
    Infeasible {
        accounts: Vec<AccountAssessment<O>>,
        shortfalls: Vec<AccountShortfall<O>>,
    },
    /// The proposal is malformed, overflows, or violates an invariant.
    Invalid { issues: Vec<ApplyError<O>> },
}

impl<O: Ontology> ExchangeAssessment<O> {
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

    pub fn accounts(&self) -> &[AccountAssessment<O>] {
        match self {
            Self::Applicable { accounts, .. } | Self::Infeasible { accounts, .. } => accounts,
            Self::Invalid { .. } => &[],
        }
    }

    pub fn account(&self, account: &O::Account) -> Option<&AccountAssessment<O>> {
        self.accounts()
            .iter()
            .find(|assessment| assessment.account() == account)
    }

    pub fn shortfalls(&self) -> &[AccountShortfall<O>] {
        match self {
            Self::Infeasible { shortfalls, .. } => shortfalls,
            Self::Applicable { .. } | Self::Invalid { .. } => &[],
        }
    }

    pub fn shortfall(&self, account: &O::Account) -> Option<&Basket<O::Asset>> {
        self.shortfalls()
            .iter()
            .find(|shortfall| shortfall.account() == account)
            .map(AccountShortfall::missing)
    }

    pub fn projected_deltas(&self) -> Option<&[AccountDelta<O>]> {
        match self {
            Self::Applicable {
                projected_deltas, ..
            } => Some(projected_deltas),
            Self::Infeasible { .. } | Self::Invalid { .. } => None,
        }
    }

    pub fn issues(&self) -> &[ApplyError<O>] {
        match self {
            Self::Invalid { issues } => issues,
            Self::Applicable { .. } | Self::Infeasible { .. } => &[],
        }
    }
}

#[derive(Debug)]
pub struct EconomyBuilder<O: Ontology> {
    accounts: IndexMap<O::Account, Account<O::Asset>>,
    rates: IndexMap<O::Rate, Rate<O>>,
    invariants: Vec<LinearInvariant<O::Asset>>,
    issues: Vec<ModelIssue<O>>,
}

impl<O: Ontology> EconomyBuilder<O> {
    pub fn new() -> Self {
        Self {
            accounts: IndexMap::new(),
            rates: IndexMap::new(),
            invariants: Vec::new(),
            issues: Vec::new(),
        }
    }

    pub fn account(mut self, id: O::Account, account: Account<O::Asset>) -> Self {
        if self.accounts.contains_key(&id) {
            self.issues
                .push(ModelIssue::DuplicateAccount { account: id });
        } else {
            self.accounts.insert(id, account);
        }
        self
    }

    pub fn rate(mut self, id: O::Rate, rate: Rate<O>) -> Self {
        if self.rates.contains_key(&id) {
            self.issues.push(ModelIssue::DuplicateRate { rate: id });
        } else {
            self.rates.insert(id, rate);
        }
        self
    }

    pub fn invariant(mut self, invariant: LinearInvariant<O::Asset>) -> Self {
        self.invariants.push(invariant);
        self
    }

    /// The economy, its accounts and rates in the order of their
    /// identifiers, however they were added: an economy is the same economy
    /// whichever order it was written in, and reads the same once stored.
    pub fn build(mut self) -> ModelBuildResult<O> {
        if !self.issues.is_empty() {
            return Err(ModelBuildError {
                issues: self.issues,
            });
        }
        self.rates.sort_unstable_keys();
        Ok(Economy {
            accounts: Accounts::new(
                self.accounts
                    .into_iter()
                    .map(|(id, account)| (id, Arc::new(account))),
            ),
            rates: Arc::new(self.rates),
            invariants: Arc::new(self.invariants),
        })
    }
}

impl<O: Ontology> Default for EconomyBuilder<O> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "O::Account: Serialize, O::Rate: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Rate: Deserialize<'de>",
))]
pub enum ModelIssue<O: Ontology> {
    DuplicateAccount { account: O::Account },
    DuplicateRate { rate: O::Rate },
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(bound(
    serialize = "O::Account: Serialize, O::Rate: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Rate: Deserialize<'de>",
))]
#[error("model definition contains duplicate identifiers")]
pub struct ModelBuildError<O: Ontology> {
    issues: Vec<ModelIssue<O>>,
}

impl<O: Ontology> ModelBuildError<O> {
    pub fn issues(&self) -> &[ModelIssue<O>] {
        &self.issues
    }
}

impl<O: Ontology> Serialize for Economy<O>
where
    O::Account: Serialize,
    O::Asset: Serialize,
    O::Rate: Serialize,
    O::Role: Serialize,
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
    deserialize = "O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>, O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>",
))]
struct EconomyData<O: Ontology> {
    accounts: Vec<(O::Account, Account<O::Asset>)>,
    rates: Vec<(O::Rate, Rate<O>)>,
    invariants: Vec<LinearInvariant<O::Asset>>,
}

impl<'de, O: Ontology> Deserialize<'de> for Economy<O>
where
    O::Account: Deserialize<'de>,
    O::Asset: Deserialize<'de>,
    O::Rate: Deserialize<'de>,
    O::Role: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = EconomyData::<O>::deserialize(deserializer)?;
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

impl<O: Ontology> JsonSchema for Economy<O>
where
    O::Account: JsonSchema,
    O::Asset: JsonSchema,
    O::Rate: JsonSchema,
    O::Role: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!(
            "Economy_{}_{}_{}_{}",
            O::Account::schema_name(),
            O::Asset::schema_name(),
            O::Rate::schema_name(),
            O::Role::schema_name(),
        )
        .into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        #[expect(dead_code, reason = "describes the JSON shape; never built")]
        #[derive(JsonSchema)]
        #[schemars(
            bound = "O::Account: JsonSchema, O::Asset: JsonSchema, O::Rate: JsonSchema, O::Role: JsonSchema"
        )]
        struct EconomySchema<O: Ontology> {
            accounts: Vec<(O::Account, Account<O::Asset>)>,
            rates: Vec<(O::Rate, Rate<O>)>,
            invariants: Vec<LinearInvariant<O::Asset>>,
        }

        generator.subschema_for::<EconomySchema<O>>()
    }
}

impl<O: Ontology> Economy<O> {
    /// Account inspection cannot bypass exchange validation by mutating balances.
    ///
    /// ```compile_fail,E0596
    /// use axionomy::{basket, paths::Economy};
    /// fn unauthorized_write(world: &mut Economy) {
    ///     let account = world.account(&"vault".parse().unwrap()).unwrap();
    ///     account.deposit(&basket([("gold".parse().unwrap(), 1)])).unwrap();
    /// }
    /// ```
    pub fn account(&self, id: &O::Account) -> Option<&Account<O::Asset>> {
        self.accounts.get(id).map(Arc::as_ref)
    }

    /// The balance of `asset` in `account_id`, zero for a missing account.
    pub fn balance(&self, account_id: &O::Account, asset: &O::Asset) -> Quantity {
        self.account(account_id)
            .map_or(Quantity::ZERO, |account| account.balance(asset))
    }

    /// The accounts, in the order of their identifiers, however they were
    /// added or opened.
    pub fn accounts(&self) -> impl Iterator<Item = (&O::Account, &Account<O::Asset>)> {
        self.accounts
            .iter()
            .map(|(id, account)| (id, account.as_ref()))
    }

    pub fn rate(&self, id: &O::Rate) -> Option<&Rate<O>> {
        self.rates.get(id)
    }

    /// The rates' identifiers, in their order.
    pub fn rate_ids(&self) -> impl Iterator<Item = &O::Rate> {
        self.rates.keys()
    }

    /// Returns the model's declared conservation laws.
    ///
    /// Invariants remain immutable engine authority. This read-only iterator is
    /// useful to explain and inspect a closed model without duplicating its
    /// definition in presentation or transport layers.
    pub fn invariants(&self) -> impl Iterator<Item = &LinearInvariant<O::Asset>> {
        self.invariants.iter()
    }

    /// Returns every asset identity referenced by the closed model.
    ///
    /// The set covers initial balances, every rate effect and requirement,
    /// and declared invariant weights. It supports construction-time schema
    /// validation without giving validators mutation authority.
    pub fn asset_keys(&self) -> BTreeSet<&O::Asset> {
        let balances = self
            .accounts
            .values()
            .flat_map(|account| account.balances().iter().map(|(asset, _)| asset));
        let rates = self.rates.values().flat_map(Rate::asset_keys);
        let invariants = self
            .invariants
            .iter()
            .flat_map(|invariant| invariant.weights().map(|(asset, _)| asset));
        balances.chain(rates).chain(invariants).collect()
    }

    /// Whether every required account exists and holds its required assets.
    pub fn matches(&self, goal: &Goal<O>) -> bool {
        goal.requirements().iter().all(|(account_id, required)| {
            self.account(account_id)
                .is_some_and(|account| account.balances().contains(required))
        })
    }

    pub fn view(
        &self,
        visible_accounts: impl IntoIterator<Item = O::Account>,
    ) -> EconomicView<'_, O> {
        EconomicView {
            economy: self,
            visible: visible_accounts.into_iter().collect(),
        }
    }

    /// Which accounts exist and every nonzero balance, ignoring the order
    /// accounts and assets were created in. Accounts open and close, so an
    /// empty account and an absent one have different keys: only the first can
    /// be bound by an exchange.
    pub fn state_key(&self) -> StateKey<O> {
        let mut accounts = Vec::new();
        let mut balances = Vec::new();
        for (account_id, holder) in self.accounts.iter() {
            accounts.push(account_id.clone());
            for (asset, value) in holder.balances().iter() {
                balances.push((account_id.clone(), asset.clone(), *value));
            }
        }
        accounts.sort();
        balances.sort();
        StateKey { accounts, balances }
    }

    fn observation_key_for(&self, visible: &BTreeSet<O::Account>) -> ObservationKey<O> {
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
                balances.push((account_id.clone(), asset.clone(), *value));
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

    /// Explains one exchange without mutating the economy.
    pub fn assess(&self, proposal: &Exchange<O>) -> ExchangeAssessment<O> {
        self.analyze(proposal).analysis.assessment
    }

    /// Returns whether one exchange is applicable to the current snapshot.
    #[must_use]
    pub fn is_applicable(&self, proposal: &Exchange<O>) -> bool {
        self.assess(proposal).is_applicable()
    }

    /// Retains only candidates applicable to the current snapshot.
    #[must_use]
    pub fn applicable(
        &self,
        candidates: impl IntoIterator<Item = Exchange<O>>,
    ) -> Vec<Exchange<O>> {
        candidates
            .into_iter()
            .filter(|proposal| self.is_applicable(proposal))
            .collect()
    }

    pub fn apply(&mut self, proposal: Exchange<O>) -> ApplyResult<O> {
        let Lifecycle {
            analysis:
                Analysis {
                    assessment,
                    prepared_accounts,
                },
            extended,
            opened,
            closed,
        } = self.analyze(&proposal);

        match assessment {
            ExchangeAssessment::Applicable {
                projected_deltas, ..
            } => {
                if let Some(extended) = extended {
                    *self = extended;
                }
                for (account_id, holder) in
                    prepared_accounts.expect("applicable assessments have prepared accounts")
                {
                    self.accounts.replace(&account_id, holder);
                }
                for account in &closed {
                    self.accounts.close(account);
                }
                Ok(Receipt::new(proposal, projected_deltas, opened, closed))
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

    /// Applies a trace in order, all of it or none of it: returns the ordered
    /// receipts when every exchange applies, and otherwise changes nothing
    /// and names the exchange that failed.
    pub fn replay(&mut self, trace: &Trace<O>) -> ReplayResult<O> {
        let (next, receipts) = self.clone().stepped(trace)?;
        *self = next;
        Ok(receipts)
    }

    /// The economy after a trace, if every exchange applies; `self` is
    /// unchanged either way.
    pub fn replayed(&self, trace: &Trace<O>) -> Result<Self, ReplayError<O>> {
        self.clone().stepped(trace).map(|(economy, _)| economy)
    }

    fn stepped(mut self, trace: &Trace<O>) -> Result<(Self, Vec<Receipt<O>>), ReplayError<O>> {
        let receipts = trace
            .exchanges()
            .iter()
            .enumerate()
            .map(|(index, exchange)| {
                Economy::apply(&mut self, exchange.clone())
                    .map_err(|error| ReplayError { index, error })
            })
            .collect::<Result<_, _>>()?;
        Ok((self, receipts))
    }
}

/// Where exchanges are applied: an economy itself, or something that keeps
/// one and records what it applies, such as a store's recorder. Code that
/// steps an economy (a rollout, a simulation, a policy) takes `&mut impl
/// Apply<O>`, so its caller decides whether the steps are kept. Whatever
/// implements it, the economy decides what every exchange does.
pub trait Apply<O: Ontology> {
    /// The state the next exchange applies to.
    fn economy(&self) -> &Economy<O>;

    /// Applies one exchange atomically; a failure changes nothing.
    fn apply(&mut self, exchange: Exchange<O>) -> ApplyResult<O>;
}

impl<O: Ontology> Apply<O> for Economy<O> {
    fn economy(&self) -> &Economy<O> {
        self
    }

    fn apply(&mut self, exchange: Exchange<O>) -> ApplyResult<O> {
        Economy::apply(self, exchange)
    }
}

impl<O: Ontology> Economy<O> {
    pub(crate) fn validate_structure(
        &self,
        proposal: &Exchange<O>,
    ) -> StructuralValidationResult<'_, O> {
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
                issues.push(ApplyError::MissingParameter { name: name.clone() });
            }
        }
        for name in proposal.parameters().keys() {
            if !parameter_names.contains(name) {
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
                && !self.accounts.contains(holder)
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
        amounts: &[ComputedAmount<O>],
        direction: ComputedDirection,
        proposal: &Exchange<O>,
        effects: &mut BTreeMap<O::Account, Effect<O::Asset>>,
    ) -> Result<(), ApplyError<O>> {
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
        prepared_accounts: &PreparedAccounts<O>,
    ) -> Accounts<O::Account, O::Asset> {
        let mut after_accounts = self.accounts.clone();
        for (account_id, holder) in prepared_accounts {
            after_accounts.replace(account_id, holder.clone());
        }
        after_accounts
    }

    fn check_conditions(
        &self,
        rule: &Rate<O>,
        proposal: &Exchange<O>,
    ) -> Result<(), ApplyError<O>> {
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
                    condition: condition.name().clone(),
                });
            }
        }

        Ok(())
    }

    fn aggregate_effects(&self, rule: &Rate<O>, proposal: &Exchange<O>) -> AggregationResult<O> {
        let mut effects: BTreeMap<O::Account, Effect<O::Asset>> = BTreeMap::new();
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
        proposal: &Exchange<O>,
        effects: &BTreeMap<O::Account, Effect<O::Asset>>,
    ) -> AvailabilityResult<O> {
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
        effects: BTreeMap<O::Account, Effect<O::Asset>>,
    ) -> PreparationResult<O> {
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
        deltas: &[AccountDelta<O>],
        prepared_accounts: &PreparedAccounts<O>,
    ) -> Result<(), ApplyError<O>> {
        let Some(violated) = self
            .invariants
            .iter()
            .find(|invariant| !invariant.conserves(deltas))
        else {
            return Ok(());
        };
        let after = self.accounts_with_prepared(prepared_accounts);
        Err(ApplyError::InvariantViolation {
            invariant: violated.name().clone(),
            before: Box::new(violated.measure(self.accounts.values().map(Arc::as_ref))),
            after: Box::new(violated.measure(after.values().map(Arc::as_ref))),
        })
    }

    /// The exchange analyzed with its accounts' lifecycle: an opened account
    /// must be absent and bound by no other role, and exists, empty, before the
    /// effects; a closed account must be left empty by them.
    fn analyze(&self, proposal: &Exchange<O>) -> Lifecycle<Self, O> {
        let unchanged = |analysis| Lifecycle {
            analysis,
            extended: None,
            opened: Vec::new(),
            closed: Vec::new(),
        };
        let Some(rule) = self.rates.get(proposal.rate()) else {
            return unchanged(self.analyze_transition(proposal));
        };
        let bindings = proposal.bindings();
        let opening: Vec<_> = rule
            .opened_roles()
            .filter_map(|role| {
                let account = bindings.get(role)?;
                if self.accounts.contains(account) {
                    Some(ApplyError::AccountExists {
                        account: account.clone(),
                    })
                } else if bindings
                    .iter()
                    .any(|(other, bound)| other != role && bound == account)
                {
                    Some(ApplyError::OpeningShared { role: role.clone() })
                } else {
                    None
                }
            })
            .collect();
        if !opening.is_empty() {
            return unchanged(invalid_analysis(opening));
        }
        let bound = |roles: &mut dyn Iterator<Item = &O::Role>| -> Vec<O::Account> {
            roles
                .filter_map(|role| bindings.get(role).cloned())
                .collect()
        };
        let opened = bound(&mut rule.opened_roles());
        let closed = bound(&mut rule.closed_roles());
        let extended = (!opened.is_empty()).then(|| {
            let mut extended = self.clone();
            for account in &opened {
                extended.accounts.open(account.clone(), Arc::default());
            }
            extended
        });
        let economy = extended.as_ref().unwrap_or(self);
        let mut analysis = economy.analyze_transition(proposal);
        let closing: Vec<_> = match (&analysis.assessment, &analysis.prepared_accounts) {
            (ExchangeAssessment::Applicable { .. }, Some(prepared)) => closed
                .iter()
                .filter_map(|account| {
                    let remaining = prepared
                        .iter()
                        .find(|(id, _)| id == account)
                        .map(|(_, holder)| holder.balances())
                        .or_else(|| {
                            economy
                                .accounts
                                .get(account)
                                .map(|holder| holder.balances())
                        })?;
                    (!remaining.is_empty()).then(|| ApplyError::ClosedNotEmpty {
                        account: account.clone(),
                        remaining: remaining.clone(),
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        if !closing.is_empty() {
            analysis = invalid_analysis(closing);
        }
        Lifecycle {
            analysis,
            extended,
            opened,
            closed,
        }
    }

    /// The exchange analyzed against this economy's accounts as they are.
    fn analyze_transition(&self, proposal: &Exchange<O>) -> Analysis<O> {
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

pub struct EconomicView<'a, O: Ontology> {
    economy: &'a Economy<O>,
    visible: BTreeSet<O::Account>,
}

/// Canonical actor-visible balances, account existence, and visibility boundary.
/// Produced keys ignore storage insertion order. Serialized keys must include
/// `existing_accounts`; older keys cannot recover this information from balances.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Account: Serialize, O::Asset: Serialize",
    deserialize = "O::Account: Deserialize<'de>, O::Asset: Deserialize<'de>",
))]
#[schemars(bound = "O::Account: JsonSchema, O::Asset: JsonSchema")]
pub struct ObservationKey<O: Ontology> {
    visible_accounts: Vec<O::Account>,
    existing_accounts: Vec<O::Account>,
    balances: Vec<(O::Account, O::Asset, Quantity)>,
}

impl<O: Ontology> ObservationKey<O> {
    pub fn visible_accounts(&self) -> &[O::Account] {
        &self.visible_accounts
    }

    /// Visible accounts present in the economy, including empty accounts.
    pub fn existing_accounts(&self) -> &[O::Account] {
        &self.existing_accounts
    }

    pub fn balances(&self) -> &[(O::Account, O::Asset, Quantity)] {
        &self.balances
    }
}

impl<O: Ontology> EconomicView<'_, O> {
    pub fn account(&self, id: &O::Account) -> Option<&Account<O::Asset>> {
        if self.visible.contains(id) {
            self.economy.account(id)
        } else {
            None
        }
    }

    pub fn balance(&self, account_id: &O::Account, asset: &O::Asset) -> Option<Quantity> {
        self.account(account_id).map(|holder| holder.balance(asset))
    }

    /// Returns a canonical identity containing only visible economic state.
    ///
    /// Equal produced keys have the same visibility boundary, visible account
    /// existence, and balance-query results, including missing versus zero.
    /// They ignore storage insertion order and all hidden accounts.
    pub fn observation_key(&self) -> ObservationKey<O> {
        self.economy.observation_key_for(&self.visible)
    }
}

/// A trace that did not apply: the position of the exchange that failed,
/// counted from zero, and why. A failed replay changes nothing.
#[derive(Debug, Clone, Error, Serialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Rate: Serialize, O::Role: Serialize, O::Account: Serialize, O::Asset: Serialize",
))]
#[schemars(
    bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema, O::Asset: JsonSchema"
)]
#[error("exchange {index}: {error}")]
pub struct ReplayError<O: Ontology> {
    pub index: usize,
    pub error: ApplyError<O>,
}

#[derive(Debug, Clone, Error, Serialize, JsonSchema)]
#[serde(bound(
    serialize = "O::Rate: Serialize, O::Role: Serialize, O::Account: Serialize, O::Asset: Serialize",
))]
#[schemars(
    bound = "O::Rate: JsonSchema, O::Role: JsonSchema, O::Account: JsonSchema, O::Asset: JsonSchema"
)]
pub enum ApplyError<O: Ontology> {
    #[error("rate does not exist")]
    MissingRate { rate: O::Rate },
    #[error("exchange is missing a role binding")]
    MissingBinding { role: O::Role },
    #[error("exchange contains an unknown role")]
    UnknownBinding { role: O::Role },
    #[error("rate roles must bind to different accounts")]
    RolesMustDiffer { left: O::Role, right: O::Role },
    #[error("bound account does not exist")]
    MissingAccount { account: O::Account },
    #[error("exchange units must be greater than zero")]
    ZeroUnits,
    #[error("exchange is missing a required rate parameter")]
    MissingParameter { name: ParameterPath },
    #[error("exchange contains an unknown rate parameter")]
    UnknownParameter { name: ParameterPath },
    #[error("scaled rate overflow")]
    RateOverflow { rate: O::Rate, asset: O::Asset },
    #[error("state-evaluated rate law could not be evaluated: {message}")]
    RateLawEvaluation { rate: O::Rate, message: String },
    #[error("state-evaluated rate condition failed")]
    RateConditionFailed {
        rate: O::Rate,
        condition: ConditionPath,
    },
    #[error("exchange is infeasible")]
    Infeasible {
        shortfalls: Vec<AccountShortfall<O>>,
    },
    #[error("account balance overflow")]
    BalanceOverflow {
        account: O::Account,
        asset: O::Asset,
    },
    #[error("declared invariant violation")]
    InvariantViolation {
        invariant: InvariantPath,
        before: Box<Measure>,
        after: Box<Measure>,
    },
    #[error("an account the exchange opens already exists")]
    AccountExists { account: O::Account },
    #[error("another role binds the account an opening role creates")]
    OpeningShared { role: O::Role },
    #[error("the exchange leaves assets in an account it closes")]
    ClosedNotEmpty {
        account: O::Account,
        remaining: Basket<O::Asset>,
    },
}

#[derive(Debug, Clone)]
struct Effect<A> {
    consume: Basket<A>,
    produce: Basket<A>,
    preserve: Basket<A>,
}

#[derive(Debug, Clone, Copy)]
enum ComputedDirection {
    Consume,
    Produce,
    Preserve,
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

type PreparedAccounts<O> = Vec<(
    <O as Ontology>::Account,
    Arc<Account<<O as Ontology>::Asset>>,
)>;

type AggregationResult<O> =
    Result<BTreeMap<<O as Ontology>::Account, Effect<<O as Ontology>::Asset>>, ApplyError<O>>;

type AvailabilityResult<O> = Result<Availability<O>, ApplyError<O>>;

type PreparationResult<O> = Result<Preparation<O>, ApplyError<O>>;

struct Availability<O: Ontology> {
    accounts: Vec<AccountAssessment<O>>,
    shortfalls: Vec<AccountShortfall<O>>,
}

struct Preparation<O: Ontology> {
    accounts: PreparedAccounts<O>,
    deltas: Vec<AccountDelta<O>>,
}

impl<A: Clone + Eq + Hash> Effect<A> {
    fn merge_computed(
        &mut self,
        direction: ComputedDirection,
        asset: A,
        value: Quantity,
    ) -> Result<(), A> {
        match direction {
            ComputedDirection::Consume => merge_quantity(&mut self.consume, asset, value),
            ComputedDirection::Produce => merge_quantity(&mut self.produce, asset, value),
            ComputedDirection::Preserve => merge_quantity(&mut self.preserve, asset, value),
        }
    }
}

struct Analysis<O: Ontology> {
    assessment: ExchangeAssessment<O>,
    prepared_accounts: Option<PreparedAccounts<O>>,
}

/// An analysis together with what the exchange does to the set of accounts.
struct Lifecycle<E, O: Ontology> {
    analysis: Analysis<O>,
    /// The economy with the exchange's opened accounts added, when it opens any.
    extended: Option<E>,
    opened: Vec<O::Account>,
    closed: Vec<O::Account>,
}

pub(crate) fn merge_scaled<A>(
    target: &mut Basket<A>,
    source: &Basket<A>,
    units: Quantity,
) -> Result<(), A>
where
    A: Clone + Eq + Hash,
{
    let scaled = source.checked_scale(units)?;
    target.checked_add(&scaled)
}

pub(crate) fn merge_quantity<A>(target: &mut Basket<A>, asset: A, amount: Quantity) -> Result<(), A>
where
    A: Clone + Eq + Hash,
{
    let Some(total) = target.quantity(&asset).checked_add(amount) else {
        return Err(asset);
    };
    target.insert(asset, total);
    Ok(())
}

/// Typed failures inside the evaluator; text is produced only for public diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExpressionError {
    MissingParameter(ParameterPath),
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
            Self::MissingParameter(name) => format!("missing parameter `{name}`"),
            Self::UnboundRole => "expression references an unbound role".to_owned(),
            Self::MissingAccount => "expression references a missing account".to_owned(),
            Self::AdditionOverflow => "addition overflow".to_owned(),
            Self::SubtractionUnderflow => "subtraction underflow".to_owned(),
            Self::MultiplicationOverflow => "multiplication overflow".to_owned(),
            Self::DivisionByZero => "division by zero".to_owned(),
        }
    }
}

pub(crate) fn evaluate_expression<O: Ontology>(
    expression: &QuantityExpression<O>,
    proposal: &Exchange<O>,
    accounts: &Accounts<O::Account, O::Asset>,
) -> Result<Quantity, ExpressionError> {
    match expression {
        QuantityExpression::Constant { value } => Ok(*value),
        QuantityExpression::Units => Ok(proposal.units()),
        QuantityExpression::Parameter { name } => proposal
            .parameters()
            .get(name)
            .copied()
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
        QuantityExpression::MulDiv {
            left,
            right,
            denominator,
            rounding,
        } => evaluate_mul_div(left, right, denominator, *rounding, proposal, accounts),
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

pub(crate) fn evaluate_condition<O: Ontology>(
    condition: &RateCondition<O>,
    proposal: &Exchange<O>,
    accounts: &Accounts<O::Account, O::Asset>,
) -> Result<bool, ExpressionError> {
    let left = evaluate_expression(condition.left(), proposal, accounts)?;
    let right = evaluate_expression(condition.right(), proposal, accounts)?;
    Ok(match condition.comparison() {
        QuantityComparison::Equal => left == right,
        QuantityComparison::GreaterThanOrEqual => left >= right,
        QuantityComparison::LessThanOrEqual => left <= right,
    })
}

fn evaluate_binary<O: Ontology>(
    left: &QuantityExpression<O>,
    right: &QuantityExpression<O>,
    proposal: &Exchange<O>,
    accounts: &Accounts<O::Account, O::Asset>,
    error: ExpressionError,
    operation: impl FnOnce(Quantity, Quantity) -> Option<Quantity>,
) -> Result<Quantity, ExpressionError> {
    let left = evaluate_expression(left, proposal, accounts)?;
    let right = evaluate_expression(right, proposal, accounts)?;
    operation(left, right).ok_or(error)
}

// Operands are evaluated left to right against the same snapshot. A zero
// denominator is reported before the full-width quotient is attempted; a
// quotient outside the backend reports multiplication overflow.
fn evaluate_mul_div<O: Ontology>(
    left: &QuantityExpression<O>,
    right: &QuantityExpression<O>,
    denominator: &QuantityExpression<O>,
    rounding: Rounding,
    proposal: &Exchange<O>,
    accounts: &Accounts<O::Account, O::Asset>,
) -> Result<Quantity, ExpressionError> {
    let left = evaluate_expression(left, proposal, accounts)?;
    let right = evaluate_expression(right, proposal, accounts)?;
    let denominator = evaluate_expression(denominator, proposal, accounts)?;
    if denominator.is_zero() {
        return Err(ExpressionError::DivisionByZero);
    }
    left.checked_mul_div(right, denominator, rounding)
        .ok_or(ExpressionError::MultiplicationOverflow)
}

fn invalid_analysis<O: Ontology>(issues: Vec<ApplyError<O>>) -> Analysis<O> {
    debug_assert!(!issues.is_empty());
    Analysis {
        assessment: ExchangeAssessment::Invalid { issues },
        prepared_accounts: None,
    }
}

fn relevant_balances<A>(holder: &Account<A>, required: &Basket<A>) -> Basket<A>
where
    A: Clone + Eq + Hash,
{
    required
        .iter()
        .map(|(asset, _)| (asset.clone(), holder.balance(asset)))
        .collect()
}

fn map_account_error<O: Ontology>(
    holder: O::Account,
    error: AccountError<O::Asset>,
) -> ApplyError<O> {
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
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Rule;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Test {}

    impl Ontology for Test {
        const NAME: &'static str = "test";
        type Account = Holder;
        type Asset = Asset;
        type Rate = Rule;
        type Role = Role;
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct EvaluationCase {
        units: String,
        parameters: Vec<(ParameterPath, String)>,
        bindings: Vec<(Role, Holder)>,
        accounts: Vec<(Holder, Vec<(Asset, String)>)>,
        expression: QuantityExpression<Test>,
        result: Option<String>,
        error: Option<String>,
        condition_right: QuantityExpression<Test>,
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
                .map(|(asset, amount)| (asset.clone(), amount.parse::<Quantity>().unwrap())),
        )
        .unwrap()
    }

    fn entries(basket: &Basket<Asset>) -> Vec<(Asset, String)> {
        basket
            .iter()
            .map(|(asset, amount)| (asset.clone(), amount.to_string()))
            .collect()
    }

    #[test]
    fn runtime_evaluation_and_effects_match_lean() {
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
        assert!(!fixtures.evaluation.is_empty());
        let mut condition_outcomes = [0; 3];
        for case in fixtures.evaluation {
            let mut proposal = Exchange::new(Rule, case.units.parse::<Quantity>().unwrap());
            for (role, holder) in &case.bindings {
                proposal = proposal.bind(role.clone(), holder.clone());
            }
            for (parameter, amount) in &case.parameters {
                proposal =
                    proposal.parameter(parameter.clone(), amount.parse::<Quantity>().unwrap());
            }
            let accounts = Accounts::new(
                case.accounts
                    .iter()
                    .map(|(id, balances)| (id.clone(), Arc::new(Account::new(amounts(balances))))),
            );
            let before = accounts.clone();
            let proposal_before = proposal.clone();
            let actual = evaluate_expression(&case.expression, &proposal, &accounts)
                .map(|value| value.to_string())
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
                    crate::path!("fixture"),
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
            assert!(accounts.iter().eq(before.iter()));
            assert_eq!(proposal, proposal_before);
        }
        assert!(condition_outcomes.into_iter().all(|count| count > 0));

        assert!(!fixtures.effects.is_empty());
        for case in fixtures.effects {
            let source = amounts(&case.source);
            let source_before = source.clone();
            let mut target = amounts(&case.target);
            let actual = merge_scaled(
                &mut target,
                &source,
                case.units.parse::<Quantity>().unwrap(),
            );
            assert_eq!(actual.err(), case.scale_error, "{case:?}");
            assert_eq!(entries(&target), case.scaled, "{case:?}");
            assert_eq!(source, source_before);
            let mut target = amounts(&case.target);
            let actual = merge_quantity(
                &mut target,
                case.asset.clone(),
                case.amount.parse::<Quantity>().unwrap(),
            );
            assert_eq!(actual.err(), case.computed_error, "{case:?}");
            assert_eq!(entries(&target), case.computed, "{case:?}");
        }
    }
}

#[cfg(test)]
mod construction_conformance {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Holder(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Asset(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Rule(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Role(u64);
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Test {}

    impl Ontology for Test {
        const NAME: &'static str = "test";
        type Account = Holder;
        type Asset = Asset;
        type Rate = Rule;
        type Role = Role;
    }

    type Builder = EconomyBuilder<Test>;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Snapshot {
        accounts: Vec<(u64, u64)>,
        rates: Vec<(u64, u64)>,
        invariants: Vec<u64>,
        issues: Vec<(u64, u64)>,
        accepted: bool,
        /// The built model's accounts and rates, in their identifiers' order.
        built_accounts: Vec<(u64, u64)>,
        built_rates: Vec<(u64, u64)>,
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

    fn expected_issues(snapshot: &Snapshot) -> Vec<ModelIssue<Test>> {
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
        assert!(!fixtures.models.is_empty());
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
                                LinearInvariant::new(value.to_string().parse().unwrap())
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
                                holder.balances().quantity(&Asset(0)).to_u64()
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
                                rule.produced(&Role(0))
                                    .unwrap()
                                    .quantity(&Asset(0))
                                    .to_u64()
                            ))
                            .collect::<Vec<_>>(),
                        snapshot.rates
                    );
                    assert_eq!(
                        builder
                            .invariants
                            .iter()
                            .map(|i| i.name().as_str().parse::<u64>().unwrap())
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
                                        holder.balances().quantity(&Asset(0)).to_u64()
                                    ))
                                    .collect::<Vec<_>>(),
                                snapshot.built_accounts
                            );
                            assert_eq!(
                                model
                                    .rates
                                    .iter()
                                    .map(|(id, rule)| (
                                        id.0,
                                        rule.produced(&Role(0))
                                            .unwrap()
                                            .quantity(&Asset(0))
                                            .to_u64()
                                    ))
                                    .collect::<Vec<_>>(),
                                snapshot.built_rates
                            );
                            assert_eq!(
                                model
                                    .invariants
                                    .iter()
                                    .map(|i| i.name().as_str().parse::<u64>().unwrap())
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Test {}

    impl Ontology for Test {
        const NAME: &'static str = "test";
        type Account = Holder;
        type Asset = Asset;
        type Rate = Rule;
        type Role = Role;
    }

    type Error = ApplyError<Test>;

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
                    name: self.parameter.parse().unwrap(),
                },
                "unknownParameter" => Error::UnknownParameter {
                    name: self.parameter.parse().unwrap(),
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
        required_parameters: Vec<ParameterPath>,
        distinct: Vec<(Role, Role)>,
        units: String,
        rate: Rule,
        bindings: Vec<(Role, Holder)>,
        parameters: Vec<(ParameterPath, String)>,
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
        assert!(!fixtures.validation.is_empty());
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
                        parameter.as_str().parse().unwrap(),
                        QuantityExpression::parameter(parameter.clone()),
                        QuantityComparison::Equal,
                        QuantityExpression::parameter(parameter.clone()),
                    );
                }
                for (left, right) in &case.distinct {
                    rule = rule.distinct(*left, *right);
                }
                let fail_condition = |rule: Rate<Test>| {
                    if trap {
                        rule.condition(
                            crate::path!("after-validation"),
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
                    Exchange::new(case.rate, case.units.parse::<Quantity>().unwrap());
                for (role, id) in &case.bindings {
                    proposal = proposal.bind(*role, *id);
                }
                for (parameter, value) in &case.parameters {
                    proposal =
                        proposal.parameter(parameter.clone(), value.parse::<Quantity>().unwrap());
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
                        condition: crate::path!("after-validation"),
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
