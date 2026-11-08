//! Extract the production account update paths from shared source.
#[path = "../src/amount.rs"]
mod amount;
#[path = "../src/quantity.rs"]
mod quantity;
use amount::AssetAmount;
use quantity::{Quantity, QuantityScalar, Rounding};
#[path = "../src/basket.rs"]
mod basket;
use basket::Basket;
#[path = "../src/account.rs"]
mod account;
use account::{Account, AccountError};
#[allow(dead_code)]
#[path = "../src/exchange.rs"]
pub mod exchange;
use exchange::{AccountDelta, Exchange, Receipt, Trace};
#[path = "../src/rate.rs"]
pub mod rate;
use rate::{
    ComputedAmount, LinearInvariant, QuantityComparison, QuantityExpression, Rate, RateCondition,
    RateError,
};
#[path = "../src/economy.rs"]
pub mod economy;

pub fn deposit<A: Clone + Eq + std::hash::Hash>(
    state: &mut Account<A>,
    assets: &Basket<A>,
) -> Result<(), AccountError<A>> {
    state.deposit(assets)
}

pub fn withdraw<A: Clone + Eq + std::hash::Hash>(
    state: &mut Account<A>,
    assets: &Basket<A>,
) -> Result<(), AccountError<A>> {
    state.withdraw(assets)
}

pub fn basket_try_entries<A: Eq + std::hash::Hash, N: QuantityScalar, I>(
    entries: I,
) -> Result<Basket<A, N>, basket::BasketError<A>>
where
    I: IntoIterator<Item = (A, Quantity<N>)>,
{
    Basket::try_from_entries(entries)
}

pub fn basket_try_amounts<A: Eq + std::hash::Hash, N: QuantityScalar, I>(
    amounts: I,
) -> Result<Basket<A, N>, basket::BasketError<A>>
where
    I: IntoIterator<Item = AssetAmount<A, N>>,
{
    Basket::try_from_amounts(amounts)
}

pub fn basket_collect<A: Eq + std::hash::Hash, N: QuantityScalar, I>(entries: I) -> Basket<A, N>
where
    I: IntoIterator<Item = (A, Quantity<N>)>,
{
    Basket::from_iter(entries)
}

pub fn basket_from_array<A: Eq + std::hash::Hash, N: QuantityScalar, const LEN: usize>(
    entries: [(A, Quantity<N>); LEN],
) -> Basket<A, N> {
    Basket::from(entries)
}

pub fn basket_new<A, N>() -> Basket<A, N> {
    Basket::new()
}

pub fn basket_default<A, N>() -> Basket<A, N> {
    Basket::default()
}

pub fn basket_len<A, N>(state: &Basket<A, N>) -> usize {
    state.len()
}

pub fn basket_empty<A, N>(state: &Basket<A, N>) -> bool {
    state.is_empty()
}

pub fn basket_entries<A, N>(state: &Basket<A, N>) -> impl Iterator<Item = (&A, &Quantity<N>)> {
    state.iter()
}

pub fn basket_sorted_entries<A: Ord, N>(
    state: &Basket<A, N>,
) -> impl Iterator<Item = (&A, &Quantity<N>)> {
    state.iter_sorted()
}

pub fn basket_get<'a, A: Eq + std::hash::Hash, N: QuantityScalar>(
    state: &'a Basket<A, N>,
    asset: &A,
) -> Option<&'a Quantity<N>> {
    state.get(asset)
}

fn main() {}

pub fn basket_add<A: Clone + Eq + std::hash::Hash>(
    state: &mut Basket<A>,
    assets: &Basket<A>,
) -> Result<(), A> {
    state.checked_add(assets)
}

pub fn basket_scale<A: Clone + Eq + std::hash::Hash>(
    state: &Basket<A>,
    units: &Quantity,
) -> Result<Basket<A>, A> {
    state.checked_scale(units)
}

pub fn basket_contains<A: Clone + Eq + std::hash::Hash>(
    state: &Basket<A>,
    required: &Basket<A>,
) -> bool {
    state.contains(required)
}

pub fn basket_equal<A: Eq + std::hash::Hash>(left: &Basket<A>, right: &Basket<A>) -> bool {
    left == right
}

pub fn prepare_exchange<A: Clone + Eq + std::hash::Hash>(
    state: &Account<A>,
    consumed: &Basket<A>,
    produced: &Basket<A>,
) -> Result<Account<A>, AccountError<A>> {
    state.prepare_exchange(consumed, produced)
}

pub fn weighted(value: &u64, coefficient: i64) -> Option<i128> {
    value.checked_weighted(coefficient)
}

pub fn invariant_conserves<AccountId, A: Eq + std::hash::Hash>(
    invariant: &LinearInvariant<A>,
    deltas: &[AccountDelta<AccountId, A>],
) -> Option<bool> {
    invariant.conserves(deltas)
}

pub fn rate_consume<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    assets: Basket<A>,
) -> Result<Rate<Role, A>, RateError<Role, A>> {
    rule.try_consume(role, assets)
}

pub fn rate_produce<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    assets: Basket<A>,
) -> Result<Rate<Role, A>, RateError<Role, A>> {
    rule.try_produce(role, assets)
}

pub fn rate_preserve<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    assets: Basket<A>,
) -> Result<Rate<Role, A>, RateError<Role, A>> {
    rule.try_preserve(role, assets)
}

#[allow(dead_code)]
pub(crate) fn evaluate<
    AccountId: Eq + std::hash::Hash,
    A: Eq + std::hash::Hash,
    RateId,
    Role: Ord,
>(
    expression: &QuantityExpression<Role, A>,
    exchange: &Exchange<RateId, Role, AccountId>,
    accounts: &indexmap::IndexMap<AccountId, std::sync::Arc<Account<A>>>,
) -> Result<Quantity, economy::ExpressionError> {
    economy::evaluate_expression(expression, exchange, accounts)
}

pub fn merge_effects<A: Clone + Eq + std::hash::Hash>(
    target: &mut Basket<A>,
    source: &Basket<A>,
    units: &Quantity,
) -> Result<(), A> {
    economy::merge_scaled(target, source, units)
}

pub fn merge_amount<A: Clone + Eq + std::hash::Hash>(
    target: &mut Basket<A>,
    asset: A,
    amount: Quantity,
) -> Result<(), A> {
    economy::merge_quantity(target, asset, amount)
}

pub fn invariant_measure<AccountId, A: Clone + Eq + std::hash::Hash>(
    invariant: &LinearInvariant<A>,
    accounts: &indexmap::IndexMap<AccountId, std::sync::Arc<Account<A>>>,
) -> Option<i128> {
    invariant.measure(accounts)
}

#[allow(dead_code)]
pub(crate) fn expression_message(error: economy::ExpressionError) -> String {
    error.into_message()
}

#[allow(dead_code)]
pub(crate) fn evaluate_condition<
    AccountId: Eq + std::hash::Hash,
    A: Eq + std::hash::Hash,
    RateId,
    Role: Ord,
>(
    condition: &RateCondition<Role, A>,
    exchange: &Exchange<RateId, Role, AccountId>,
    accounts: &indexmap::IndexMap<AccountId, std::sync::Arc<Account<A>>>,
) -> Result<bool, economy::ExpressionError> {
    economy::evaluate_condition(condition, exchange, accounts)
}

