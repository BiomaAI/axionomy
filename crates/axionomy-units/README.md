# axionomy-units

`axionomy-units` is the dimension-safe physical authoring boundary for
Axionomy. It uses [`uom`](https://crates.io/crates/uom) with exact rational
storage to validate dimensions and conversions, then lowers a measured value
into an `AssetAmount<A, N>`.

The asset still defines economic meaning and names the atomic basis. For
example, a binding may define one `CargoGram` asset as exactly one gram. A
12.5-kilogram `uom` mass then becomes `Asset::CargoGram` plus
`Quantity(12_500)`. Different physical dimensions never coexist as erased
runtime values inside a basket.

```rust
use axionomy::{AssetAmount, Quantity};
use axionomy_units::{MeasuredAsset, Rational};
use axionomy_units::si::mass::{gram, kilogram};
use axionomy_units::si::rational64::Mass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asset {
    CargoGram,
}

let binding = MeasuredAsset::new(
    Asset::CargoGram,
    Mass::new::<gram>(Rational::from_integer(1)),
)?;
let amount: AssetAmount<Asset> =
    binding.encode(Mass::new::<kilogram>(Rational::new(25, 2)))?;

assert_eq!(amount.asset(), &Asset::CargoGram);
assert_eq!(amount.quantity(), &Quantity::new(12_500));
# Ok::<(), Box<dyn std::error::Error>>(())
```
