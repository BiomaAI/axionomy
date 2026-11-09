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

Currency-like assets declare a fixed decimal precision instead of a physical
unit. One atom is `10^-decimals` of a whole display unit, as with token
`decimals`, but the precision is part of the asset key: the same logical ID
cannot appear with two precisions, so cents and micro-dollars never alias.
Text and rational inputs must be exact multiples of one atom; nothing rounds.

```rust
use axionomy::{AssetAmount, Quantity};
use axionomy_units::{AssetSchema, UnitAsset};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AssetId {
    Usd,
}

let mut assets = AssetSchema::new();
let usd = assets.define_decimal(AssetId::Usd, 2)?;
let price: AssetAmount<UnitAsset<AssetId>> = usd.parse("12.50")?;

assert_eq!(price.quantity(), Quantity::new(1_250));
assert_eq!(usd.format(price.quantity()), "12.50");
assert!(usd.parse("12.505").is_err());
assert!(assets.define_decimal(AssetId::Usd, 6).is_err());
# Ok::<(), Box<dyn std::error::Error>>(())
```

The precision is capped at 18 decimals so one whole unit fits a `u64` atom
count. Formatting always prints exactly `decimals` fraction digits and
round-trips through `parse`.

`decode` is the inverse of `encode`: it turns a quantity of atoms back into
an exact `uom` value, so 900 gram atoms read as exactly nine tenths of a
kilogram, in whichever compatible unit the reader asks for.

Use `define_discrete` for permissions, facts, and other atomically counted
assets so measured and discrete values can share one `UnitAsset<Id>` basket.
Use `AssetSchema::build_economy` and `validate_goal` to reject unknown or
conflicting keys across the complete model.

`uom` stores values as exact `Rational64`, so one physical value lowers to at
most `i64::MAX` atoms; decimal assets parse any amount below `2²⁵⁶` atoms. The
economy's arithmetic is 256-bit either way.

Run the structured example at `INFO`, or set `RUST_LOG=debug` to include the
canonical denomination definition:

```console
cargo run -p axionomy-units --example cargo_mass
```