pub fn model_new<AccountId: Eq + std::hash::Hash, A, RateId: Eq + std::hash::Hash, Role>()
-> economy::EconomyBuilder<AccountId, A, RateId, Role> {
    economy::EconomyBuilder::new()
}

pub fn model_account<AccountId: Eq + std::hash::Hash, A, RateId: Eq + std::hash::Hash, Role>(
    builder: economy::EconomyBuilder<AccountId, A, RateId, Role>,
    id: AccountId,
    state: Account<A>,
) -> economy::EconomyBuilder<AccountId, A, RateId, Role> {
    builder.account(id, state)
}

pub fn model_rate<AccountId: Eq + std::hash::Hash, A, RateId: Eq + std::hash::Hash, Role>(
    builder: economy::EconomyBuilder<AccountId, A, RateId, Role>,
    id: RateId,
    rule: Rate<Role, A>,
) -> economy::EconomyBuilder<AccountId, A, RateId, Role> {
    builder.rate(id, rule)
}

pub fn model_invariant<AccountId: Eq + std::hash::Hash, A, RateId: Eq + std::hash::Hash, Role>(
    builder: economy::EconomyBuilder<AccountId, A, RateId, Role>,
    invariant: LinearInvariant<A>,
) -> economy::EconomyBuilder<AccountId, A, RateId, Role> {
    builder.invariant(invariant)
}

pub fn model_build<AccountId: Eq + std::hash::Hash, A, RateId: Eq + std::hash::Hash, Role>(
    builder: economy::EconomyBuilder<AccountId, A, RateId, Role>,
) -> economy::ModelBuildResult<AccountId, A, RateId, Role> {
    builder.build()
}

pub fn rate_new<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>() -> Rate<Role, A> {
    Rate::new()
}

pub fn rate_distinct<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    left: Role,
    right: Role,
) -> Rate<Role, A> {
    rule.distinct(left, right)
}

pub fn rate_computed_consume<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    asset: A,
    expression: QuantityExpression<Role, A>,
) -> Rate<Role, A> {
    rule.consume_computed(role, asset, expression)
}

pub fn rate_computed_produce<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    asset: A,
    expression: QuantityExpression<Role, A>,
) -> Rate<Role, A> {
    rule.produce_computed(role, asset, expression)
}

pub fn rate_computed_preserve<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    asset: A,
    expression: QuantityExpression<Role, A>,
) -> Rate<Role, A> {
    rule.preserve_computed(role, asset, expression)
}

pub fn rate_condition<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    label: String,
    left: QuantityExpression<Role, A>,
    comparison: QuantityComparison,
    right: QuantityExpression<Role, A>,
) -> Rate<Role, A> {
    rule.condition(label, left, comparison, right)
}

pub fn rate_parameters<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: &Rate<Role, A>,
) -> std::collections::BTreeSet<&str> {
    rule.parameter_names()
}

pub fn rate_assets<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: &Rate<Role, A>,
) -> Vec<&A> {
    rule.computed_asset_keys()
}

pub fn model_default<AccountId: Eq + std::hash::Hash, A, RateId: Eq + std::hash::Hash, Role>()
-> economy::EconomyBuilder<AccountId, A, RateId, Role> {
    economy::EconomyBuilder::default()
}

pub fn rate_default<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>() -> Rate<Role, A> {
    Rate::default()
}

#[allow(dead_code, clippy::type_complexity)]
pub(crate) fn validate_model_proposal<
    'a,
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &'a economy::Economy<AccountId, A, RateId, Role>,
    proposal: &Exchange<RateId, Role, AccountId>,
) -> Result<&'a Rate<Role, A>, Vec<economy::ApplyError<RateId, Role, AccountId, A>>> {
    model.validate_structure(proposal)
}

pub fn assess_model_proposal<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    proposal: &Exchange<RateId, Role, AccountId>,
) -> economy::ExchangeAssessment<AccountId, A, RateId, Role> {
    model.assess(proposal)
}

pub fn model_is_applicable<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    proposal: &Exchange<RateId, Role, AccountId>,
) -> bool {
    model.is_applicable(proposal)
}

pub fn model_applicable<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    candidates: Vec<Exchange<RateId, Role, AccountId>>,
) -> Vec<Exchange<RateId, Role, AccountId>> {
    model.applicable(candidates)
}

pub fn apply_model_proposal<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &mut economy::Economy<AccountId, A, RateId, Role>,
    proposal: Exchange<RateId, Role, AccountId>,
) -> economy::ApplyResult<AccountId, A, RateId, Role> {
    model.apply(proposal)
}

pub fn model_fork<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
) -> economy::Economy<AccountId, A, RateId, Role> {
    model.fork()
}

pub fn model_simulate<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    proposal: Exchange<RateId, Role, AccountId>,
) -> economy::SimulationResult<AccountId, A, RateId, Role> {
    model.simulate(proposal)
}

pub fn model_replay<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &mut economy::Economy<AccountId, A, RateId, Role>,
    trace: &Trace<RateId, Role, AccountId>,
) -> economy::ReplayResult<AccountId, A, RateId, Role> {
    model.replay(trace)
}

#[allow(clippy::type_complexity)] // Keep the forwarding signature identical to production.
pub fn model_replayed<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    trace: &Trace<RateId, Role, AccountId>,
) -> Result<
    economy::Economy<AccountId, A, RateId, Role>,
    economy::ApplyError<RateId, Role, AccountId, A>,
> {
    model.replayed(trace)
}

pub fn trace_new<RateId, Role, AccountId>() -> Trace<RateId, Role, AccountId> {
    Trace::new()
}

pub fn trace_default<RateId, Role, AccountId>() -> Trace<RateId, Role, AccountId> {
    Trace::default()
}

pub fn trace_push<RateId, Role, AccountId>(
    trace: &mut Trace<RateId, Role, AccountId>,
    proposal: Exchange<RateId, Role, AccountId>,
) {
    trace.push(proposal)
}

pub fn trace_exchanges<RateId, Role, AccountId>(
    trace: &Trace<RateId, Role, AccountId>,
) -> &[Exchange<RateId, Role, AccountId>] {
    trace.exchanges()
}

pub fn trace_extend<RateId, Role, AccountId, N, I>(
    trace: &mut Trace<RateId, Role, AccountId, N>,
    exchanges: I,
) where
    I: IntoIterator<Item = Exchange<RateId, Role, AccountId, N>>,
{
    trace.extend(exchanges)
}

pub fn trace_into_exchanges<RateId, Role, AccountId>(
    trace: Trace<RateId, Role, AccountId>,
) -> Vec<Exchange<RateId, Role, AccountId>> {
    trace.into_exchanges()
}

pub fn inspect_model_account<
    'a,
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &'a economy::Economy<AccountId, A, RateId, Role>,
    id: &AccountId,
) -> Option<&'a Account<A>> {
    model.account(id)
}

pub fn model_balance<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    id: &AccountId,
    asset: &A,
) -> Quantity<u64> {
    model.balance(id, asset)
}

pub fn model_matches<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    goal: &economy::Goal<AccountId, A>,
) -> bool {
    model.matches(goal)
}

pub fn model_view<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
    visible: Vec<AccountId>,
) -> economy::EconomicView<'_, AccountId, A, RateId, Role> {
    model.view(visible)
}

pub fn model_state_key<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
) -> Vec<(AccountId, A, Quantity<u64>)> {
    model.state_key()
}

pub fn model_fingerprint<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    model: &economy::Economy<AccountId, A, RateId, Role>,
) -> economy::StateFingerprint {
    model.state_fingerprint()
}

