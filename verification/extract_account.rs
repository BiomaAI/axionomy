//! Extract the production account update paths from shared source.
#[path = "../src/amount.rs"]
mod amount;
#[path = "../src/quantity.rs"]
mod quantity;
use amount::AssetAmount;
use quantity::{Quantity, QuantityScalar};
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
