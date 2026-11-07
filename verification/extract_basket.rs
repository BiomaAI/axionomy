//! Compile the unchanged production source before drop elaboration for Aeneas.
#[path = "../src/amount.rs"]
mod amount;
#[path = "../src/quantity.rs"]
mod quantity;
use amount::AssetAmount;
use quantity::{Quantity, QuantityScalar};
#[path = "../src/basket.rs"]
mod basket;
use basket::{Basket, BasketError};

pub fn construct<A: Eq + std::hash::Hash>(
    entries: Vec<(A, Quantity)>,
) -> Result<Basket<A>, BasketError<A>> {
    basket::construct_entries(entries)
}

fn main() {}
