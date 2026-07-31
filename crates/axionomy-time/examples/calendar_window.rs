use axionomy::{AssetAmount, Basket, Quantity};
use axionomy_time::jiff::{SignedDuration, Span, Zoned};
use axionomy_time::{CalendarWindow, TimelineAsset};
use axionomy_units::{AssetSchema, UnitAsset};
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AssetId {
    ElapsedHour,
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

    let opens: Zoned = "2024-03-09T12:00:00-05:00[America/New_York]".parse()?;
    let window = CalendarWindow::from_span(opens, Span::new().days(1))?;
    let mut assets = AssetSchema::new();
    let elapsed_hour = TimelineAsset::define(
        &mut assets,
        AssetId::ElapsedHour,
        SignedDuration::from_hours(1),
    )?;
    let amount: AssetAmount<UnitAsset<AssetId>> = window.encode(&elapsed_hour)?;

    info!(
        opens = %window.opens(),
        closes = %window.closes(),
        calendar_span = "1 day",
        elapsed_hours = %amount.quantity(),
        "resolved a calendar-aware window"
    );

    let timeline = Basket::try_from_amounts([amount])?;
    info!(
        asset_id = ?elapsed_hour.asset().id(),
        quantity = %timeline.quantity(elapsed_hour.asset()),
        "lowered elapsed time into authoritative economic state"
    );
    debug!(
        definition = ?elapsed_hour.asset().definition(),
        "canonical timeline denomination"
    );

    assert_eq!(timeline.quantity(elapsed_hour.asset()), Quantity::new(23));
    Ok(())
}