pub fn view_account<
    'a,
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    view: &'a economy::EconomicView<'_, AccountId, A, RateId, Role>,
    id: &AccountId,
) -> Option<&'a Account<A>> {
    view.account(id)
}

pub fn view_balance<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    view: &economy::EconomicView<'_, AccountId, A, RateId, Role>,
    id: &AccountId,
    asset: &A,
) -> Option<Quantity<u64>> {
    view.balance(id, asset)
}

pub fn view_key<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
>(
    view: &economy::EconomicView<'_, AccountId, A, RateId, Role>,
) -> economy::ObservationKey<AccountId, A> {
    view.observation_key()
}

pub fn goal_new<AccountId: Ord, A>() -> economy::Goal<AccountId, A> {
    economy::Goal::new()
}

pub fn goal_default<AccountId: Ord, A>() -> economy::Goal<AccountId, A> {
    economy::Goal::default()
}

pub fn goal_require<AccountId: Ord, A>(
    goal: economy::Goal<AccountId, A>,
    id: AccountId,
    assets: Basket<A>,
) -> economy::Goal<AccountId, A> {
    goal.require(id, assets)
}

pub fn goal_requirements<AccountId: Ord, A>(
    goal: &economy::Goal<AccountId, A>,
) -> &std::collections::BTreeMap<AccountId, Basket<A>> {
    goal.requirements()
}

pub fn goal_assets<AccountId: Ord, A: Ord>(
    goal: &economy::Goal<AccountId, A>,
) -> std::collections::BTreeSet<&A> {
    goal.asset_keys()
}

pub fn key_visible<AccountId, A>(key: &economy::ObservationKey<AccountId, A>) -> &[AccountId] {
    key.visible_accounts()
}

pub fn key_existing<AccountId, A>(key: &economy::ObservationKey<AccountId, A>) -> &[AccountId] {
    key.existing_accounts()
}

pub fn key_balances<AccountId, A>(
    key: &economy::ObservationKey<AccountId, A>,
) -> &[(AccountId, A, Quantity<u64>)] {
    key.balances()
}

pub fn fingerprint_get(value: economy::StateFingerprint) -> u64 {
    value.get()
}

// Typed public data boundaries: each root forwards to the production API.

pub fn amount_new<A, N>(asset: A, value: Quantity<N>) -> AssetAmount<A, N> {
    AssetAmount::new(asset, value)
}

pub fn amount_asset<A, N>(value: &AssetAmount<A, N>) -> &A {
    value.asset()
}

pub fn amount_quantity<A, N>(value: &AssetAmount<A, N>) -> &Quantity<N> {
    value.quantity()
}

pub fn amount_parts<A, N>(value: AssetAmount<A, N>) -> (A, Quantity<N>) {
    value.into_parts()
}

pub fn amount_from_pair<A, N>(parts: (A, Quantity<N>)) -> AssetAmount<A, N> {
    AssetAmount::from(parts)
}

pub fn amount_to_pair<A, N>(value: AssetAmount<A, N>) -> (A, Quantity<N>) {
    value.into()
}

pub fn quantity_new(value: u64) -> Quantity {
    Quantity::new(value)
}

pub fn quantity_get(value: Quantity) -> u64 {
    value.get()
}

pub fn quantity_from_u64(value: u64) -> Quantity {
    Quantity::from(value)
}

pub fn quantity_from_u32(value: u32) -> Quantity {
    Quantity::from(value)
}

pub fn quantity_validate<N: QuantityScalar>(
    value: N,
) -> Result<Quantity<N>, quantity::QuantityError> {
    Quantity::try_from_scalar(value)
}

pub fn quantity_borrow<N: QuantityScalar>(value: &Quantity<N>) -> &N {
    value.as_scalar()
}

pub fn quantity_consume<N: QuantityScalar>(value: Quantity<N>) -> N {
    value.into_scalar()
}

pub fn account_new<A, N>(assets: Basket<A, N>) -> Account<A, N> {
    Account::new(assets)
}

pub fn account_default<A, N>() -> Account<A, N> {
    Account::default()
}

pub fn account_from_basket<A, N>(assets: Basket<A, N>) -> Account<A, N> {
    Account::from(assets)
}

pub fn account_into_basket<A, N>(holder: Account<A, N>) -> Basket<A, N> {
    holder.into_balances()
}

pub fn account_borrow_basket<A, N>(holder: &Account<A, N>) -> &Basket<A, N> {
    holder.balances()
}

pub fn account_equal<A: Eq + std::hash::Hash>(left: &Account<A>, right: &Account<A>) -> bool {
    left == right
}

pub fn proposal_new<RateId, Role: Ord, AccountId, N>(
    rate_id: RateId,
    units: Quantity<N>,
) -> Exchange<RateId, Role, AccountId, N> {
    Exchange::new(rate_id, units)
}

pub fn proposal_bind<RateId, Role: Ord, AccountId, N>(
    proposal: Exchange<RateId, Role, AccountId, N>,
    role: Role,
    holder: AccountId,
) -> Exchange<RateId, Role, AccountId, N> {
    proposal.bind(role, holder)
}

pub fn proposal_parameter<RateId, Role: Ord, AccountId, N>(
    proposal: Exchange<RateId, Role, AccountId, N>,
    name: String,
    value: Quantity<N>,
) -> Exchange<RateId, Role, AccountId, N> {
    proposal.parameter(name, value)
}

pub fn proposal_rate<RateId, Role: Ord, AccountId, N>(
    proposal: &Exchange<RateId, Role, AccountId, N>,
) -> &RateId {
    proposal.rate()
}

pub fn proposal_bindings<RateId, Role: Ord, AccountId, N>(
    proposal: &Exchange<RateId, Role, AccountId, N>,
) -> &std::collections::BTreeMap<Role, AccountId> {
    proposal.bindings()
}

pub fn proposal_units<RateId, Role: Ord, AccountId, N>(
    proposal: &Exchange<RateId, Role, AccountId, N>,
) -> &Quantity<N> {
    proposal.units()
}

pub fn proposal_parameters<RateId, Role: Ord, AccountId, N>(
    proposal: &Exchange<RateId, Role, AccountId, N>,
) -> &std::collections::BTreeMap<String, Quantity<N>> {
    proposal.parameters()
}

pub fn delta_account<AccountId, A, N>(value: &AccountDelta<AccountId, A, N>) -> &AccountId {
    value.account()
}

pub fn delta_consumed<AccountId, A, N>(value: &AccountDelta<AccountId, A, N>) -> &Basket<A, N> {
    value.consumed()
}

pub fn delta_produced<AccountId, A, N>(value: &AccountDelta<AccountId, A, N>) -> &Basket<A, N> {
    value.produced()
}

pub fn delta_preserved<AccountId, A, N>(value: &AccountDelta<AccountId, A, N>) -> &Basket<A, N> {
    value.preserved()
}

pub fn receipt_exchange<RateId, Role, AccountId, A, N>(
    value: &Receipt<RateId, Role, AccountId, A, N>,
) -> &Exchange<RateId, Role, AccountId, N> {
    value.exchange()
}

pub fn receipt_deltas<RateId, Role, AccountId, A, N>(
    value: &Receipt<RateId, Role, AccountId, A, N>,
) -> &[AccountDelta<AccountId, A, N>] {
    value.deltas()
}

pub fn account_assessment_account<AccountId, A, N>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> &AccountId {
    value.account()
}

