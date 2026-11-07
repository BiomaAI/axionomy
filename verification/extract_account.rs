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
