use axionomy_units::si::length::meter;
use axionomy_units::si::mass::gram;
use axionomy_units::si::rational64::{Length, Mass};
use axionomy_units::{AssetSchema, Rational};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AssetId {
    Cargo,
}

fn main() {
    let mut schema = AssetSchema::new();
    let cargo = schema
        .define_measure(
            AssetId::Cargo,
            Mass::new::<gram>(Rational::from_integer(1)),
        )
        .unwrap();

    let _ = cargo.encode::<u64>(Length::new::<meter>(Rational::from_integer(1)));
}