pub fn account_assessment_available<AccountId, A, N>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> &Basket<A, N> {
    value.available()
}

pub fn account_assessment_required<AccountId, A, N>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> &Basket<A, N> {
    value.required()
}

pub fn account_assessment_consumed<AccountId, A, N>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> &Basket<A, N> {
    value.consumed()
}

pub fn account_assessment_produced<AccountId, A, N>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> &Basket<A, N> {
    value.produced()
}

pub fn account_assessment_preserved<AccountId, A, N>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> &Basket<A, N> {
    value.preserved()
}

pub fn account_shortfall_account<AccountId, A, N>(
    value: &economy::AccountShortfall<AccountId, A, N>,
) -> &AccountId {
    value.account()
}

pub fn account_shortfall_missing<AccountId, A, N>(
    value: &economy::AccountShortfall<AccountId, A, N>,
) -> &Basket<A, N> {
    value.missing()
}

pub fn assessment_status<AccountId, A, RateId, Role, N: QuantityScalar>(
    value: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) -> economy::AssessmentStatus {
    value.status()
}

pub fn assessment_accounts<AccountId, A, RateId, Role, N: QuantityScalar>(
    value: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) -> &[economy::AccountAssessment<AccountId, A, N>] {
    value.accounts()
}

pub fn assessment_shortfalls<AccountId, A, RateId, Role, N: QuantityScalar>(
    value: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) -> &[economy::AccountShortfall<AccountId, A, N>] {
    value.shortfalls()
}

pub fn assessment_projected_deltas<AccountId, A, RateId, Role, N: QuantityScalar>(
    value: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) -> Option<&[AccountDelta<AccountId, A, N>]> {
    value.projected_deltas()
}

pub fn assessment_issues<AccountId, A, RateId, Role, N: QuantityScalar>(
    value: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) -> &[economy::ApplyError<RateId, Role, AccountId, A, N>] {
    value.issues()
}

pub fn computed_role<Role, A, N>(value: &ComputedAmount<Role, A, N>) -> &Role {
    value.role()
}

pub fn computed_asset<Role, A, N>(value: &ComputedAmount<Role, A, N>) -> &A {
    value.asset()
}

pub fn computed_quantity<Role, A, N>(
    value: &ComputedAmount<Role, A, N>,
) -> &QuantityExpression<Role, A, N> {
    value.quantity()
}

pub fn condition_name<Role, A, N>(value: &RateCondition<Role, A, N>) -> &str {
    value.name()
}

pub fn condition_left<Role, A, N>(
    value: &RateCondition<Role, A, N>,
) -> &QuantityExpression<Role, A, N> {
    value.left()
}

pub fn condition_comparison<Role, A, N>(value: &RateCondition<Role, A, N>) -> QuantityComparison {
    value.comparison()
}

pub fn condition_right<Role, A, N>(
    value: &RateCondition<Role, A, N>,
) -> &QuantityExpression<Role, A, N> {
    value.right()
}

pub fn model_build_error_issues<AccountId, RateId>(
    value: &economy::ModelBuildError<AccountId, RateId>,
) -> &[economy::ModelIssue<AccountId, RateId>] {
    value.issues()
}

pub fn expression_constant<Role, A>(value: u64) -> QuantityExpression<Role, A> {
    QuantityExpression::constant(value)
}

pub fn expression_units<Role, A, N>() -> QuantityExpression<Role, A, N> {
    QuantityExpression::units()
}

pub fn expression_balance<Role, A, N>(role: Role, asset: A) -> QuantityExpression<Role, A, N> {
    QuantityExpression::balance(role, asset)
}

pub fn expression_parameter<Role, A, N>(name: String) -> QuantityExpression<Role, A, N> {
    QuantityExpression::parameter(name)
}

pub fn expression_plus<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::plus(left, right)
}

pub fn expression_subtract<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::subtract(left, right)
}

pub fn expression_multiply<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::multiply(left, right)
}

pub fn expression_mul_div<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
    denominator: QuantityExpression<Role, A, N>,
    rounding: Rounding,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::mul_div(left, right, denominator, rounding)
}

pub fn expression_divide<Role, A>(
    numerator: QuantityExpression<Role, A>,
    denominator: QuantityExpression<Role, A>,
    rounding: Rounding,
) -> QuantityExpression<Role, A> {
    QuantityExpression::divide(numerator, denominator, rounding)
}

pub fn quantity_mul_div(
    left: &Quantity,
    right: &Quantity,
    denominator: &Quantity,
    rounding: Rounding,
) -> Option<Quantity> {
    left.checked_mul_div(right, denominator, rounding)
}

pub fn expression_minimum<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::minimum(left, right)
}

pub fn expression_maximum<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::maximum(left, right)
}

pub fn expression_add<Role, A, N>(
    left: QuantityExpression<Role, A, N>,
    right: QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    left + right
}

pub fn basket_insert_amount<A: Eq + std::hash::Hash, N: QuantityScalar>(
    state: &mut Basket<A, N>,
    value: AssetAmount<A, N>,
) -> Option<Quantity<N>> {
    state.insert_amount(value)
}

// Borrowed inspection and invariant construction boundaries.

pub fn assessment_account<'a, AccountId: PartialEq, A, RateId, Role, N: QuantityScalar>(
    value: &'a economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
    id: &AccountId,
) -> Option<&'a economy::AccountAssessment<AccountId, A, N>> {
    value.account(id)
}

pub fn assessment_shortfall<'a, AccountId: PartialEq, A, RateId, Role, N: QuantityScalar>(
    value: &'a economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
    id: &AccountId,
) -> Option<&'a Basket<A, N>> {
    value.shortfall(id)
}

pub fn inspect_rate_roles<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash, N: QuantityScalar>(
    rule: &Rate<Role, A, N>,
) -> impl Iterator<Item = &Role> {
    rule.roles()
}

pub fn inspect_rate_distinct_roles<
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &Rate<Role, A, N>,
) -> impl Iterator<Item = &(Role, Role)> {
    rule.distinct_roles()
}

pub fn inspect_rate_computed_consumed<
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &Rate<Role, A, N>,
) -> &[ComputedAmount<Role, A, N>] {
    rule.computed_consumed()
}

pub fn inspect_rate_computed_produced<
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &Rate<Role, A, N>,
) -> &[ComputedAmount<Role, A, N>] {
    rule.computed_produced()
}

pub fn inspect_rate_computed_preserved<
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &Rate<Role, A, N>,
) -> &[ComputedAmount<Role, A, N>] {
    rule.computed_preserved()
}

pub fn inspect_rate_conditions<
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &Rate<Role, A, N>,
) -> &[RateCondition<Role, A, N>] {
    rule.conditions()
}

pub fn inspect_rate_consumed<
    'a,
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &'a Rate<Role, A, N>,
    role: &Role,
) -> Option<&'a Basket<A, N>> {
    rule.consumed(role)
}

pub fn inspect_rate_produced<
    'a,
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &'a Rate<Role, A, N>,
    role: &Role,
) -> Option<&'a Basket<A, N>> {
    rule.produced(role)
}

pub fn inspect_rate_preserved<
    'a,
    Role: Clone + Ord,
    A: Clone + Eq + std::hash::Hash,
    N: QuantityScalar,
>(
    rule: &'a Rate<Role, A, N>,
    role: &Role,
) -> Option<&'a Basket<A, N>> {
    rule.preserved(role)
}

