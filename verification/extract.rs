//! Extraction entry points: instantiate the production API at u64.
//! No arithmetic is reimplemented here; Charon follows these calls into axionomy.
//! This example shares the production Cargo.lock and dependency resolution.

use axionomy::Quantity;

pub fn checked_add(left: &Quantity, right: &Quantity) -> Option<Quantity> {
    left.checked_add(right)
}

pub fn checked_sub(left: &Quantity, right: &Quantity) -> Option<Quantity> {
    left.checked_sub(right)
}

pub fn checked_mul(left: &Quantity, right: &Quantity) -> Option<Quantity> {
    left.checked_mul(right)
}

pub fn checked_div(left: &Quantity, right: &Quantity) -> Option<Quantity> {
    left.checked_div(right)
}

fn main() {}
