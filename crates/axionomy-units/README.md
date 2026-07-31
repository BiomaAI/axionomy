# axionomy-units

`axionomy-units` is the unit-safe physical authoring boundary for Axionomy. It
uses [`uom`](https://crates.io/crates/uom) with exact rational storage to
validate dimensions and conversions, then lowers a measured value into an
ordinary asset-qualified atomic quantity.

The canonical denomination is part of the asset identity. An `AssetSchema`
declares each logical ID once and issues typed handles; accounts and baskets
never select their own units. Inputs expressed in kilograms, grams, or another
compatible unit normalize through the same handle and therefore produce the
same `UnitAsset` key. Conflicting dimensions, quantity kinds, or atomic bases
cannot silently alias.

```rust
use axionomy::{AssetAmount, Basket, Quantity};
use axionomy_units::{AssetSchema, Rational, UnitAsset};
use axionomy_units::si::mass::{gram, kilogram};
use axionomy_units::si::rational64::Mass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AssetId {
    Cargo,
}

let mut assets = AssetSchema::new();
let cargo = assets.define_measure(
    AssetId::Cargo,
    Mass::new::<gram>(Rational::from_integer(1)),
)?;
let amount: AssetAmount<UnitAsset<AssetId>> =
    cargo.encode(Mass::new::<kilogram>(Rational::new(25, 2)))?;
let basket = Basket::try_from_amounts([amount])?;

assert_eq!(basket.quantity(cargo.asset()), Quantity::new(12_500));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `define_discrete` for permissions, facts, and other atomically counted
assets so measured and discrete values can share one `UnitAsset<Id>` basket.
Use `AssetSchema::build_economy` and `validate_goal` to reject unknown or
conflicting keys across the complete model.

Run the structured example with:

```console
cargo run -p axionomy-units --example cargo_mass
```