pub fn invariant_new<A: Eq + std::hash::Hash, S: Into<String>>(name: S) -> LinearInvariant<A> {
    LinearInvariant::new(name)
}

pub fn invariant_from_serialized_weights<A: Eq + std::hash::Hash>(
    name: String,
    entries: Vec<(A, i64)>,
) -> Result<LinearInvariant<A>, A> {
    LinearInvariant::from_serialized_weights(name, entries)
}

pub fn invariant_weight<A: Eq + std::hash::Hash>(
    value: LinearInvariant<A>,
    asset: A,
    weight: i64,
) -> LinearInvariant<A> {
    value.weight(asset, weight)
}

pub fn invariant_name<A: Eq + std::hash::Hash>(value: &LinearInvariant<A>) -> &str {
    value.name()
}

pub fn invariant_weights<A: Eq + std::hash::Hash>(
    value: &LinearInvariant<A>,
) -> impl Iterator<Item = (&A, i64)> {
    value.weights()
}

pub fn inspect_model_accounts<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> impl Iterator<Item = (&AccountId, &Account<A, N>)> {
    model.accounts()
}

pub fn inspect_model_rate_ids<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> impl Iterator<Item = &RateId> {
    model.rate_ids()
}

pub fn inspect_model_invariants<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> impl Iterator<Item = &LinearInvariant<A>> {
    model.invariants()
}

pub fn inspect_model_asset_keys<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> std::collections::BTreeSet<&A> {
    model.asset_keys()
}

pub fn inspect_model_rate<
    'a,
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &'a economy::Economy<AccountId, A, RateId, Role, N>,
    id: &RateId,
) -> Option<&'a Rate<Role, A, N>> {
    model.rate(id)
}

pub fn inspect_model_accounts_next<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> Option<(&AccountId, &Account<A, N>)> {
    model.accounts().next()
}

pub fn inspect_model_accounts_hint<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> (usize, Option<usize>) {
    model.accounts().size_hint()
}

pub fn inspect_model_rate_ids_next<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> Option<&RateId> {
    model.rate_ids().next()
}

pub fn inspect_model_rate_ids_hint<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> (usize, Option<usize>) {
    model.rate_ids().size_hint()
}

pub fn inspect_model_invariants_next<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> Option<&LinearInvariant<A>> {
    model.invariants().next()
}

pub fn inspect_model_invariants_hint<
    AccountId: Clone + Eq + std::hash::Hash + Ord,
    A: Clone + Eq + std::hash::Hash + Ord,
    RateId: Clone + Eq + std::hash::Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
>(
    model: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> (usize, Option<usize>) {
    model.invariants().size_hint()
}

pub fn invariant_weights_hint<A: Eq + std::hash::Hash>(
    value: &LinearInvariant<A>,
) -> (usize, Option<usize>) {
    value.weights().size_hint()
}

pub fn rate_consume_or_panic<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    assets: Basket<A>,
) -> Rate<Role, A> {
    rule.consume(role, assets)
}

pub fn rate_produce_or_panic<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    assets: Basket<A>,
) -> Rate<Role, A> {
    rule.produce(role, assets)
}

pub fn rate_preserve_or_panic<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash>(
    rule: Rate<Role, A>,
    role: Role,
    assets: Basket<A>,
) -> Rate<Role, A> {
    rule.preserve(role, assets)
}

pub fn basket_literal<A: Eq + std::hash::Hash, const LEN: usize>(
    entries: [(A, u64); LEN],
) -> Basket<A> {
    rate::basket(entries)
}

pub fn proposal_parameter_with<RateId, Role: Ord, AccountId, N, S: Into<String>>(
    proposal: Exchange<RateId, Role, AccountId, N>,
    name: S,
    value: Quantity<N>,
) -> Exchange<RateId, Role, AccountId, N> {
    proposal.parameter(name, value)
}

pub fn expression_parameter_with<Role, A, N, S: Into<String>>(
    name: S,
) -> QuantityExpression<Role, A, N> {
    QuantityExpression::parameter(name)
}

pub fn rate_condition_with<Role: Clone + Ord, A: Clone + Eq + std::hash::Hash, S: Into<String>>(
    rule: Rate<Role, A>,
    label: S,
    left: QuantityExpression<Role, A>,
    comparison: QuantityComparison,
    right: QuantityExpression<Role, A>,
) -> Rate<Role, A> {
    rule.condition(label, left, comparison, right)
}

pub fn derived_quantity_eq<N: PartialEq>(left: &Quantity<N>, right: &Quantity<N>) -> bool {
    left == right
}

pub fn derived_quantity_clone<N: Clone>(value: &Quantity<N>) -> Quantity<N> {
    value.clone()
}

pub fn derived_amount_eq<A: PartialEq, N: PartialEq>(
    left: &AssetAmount<A, N>,
    right: &AssetAmount<A, N>,
) -> bool {
    left == right
}

pub fn derived_amount_clone<A: Clone, N: Clone>(value: &AssetAmount<A, N>) -> AssetAmount<A, N> {
    value.clone()
}

pub fn derived_expression_eq<Role: PartialEq, A: PartialEq, N: PartialEq>(
    left: &QuantityExpression<Role, A, N>,
    right: &QuantityExpression<Role, A, N>,
) -> bool {
    left == right
}

pub fn derived_expression_clone<Role: Clone, A: Clone, N: Clone>(
    value: &QuantityExpression<Role, A, N>,
) -> QuantityExpression<Role, A, N> {
    clone_value(value)
}

pub fn derived_computed_amount_eq<Role: PartialEq, A: PartialEq, N: PartialEq>(
    left: &ComputedAmount<Role, A, N>,
    right: &ComputedAmount<Role, A, N>,
) -> bool {
    left == right
}

pub fn derived_computed_amount_clone<Role: Clone, A: Clone, N: Clone>(
    value: &ComputedAmount<Role, A, N>,
) -> ComputedAmount<Role, A, N> {
    value.clone()
}

pub fn derived_condition_eq<Role: PartialEq, A: PartialEq, N: PartialEq>(
    left: &RateCondition<Role, A, N>,
    right: &RateCondition<Role, A, N>,
) -> bool {
    left == right
}

pub fn derived_condition_clone<Role: Clone, A: Clone, N: Clone>(
    value: &RateCondition<Role, A, N>,
) -> RateCondition<Role, A, N> {
    value.clone()
}

pub fn derived_comparison_eq(left: &QuantityComparison, right: &QuantityComparison) -> bool {
    left == right
}

// Extract Clone explicitly even for Copy types.
#[allow(clippy::clone_on_copy)]
pub fn derived_comparison_clone(value: &QuantityComparison) -> QuantityComparison {
    value.clone()
}

pub fn derived_rounding_eq(left: &Rounding, right: &Rounding) -> bool {
    left == right
}

// Extract Clone explicitly even for Copy types.
#[allow(clippy::clone_on_copy)]
pub fn derived_rounding_clone(value: &Rounding) -> Rounding {
    value.clone()
}

