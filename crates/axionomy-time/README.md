# axionomy-time

`axionomy-time` uses [Jiff](https://crates.io/crates/jiff) to author exact
elapsed-time and calendar-aware obligations, then lowers them into explicit
Axionomy timeline assets.

Clocks and schedules never become hidden engine state. A model may define one
`ElapsedHour` asset as one physical hour, calculate a calendar interval with
Jiff—including time-zone and daylight-saving behavior—and encode the result as
an `AssetAmount<A, N>` used by accounts, rates, and exchanges.

```rust
use axionomy::{AssetAmount, Quantity};
use axionomy_time::{CalendarWindow, TimelineAsset};
use axionomy_time::jiff::{SignedDuration, Span, Zoned};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asset {
    ElapsedHour,
}

let opens: Zoned =
    "2024-03-09T12:00:00-05:00[America/New_York]".parse()?;
let window = CalendarWindow::from_span(opens, Span::new().days(1))?;
let timeline =
    TimelineAsset::new(Asset::ElapsedHour, SignedDuration::from_hours(1))?;
let amount: AssetAmount<Asset> = window.encode(&timeline)?;

// The spring daylight-saving transition makes this calendar day 23 hours.
assert_eq!(amount.quantity(), &Quantity::new(23));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Run the structured example with:

```console
cargo run -p axionomy-time --example calendar_window
```
