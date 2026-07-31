use axionomy::{AssetAmount, Basket, Quantity};
use axionomy_time::jiff::{SignedDuration, Span, Zoned};
use axionomy_time::{CalendarWindow, TimelineAsset};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Asset {
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
    let elapsed_hour = TimelineAsset::new(Asset::ElapsedHour, SignedDuration::from_hours(1))?;
    let amount: AssetAmount<Asset> = window.encode(&elapsed_hour)?;

    info!(
        opens = %window.opens(),
        closes = %window.closes(),
        calendar_span = "1 day",
        elapsed_hours = %amount.quantity(),
        "resolved a calendar-aware window"
    );

    let mut timeline = Basket::new();
    timeline.insert_amount(amount);
    info!(
        asset = ?Asset::ElapsedHour,
        quantity = %timeline.quantity(&Asset::ElapsedHour),
        "lowered elapsed time into authoritative economic state"
    );

    assert_eq!(timeline.quantity(&Asset::ElapsedHour), Quantity::new(23));
    Ok(())
}
