use axionomy::{AssetAmount, Basket, Quantity};
use axionomy_units::si::mass::{gram, kilogram};
use axionomy_units::si::rational64::Mass;
use axionomy_units::{AssetSchema, Rational, UnitAsset};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AssetId {
    Cargo,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .without_time()
        .compact()
        .init();

    let mut assets = AssetSchema::new();
    let cargo_gram =
        assets.define_measure(AssetId::Cargo, Mass::new::<gram>(Rational::from_integer(1)))?;
    let measured = Mass::new::<kilogram>(Rational::new(25, 2));
    let amount: AssetAmount<UnitAsset<AssetId>> = cargo_gram.encode(measured)?;

    info!(
        input = "12.5 kg",
        asset = ?amount.asset(),
        atoms = %amount.quantity(),
        atomic_basis = "1 g",
        "lowered a dimension-safe value into an economic amount"
    );

    let cargo = Basket::try_from_amounts([amount])?;
    info!(
        basket_entries = cargo.len(),
        cargo_grams = %cargo.quantity(cargo_gram.asset()),
        "inserted the ordinary asset amount into a heterogeneous basket"
    );

    assert_eq!(cargo.quantity(cargo_gram.asset()), Quantity::new(12_500));
    Ok(())
}
