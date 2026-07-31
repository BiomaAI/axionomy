# axionomy-time

`axionomy-time` uses [Jiff](https://crates.io/crates/jiff) to author exact
elapsed-time and calendar-aware obligations, then lowers them into the same
self-describing `UnitAsset` representation used by `axionomy-units`.

Clocks and schedules never become hidden engine state. A model declares one
canonical timeline asset in its shared `AssetSchema`; every account, rate, and
goal then uses that asset identity. Jiff may resolve time zones and
daylight-saving behavior, while `uom` may author ordinary physical durations,
without creating competing denominations.

```rust
use axionomy::{AssetAmount, Quantity};
use axionomy_time::{CalendarWindow, TimelineAsset};
use axionomy_time::jiff::{SignedDuration, Span, Zoned};
use axionomy_units::{AssetSchema, UnitAsset};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AssetId {
    ElapsedHour,
}

let mut assets = AssetSchema::new();
let elapsed_hour = TimelineAsset::define(
    &mut assets,
    AssetId::ElapsedHour,
    SignedDuration::from_hours(1),
)?;
let opens: Zoned =
    "2024-03-09T12:00:00-05:00[America/New_York]".parse()?;
let window = CalendarWindow::from_span(opens, Span::new().days(1))?;
let amount: AssetAmount<UnitAsset<AssetId>> = window.encode(&elapsed_hour)?;

// The spring daylight-saving transition makes this calendar day 23 hours.
assert_eq!(amount.quantity(), &Quantity::new(23));
# Ok::<(), Box<dyn std::error::Error>>(())
```

`TimelineAsset::from_schema` reconstructs a Jiff handle for a compatible time
asset originally declared through `uom`, proving both authoring systems share
one denomination rather than maintaining separate clocks.

Encoding currently requires the canonical timeline count to fit `u64` before
it is converted into the selected `Quantity<N>` backend. A wider backend extends
economy arithmetic after construction; it does not widen this adapter boundary.

Run the structured example at `INFO`, or set `RUST_LOG=debug` to include the
canonical timeline definition:

```console
cargo run -p axionomy-time --example calendar_window
```
