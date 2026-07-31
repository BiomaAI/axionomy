#![doc = include_str!("../README.md")]

use axionomy::{AssetAmount, Quantity, QuantityScalar};
use jiff::{SignedDuration, Span, Timestamp, Zoned};
use thiserror::Error;

pub use jiff;

/// Binds elapsed time to an explicit atomic timeline asset.
#[derive(Debug, Clone)]
pub struct TimelineAsset<A> {
    asset: A,
    atomic_nanoseconds: i128,
}

impl<A> TimelineAsset<A> {
    pub fn new(asset: A, atomic: SignedDuration) -> Result<Self, TimeBindingError> {
        let atomic_nanoseconds = atomic.as_nanos();
        if atomic_nanoseconds <= 0 {
            return Err(TimeBindingError::InvalidAtomicBasis);
        }
        Ok(Self {
            asset,
            atomic_nanoseconds,
        })
    }

    pub fn asset(&self) -> &A {
        &self.asset
    }

    /// Lowers an elapsed duration into an exact asset-qualified amount.
    pub fn encode<N>(&self, duration: SignedDuration) -> Result<AssetAmount<A, N>, TimeBindingError>
    where
        A: Clone,
        N: QuantityScalar,
    {
        let nanoseconds = duration.as_nanos();
        if nanoseconds < 0 {
            return Err(TimeBindingError::NegativeDuration);
        }
        if nanoseconds % self.atomic_nanoseconds != 0 {
            return Err(TimeBindingError::InexactAtomicConversion);
        }
        let count = u64::try_from(nanoseconds / self.atomic_nanoseconds)
            .map_err(|_| TimeBindingError::NumericRange)?;
        let scalar = N::from_u64(count).ok_or(TimeBindingError::NumericRange)?;
        let quantity =
            Quantity::try_from_scalar(scalar).map_err(|_| TimeBindingError::NegativeDuration)?;
        Ok(AssetAmount::new(self.asset.clone(), quantity))
    }

    pub fn encode_between<N>(
        &self,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<AssetAmount<A, N>, TimeBindingError>
    where
        A: Clone,
        N: QuantityScalar,
    {
        self.encode(end.duration_since(start))
    }

    pub fn encode_zoned_between<N>(
        &self,
        start: &Zoned,
        end: &Zoned,
    ) -> Result<AssetAmount<A, N>, TimeBindingError>
    where
        A: Clone,
        N: QuantityScalar,
    {
        self.encode(end.duration_since(start))
    }
}

/// A validated calendar-aware interval used to author timeline obligations.
///
/// It is an authoring value, not hidden runtime state. Calling [`encode`]
/// lowers its elapsed duration into an explicit asset and quantity.
#[derive(Debug, Clone)]
pub struct CalendarWindow {
    opens: Zoned,
    closes: Zoned,
}

impl CalendarWindow {
    pub fn new(opens: Zoned, closes: Zoned) -> Result<Self, TimeBindingError> {
        if closes.duration_since(&opens).is_negative() {
            return Err(TimeBindingError::WindowEndsBeforeItStarts);
        }
        Ok(Self { opens, closes })
    }

    /// Builds a window using Jiff's calendar-aware span arithmetic.
    pub fn from_span(opens: Zoned, span: Span) -> Result<Self, TimeBindingError> {
        let closes = opens.checked_add(span)?;
        Self::new(opens, closes)
    }

    pub fn opens(&self) -> &Zoned {
        &self.opens
    }

    pub fn closes(&self) -> &Zoned {
        &self.closes
    }

    pub fn elapsed(&self) -> SignedDuration {
        self.closes.duration_since(&self.opens)
    }

    pub fn encode<A, N>(
        &self,
        timeline: &TimelineAsset<A>,
    ) -> Result<AssetAmount<A, N>, TimeBindingError>
    where
        A: Clone,
        N: QuantityScalar,
    {
        timeline.encode(self.elapsed())
    }
}

#[derive(Debug, Error)]
pub enum TimeBindingError {
    #[error("atomic timeline basis must be greater than zero")]
    InvalidAtomicBasis,
    #[error("elapsed time cannot become a negative economic quantity")]
    NegativeDuration,
    #[error("duration is not an exact multiple of the timeline asset's atomic basis")]
    InexactAtomicConversion,
    #[error("timeline count is outside the selected quantity backend's supported range")]
    NumericRange,
    #[error("calendar window ends before it starts")]
    WindowEndsBeforeItStarts,
    #[error(transparent)]
    Calendar(#[from] jiff::Error),
}

#[cfg(test)]
mod tests {
    use super::{CalendarWindow, TimelineAsset};
    use axionomy::{AssetAmount, Quantity};
    use jiff::{SignedDuration, Span, Zoned};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Asset {
        ElapsedHour,
    }

    #[test]
    fn timestamp_duration_lowers_into_timeline_assets() {
        let timeline =
            TimelineAsset::new(Asset::ElapsedHour, SignedDuration::from_hours(1)).unwrap();
        let amount: AssetAmount<Asset> = timeline.encode(SignedDuration::from_hours(36)).unwrap();

        assert_eq!(amount.asset(), &Asset::ElapsedHour);
        assert_eq!(amount.quantity(), &Quantity::new(36));
    }

    #[test]
    fn calendar_days_respect_daylight_saving_transitions() {
        let opens: Zoned = "2024-03-09T12:00:00-05:00[America/New_York]"
            .parse()
            .unwrap();
        let window = CalendarWindow::from_span(opens, Span::new().days(1)).unwrap();
        let timeline =
            TimelineAsset::new(Asset::ElapsedHour, SignedDuration::from_hours(1)).unwrap();
        let amount: AssetAmount<Asset> = window.encode(&timeline).unwrap();

        assert_eq!(amount.quantity(), &Quantity::new(23));
    }
}
