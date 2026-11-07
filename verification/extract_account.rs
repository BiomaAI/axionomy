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