pub fn derived_fingerprint_eq(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> bool {
    left == right
}

// Extract Clone explicitly even for Copy types.
#[allow(clippy::clone_on_copy)]
pub fn derived_fingerprint_clone(value: &economy::StateFingerprint) -> economy::StateFingerprint {
    value.clone()
}

pub fn derived_assessment_status_eq(
    left: &economy::AssessmentStatus,
    right: &economy::AssessmentStatus,
) -> bool {
    left == right
}

// Extract Clone explicitly even for Copy types.
#[allow(clippy::clone_on_copy)]
pub fn derived_assessment_status_clone(
    value: &economy::AssessmentStatus,
) -> economy::AssessmentStatus {
    value.clone()
}

pub fn derived_model_issue_eq<AccountId: PartialEq, RateId: PartialEq>(
    left: &economy::ModelIssue<AccountId, RateId>,
    right: &economy::ModelIssue<AccountId, RateId>,
) -> bool {
    left == right
}

pub fn derived_model_issue_clone<AccountId: Clone, RateId: Clone>(
    value: &economy::ModelIssue<AccountId, RateId>,
) -> economy::ModelIssue<AccountId, RateId> {
    value.clone()
}

pub fn derived_model_error_eq<AccountId: PartialEq, RateId: PartialEq>(
    left: &economy::ModelBuildError<AccountId, RateId>,
    right: &economy::ModelBuildError<AccountId, RateId>,
) -> bool {
    left == right
}

pub fn derived_model_error_clone<AccountId: Clone, RateId: Clone>(
    value: &economy::ModelBuildError<AccountId, RateId>,
) -> economy::ModelBuildError<AccountId, RateId> {
    value.clone()
}

pub fn derived_basket_error_eq<A: PartialEq>(
    left: &basket::BasketError<A>,
    right: &basket::BasketError<A>,
) -> bool {
    left == right
}

pub fn derived_basket_error_clone<A: Clone>(
    value: &basket::BasketError<A>,
) -> basket::BasketError<A> {
    value.clone()
}

pub fn derived_rate_error_eq<Role: PartialEq, A: PartialEq>(
    left: &RateError<Role, A>,
    right: &RateError<Role, A>,
) -> bool {
    left == right
}

pub fn derived_rate_error_clone<Role: Clone, A: Clone>(
    value: &RateError<Role, A>,
) -> RateError<Role, A> {
    value.clone()
}

pub fn derived_quantity_error_eq(
    left: &quantity::QuantityError,
    right: &quantity::QuantityError,
) -> bool {
    left == right
}

// Extract Clone explicitly even for Copy types.
#[allow(clippy::clone_on_copy)]
pub fn derived_quantity_error_clone(value: &quantity::QuantityError) -> quantity::QuantityError {
    value.clone()
}

pub fn derived_proposal_eq<
    RateId: PartialEq,
    Role: PartialEq,
    AccountId: PartialEq,
    N: PartialEq,
>(
    left: &Exchange<RateId, Role, AccountId, N>,
    right: &Exchange<RateId, Role, AccountId, N>,
) -> bool {
    left == right
}

pub fn derived_proposal_clone<RateId: Clone, Role: Clone, AccountId: Clone, N: Clone>(
    value: &Exchange<RateId, Role, AccountId, N>,
) -> Exchange<RateId, Role, AccountId, N> {
    value.clone()
}

pub fn derived_trace_eq<RateId: PartialEq, Role: PartialEq, AccountId: PartialEq, N: PartialEq>(
    left: &Trace<RateId, Role, AccountId, N>,
    right: &Trace<RateId, Role, AccountId, N>,
) -> bool {
    left == right
}

pub fn derived_trace_clone<RateId: Clone, Role: Clone, AccountId: Clone, N: Clone>(
    value: &Trace<RateId, Role, AccountId, N>,
) -> Trace<RateId, Role, AccountId, N> {
    value.clone()
}

pub fn derived_observation_key_eq<AccountId: PartialEq, A: PartialEq, N: PartialEq>(
    left: &economy::ObservationKey<AccountId, A, N>,
    right: &economy::ObservationKey<AccountId, A, N>,
) -> bool {
    left == right
}

pub fn derived_observation_key_clone<AccountId: Clone, A: Clone, N: Clone>(
    value: &economy::ObservationKey<AccountId, A, N>,
) -> economy::ObservationKey<AccountId, A, N> {
    value.clone()
}

pub fn derived_quantity_partial_cmp<N: PartialOrd>(
    left: &Quantity<N>,
    right: &Quantity<N>,
) -> Option<std::cmp::Ordering> {
    left.partial_cmp(right)
}

pub fn derived_quantity_lt<N: PartialOrd>(left: &Quantity<N>, right: &Quantity<N>) -> bool {
    left.lt(right)
}

pub fn derived_quantity_le<N: PartialOrd>(left: &Quantity<N>, right: &Quantity<N>) -> bool {
    left.le(right)
}

pub fn derived_quantity_gt<N: PartialOrd>(left: &Quantity<N>, right: &Quantity<N>) -> bool {
    left.gt(right)
}

pub fn derived_quantity_ge<N: PartialOrd>(left: &Quantity<N>, right: &Quantity<N>) -> bool {
    left.ge(right)
}

pub fn derived_quantity_cmp<N: Ord>(left: &Quantity<N>, right: &Quantity<N>) -> std::cmp::Ordering {
    left.cmp(right)
}

pub fn derived_fingerprint_partial_cmp(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> Option<std::cmp::Ordering> {
    left.partial_cmp(right)
}

pub fn derived_fingerprint_lt(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> bool {
    left.lt(right)
}

pub fn derived_fingerprint_le(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> bool {
    left.le(right)
}

pub fn derived_fingerprint_gt(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> bool {
    left.gt(right)
}

pub fn derived_fingerprint_ge(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> bool {
    left.ge(right)
}

pub fn derived_fingerprint_cmp(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> std::cmp::Ordering {
    left.cmp(right)
}

pub fn derived_quantity_hash<N: std::hash::Hash, H: std::hash::Hasher>(
    value: &Quantity<N>,
    state: &mut H,
) {
    std::hash::Hash::hash(value, state)
}

pub fn derived_amount_hash<A: std::hash::Hash, N: std::hash::Hash, H: std::hash::Hasher>(
    value: &AssetAmount<A, N>,
    state: &mut H,
) {
    std::hash::Hash::hash(value, state)
}

pub fn derived_fingerprint_hash<H: std::hash::Hasher>(
    value: &economy::StateFingerprint,
    state: &mut H,
) {
    std::hash::Hash::hash(value, state)
}

pub fn derived_observation_key_hash<
    AccountId: std::hash::Hash,
    A: std::hash::Hash,
    N: std::hash::Hash,
    H: std::hash::Hasher,
>(
    value: &economy::ObservationKey<AccountId, A, N>,
    state: &mut H,
) {
    std::hash::Hash::hash(value, state)
}

pub fn clone_value<T: Clone>(value: &T) -> T {
    value.clone()
}

pub fn derived_basket_clone<A: Clone, N: Clone>(value: &Basket<A, N>) -> Basket<A, N> {
    value.clone()
}

pub fn derived_account_clone<A: Clone, N: Clone>(value: &Account<A, N>) -> Account<A, N> {
    value.clone()
}

pub fn derived_account_error_clone<A: Clone + Eq + std::hash::Hash, N: QuantityScalar>(
    value: &AccountError<A, N>,
) -> AccountError<A, N> {
    value.clone()
}

pub fn derived_delta_clone<AccountId: Clone, A: Clone, N: Clone>(
    value: &AccountDelta<AccountId, A, N>,
) -> AccountDelta<AccountId, A, N> {
    value.clone()
}

pub fn derived_receipt_clone<RateId: Clone, Role: Clone, AccountId: Clone, A: Clone, N: Clone>(
    value: &Receipt<RateId, Role, AccountId, A, N>,
) -> Receipt<RateId, Role, AccountId, A, N> {
    value.clone()
}

pub fn derived_goal_clone<AccountId: Clone, A: Clone, N: Clone>(
    value: &economy::Goal<AccountId, A, N>,
) -> economy::Goal<AccountId, A, N> {
    value.clone()
}

pub fn derived_rate_clone<Role: Clone, A: Clone, N: Clone>(
    value: &Rate<Role, A, N>,
) -> Rate<Role, A, N> {
    value.clone()
}

pub fn derived_invariant_clone<A: Clone>(value: &LinearInvariant<A>) -> LinearInvariant<A> {
    value.clone()
}

pub fn derived_economy_clone<AccountId: Clone, A: Clone, RateId: Clone, Role: Clone, N: Clone>(
    value: &economy::Economy<AccountId, A, RateId, Role, N>,
) -> economy::Economy<AccountId, A, RateId, Role, N> {
    value.clone()
}

pub fn derived_account_assessment_clone<AccountId: Clone, A: Clone, N: Clone>(
    value: &economy::AccountAssessment<AccountId, A, N>,
) -> economy::AccountAssessment<AccountId, A, N> {
    value.clone()
}

pub fn derived_shortfall_clone<AccountId: Clone, A: Clone, N: Clone>(
    value: &economy::AccountShortfall<AccountId, A, N>,
) -> economy::AccountShortfall<AccountId, A, N> {
    value.clone()
}

pub fn derived_assessment_clone<
    AccountId: Clone,
    A: Clone,
    RateId: Clone,
    Role: Clone,
    N: QuantityScalar,
>(
    value: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) -> economy::ExchangeAssessment<AccountId, A, RateId, Role, N> {
    value.clone()
}

pub fn derived_apply_error_clone<
    RateId: Clone,
    Role: Clone,
    AccountId: Clone,
    A: Clone,
    N: QuantityScalar,
>(
    value: &economy::ApplyError<RateId, Role, AccountId, A, N>,
) -> economy::ApplyError<RateId, Role, AccountId, A, N> {
    value.clone()
}

pub fn derived_account_error_eq<A: Eq + std::hash::Hash, N: QuantityScalar>(
    left: &AccountError<A, N>,
    right: &AccountError<A, N>,
) -> bool {
    left == right
}

pub fn inherited_quantity_clone_from<N: Clone>(target: &mut Quantity<N>, source: &Quantity<N>) {
    target.clone_from(source)
}

pub fn inherited_amount_clone_from<A: Clone, N: Clone>(
    target: &mut AssetAmount<A, N>,
    source: &AssetAmount<A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_expression_clone_from<Role: Clone, A: Clone, N: Clone>(
    target: &mut QuantityExpression<Role, A, N>,
    source: &QuantityExpression<Role, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_computed_amount_clone_from<Role: Clone, A: Clone, N: Clone>(
    target: &mut ComputedAmount<Role, A, N>,
    source: &ComputedAmount<Role, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_condition_clone_from<Role: Clone, A: Clone, N: Clone>(
    target: &mut RateCondition<Role, A, N>,
    source: &RateCondition<Role, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_comparison_clone_from(
    target: &mut QuantityComparison,
    source: &QuantityComparison,
) {
    target.clone_from(source)
}

pub fn inherited_rounding_clone_from(target: &mut Rounding, source: &Rounding) {
    target.clone_from(source)
}

pub fn inherited_fingerprint_clone_from(
    target: &mut economy::StateFingerprint,
    source: &economy::StateFingerprint,
) {
    target.clone_from(source)
}

pub fn inherited_assessment_status_clone_from(
    target: &mut economy::AssessmentStatus,
    source: &economy::AssessmentStatus,
) {
    target.clone_from(source)
}

pub fn inherited_model_issue_clone_from<AccountId: Clone, RateId: Clone>(
    target: &mut economy::ModelIssue<AccountId, RateId>,
    source: &economy::ModelIssue<AccountId, RateId>,
) {
    target.clone_from(source)
}

pub fn inherited_model_error_clone_from<AccountId: Clone, RateId: Clone>(
    target: &mut economy::ModelBuildError<AccountId, RateId>,
    source: &economy::ModelBuildError<AccountId, RateId>,
) {
    target.clone_from(source)
}

pub fn inherited_basket_error_clone_from<A: Clone>(
    target: &mut basket::BasketError<A>,
    source: &basket::BasketError<A>,
) {
    target.clone_from(source)
}

pub fn inherited_rate_error_clone_from<Role: Clone, A: Clone>(
    target: &mut RateError<Role, A>,
    source: &RateError<Role, A>,
) {
    target.clone_from(source)
}

pub fn inherited_quantity_error_clone_from(
    target: &mut quantity::QuantityError,
    source: &quantity::QuantityError,
) {
    target.clone_from(source)
}

pub fn inherited_proposal_clone_from<RateId: Clone, Role: Clone, AccountId: Clone, N: Clone>(
    target: &mut Exchange<RateId, Role, AccountId, N>,
    source: &Exchange<RateId, Role, AccountId, N>,
) {
    target.clone_from(source)
}

pub fn inherited_trace_clone_from<RateId: Clone, Role: Clone, AccountId: Clone, N: Clone>(
    target: &mut Trace<RateId, Role, AccountId, N>,
    source: &Trace<RateId, Role, AccountId, N>,
) {
    target.clone_from(source)
}

pub fn inherited_observation_key_clone_from<AccountId: Clone, A: Clone, N: Clone>(
    target: &mut economy::ObservationKey<AccountId, A, N>,
    source: &economy::ObservationKey<AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_basket_clone_from<A: Clone, N: Clone>(
    target: &mut Basket<A, N>,
    source: &Basket<A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_account_clone_from<A: Clone, N: Clone>(
    target: &mut Account<A, N>,
    source: &Account<A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_account_error_clone_from<A: Clone + Eq + std::hash::Hash, N: QuantityScalar>(
    target: &mut AccountError<A, N>,
    source: &AccountError<A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_delta_clone_from<AccountId: Clone, A: Clone, N: Clone>(
    target: &mut AccountDelta<AccountId, A, N>,
    source: &AccountDelta<AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_receipt_clone_from<
    RateId: Clone,
    Role: Clone,
    AccountId: Clone,
    A: Clone,
    N: Clone,
>(
    target: &mut Receipt<RateId, Role, AccountId, A, N>,
    source: &Receipt<RateId, Role, AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_goal_clone_from<AccountId: Clone, A: Clone, N: Clone>(
    target: &mut economy::Goal<AccountId, A, N>,
    source: &economy::Goal<AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_rate_clone_from<Role: Clone, A: Clone, N: Clone>(
    target: &mut Rate<Role, A, N>,
    source: &Rate<Role, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_invariant_clone_from<A: Clone>(
    target: &mut LinearInvariant<A>,
    source: &LinearInvariant<A>,
) {
    target.clone_from(source)
}

pub fn inherited_economy_clone_from<
    AccountId: Clone,
    A: Clone,
    RateId: Clone,
    Role: Clone,
    N: Clone,
>(
    target: &mut economy::Economy<AccountId, A, RateId, Role, N>,
    source: &economy::Economy<AccountId, A, RateId, Role, N>,
) {
    target.clone_from(source)
}

pub fn inherited_account_assessment_clone_from<AccountId: Clone, A: Clone, N: Clone>(
    target: &mut economy::AccountAssessment<AccountId, A, N>,
    source: &economy::AccountAssessment<AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_shortfall_clone_from<AccountId: Clone, A: Clone, N: Clone>(
    target: &mut economy::AccountShortfall<AccountId, A, N>,
    source: &economy::AccountShortfall<AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_assessment_clone_from<
    AccountId: Clone,
    A: Clone,
    RateId: Clone,
    Role: Clone,
    N: QuantityScalar,
>(
    target: &mut economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
    source: &economy::ExchangeAssessment<AccountId, A, RateId, Role, N>,
) {
    target.clone_from(source)
}

pub fn inherited_apply_error_clone_from<
    RateId: Clone,
    Role: Clone,
    AccountId: Clone,
    A: Clone,
    N: QuantityScalar,
>(
    target: &mut economy::ApplyError<RateId, Role, AccountId, A, N>,
    source: &economy::ApplyError<RateId, Role, AccountId, A, N>,
) {
    target.clone_from(source)
}

pub fn inherited_quantity_ne<N: PartialEq>(left: &Quantity<N>, right: &Quantity<N>) -> bool {
    left != right
}

pub fn inherited_amount_ne<A: PartialEq, N: PartialEq>(
    left: &AssetAmount<A, N>,
    right: &AssetAmount<A, N>,
) -> bool {
    left != right
}

pub fn inherited_expression_ne<Role: PartialEq, A: PartialEq, N: PartialEq>(
    left: &QuantityExpression<Role, A, N>,
    right: &QuantityExpression<Role, A, N>,
) -> bool {
    left != right
}

pub fn inherited_computed_amount_ne<Role: PartialEq, A: PartialEq, N: PartialEq>(
    left: &ComputedAmount<Role, A, N>,
    right: &ComputedAmount<Role, A, N>,
) -> bool {
    left != right
}

pub fn inherited_condition_ne<Role: PartialEq, A: PartialEq, N: PartialEq>(
    left: &RateCondition<Role, A, N>,
    right: &RateCondition<Role, A, N>,
) -> bool {
    left != right
}

pub fn inherited_comparison_ne(left: &QuantityComparison, right: &QuantityComparison) -> bool {
    left != right
}

pub fn inherited_rounding_ne(left: &Rounding, right: &Rounding) -> bool {
    left != right
}

pub fn inherited_fingerprint_ne(
    left: &economy::StateFingerprint,
    right: &economy::StateFingerprint,
) -> bool {
    left != right
}

pub fn inherited_assessment_status_ne(
    left: &economy::AssessmentStatus,
    right: &economy::AssessmentStatus,
) -> bool {
    left != right
}

pub fn inherited_model_issue_ne<AccountId: PartialEq, RateId: PartialEq>(
    left: &economy::ModelIssue<AccountId, RateId>,
    right: &economy::ModelIssue<AccountId, RateId>,
) -> bool {
    left != right
}

pub fn inherited_model_error_ne<AccountId: PartialEq, RateId: PartialEq>(
    left: &economy::ModelBuildError<AccountId, RateId>,
    right: &economy::ModelBuildError<AccountId, RateId>,
) -> bool {
    left != right
}

pub fn inherited_basket_error_ne<A: PartialEq>(
    left: &basket::BasketError<A>,
    right: &basket::BasketError<A>,
) -> bool {
    left != right
}

pub fn inherited_rate_error_ne<Role: PartialEq, A: PartialEq>(
    left: &RateError<Role, A>,
    right: &RateError<Role, A>,
) -> bool {
    left != right
}

pub fn inherited_quantity_error_ne(
    left: &quantity::QuantityError,
    right: &quantity::QuantityError,
) -> bool {
    left != right
}

pub fn inherited_proposal_ne<
    RateId: PartialEq,
    Role: PartialEq,
    AccountId: PartialEq,
    N: PartialEq,
>(
    left: &Exchange<RateId, Role, AccountId, N>,
    right: &Exchange<RateId, Role, AccountId, N>,
) -> bool {
    left != right
}

pub fn inherited_trace_ne<
    RateId: PartialEq,
    Role: PartialEq,
    AccountId: PartialEq,
    N: PartialEq,
>(
    left: &Trace<RateId, Role, AccountId, N>,
    right: &Trace<RateId, Role, AccountId, N>,
) -> bool {
    left != right
}

pub fn inherited_observation_key_ne<AccountId: PartialEq, A: PartialEq, N: PartialEq>(
    left: &economy::ObservationKey<AccountId, A, N>,
    right: &economy::ObservationKey<AccountId, A, N>,
) -> bool {
    left != right
}

pub fn inherited_account_error_ne<A: Eq + std::hash::Hash, N: QuantityScalar>(
    left: &AccountError<A, N>,
    right: &AccountError<A, N>,
) -> bool {
    left != right
}

pub fn inherited_basket_ne<A: Eq + std::hash::Hash, N: PartialEq>(
    left: &Basket<A, N>,
    right: &Basket<A, N>,
) -> bool {
    left != right
}

pub fn inherited_account_ne<A: Eq + std::hash::Hash, N: PartialEq>(
    left: &Account<A, N>,
    right: &Account<A, N>,
) -> bool {
    left != right
}

pub fn inherited_quantity_min<N: Ord>(left: Quantity<N>, right: Quantity<N>) -> Quantity<N> {
    left.min(right)
}

pub fn inherited_quantity_max<N: Ord>(left: Quantity<N>, right: Quantity<N>) -> Quantity<N> {
    left.max(right)
}

pub fn inherited_quantity_clamp<N: Ord>(
    value: Quantity<N>,
    lower: Quantity<N>,
    upper: Quantity<N>,
) -> Quantity<N> {
    value.clamp(lower, upper)
}

pub fn inherited_fingerprint_min(
    left: economy::StateFingerprint,
    right: economy::StateFingerprint,
) -> economy::StateFingerprint {
    left.min(right)
}

pub fn inherited_fingerprint_max(
    left: economy::StateFingerprint,
    right: economy::StateFingerprint,
) -> economy::StateFingerprint {
    left.max(right)
}

pub fn inherited_fingerprint_clamp(
    value: economy::StateFingerprint,
    lower: economy::StateFingerprint,
    upper: economy::StateFingerprint,
) -> economy::StateFingerprint {
    value.clamp(lower, upper)
}

pub fn inherited_quantity_hash_slice<N: std::hash::Hash, H: std::hash::Hasher>(
    values: &[Quantity<N>],
    state: &mut H,
) {
    std::hash::Hash::hash_slice(values, state)
}

pub fn inherited_amount_hash_slice<A: std::hash::Hash, N: std::hash::Hash, H: std::hash::Hasher>(
    values: &[AssetAmount<A, N>],
    state: &mut H,
) {
    std::hash::Hash::hash_slice(values, state)
}

pub fn inherited_fingerprint_hash_slice<H: std::hash::Hasher>(
    values: &[economy::StateFingerprint],
    state: &mut H,
) {
    std::hash::Hash::hash_slice(values, state)
}

pub fn inherited_observation_key_hash_slice<
    AccountId: std::hash::Hash,
    A: std::hash::Hash,
    N: std::hash::Hash,
    H: std::hash::Hasher,
>(
    values: &[economy::ObservationKey<AccountId, A, N>],
    state: &mut H,
) {
    std::hash::Hash::hash_slice(values, state)
}
