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
// Receipt/delta constructors are unused by this extraction entrypoint subset.
#[allow(dead_code)]
#[path = "../src/exchange.rs"]
pub mod exchange;
use exchange::{AccountDelta, Exchange, Receipt, Trace};
#[path = "../src/rate.rs"]
pub mod rate;
use rate::{
    LinearInvariant, QuantityComparison, QuantityExpression, Rate, RateCondition, RateError,
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
