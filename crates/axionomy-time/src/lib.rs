#![doc = include_str!("../README.md")]

use axionomy::{AssetAmount, Quantity, QuantityScalar};
use axionomy_units::si::SI;
use axionomy_units::si::rational64::Time;
use axionomy_units::si::time::{Dimension as TimeDimension, second};
use axionomy_units::{
    AssetDefinition, AssetSchema, AssetSchemaError, MeasureScalar, MeasuredAsset, Rational,
    UnitAsset,
};
use jiff::{SignedDuration, Span, Timestamp, Zoned};
use std::hash::Hash;
use thiserror::Error;

pub use jiff;

type TimeMeasure<Id> = MeasuredAsset<Id, TimeDimension, SI<MeasureScalar>>;

/// Typed Jiff authoring handle for one schema-defined timeline asset.
#[derive(Debug, Clone)]
pub struct TimelineAsset<Id> {
    asset: UnitAsset<Id>,
    atomic_nanoseconds: i128,
}

impl<Id> TimelineAsset<Id>
where
    Id: Clone + Eq + Hash,
{
    /// Declares one canonical elapsed-time asset in the shared asset schema.
    pub fn define(
        schema: &mut AssetSchema<Id>,
        id: Id,
        atomic: SignedDuration,
    ) -> Result<Self, TimelineDefinitionError<Id>> {
        let atomic_nanoseconds = validate_atomic_duration(atomic)?;
        let numerator =
            i64::try_from(atomic_nanoseconds).map_err(|_| TimelineDefinitionError::NumericRange)?;
        let atomic_time = Time::new::<second>(Rational::new(numerator, 1_000_000_000));
        let measured: TimeMeasure<Id> = schema.define_measure(id, atomic_time)?;
        Ok(Self {
            asset: measured.asset().clone(),
            atomic_nanoseconds,
        })
    }

    /// Reconstructs a Jiff handle for an existing compatible time asset.
    pub fn from_schema(
        schema: &AssetSchema<Id>,
        id: &Id,
    ) -> Result<Self, TimelineDefinitionError<Id>> {
        let measured: TimeMeasure<Id> = schema.measured(id)?;
        let AssetDefinition::Measured(definition) = measured.asset().definition() else {
            unreachable!("schema.measured returned a discrete asset")
        };
        let atomic_nanoseconds = base_seconds_to_nanoseconds(definition.atomic_base_value())?;
        Ok(Self {
            asset: measured.asset().clone(),
            atomic_nanoseconds,
        })
    }

    pub const fn asset(&self) -> &UnitAsset<Id> {
        &self.asset
    }

    /// Lowers elapsed duration into the timeline asset's canonical atoms.
    ///
    /// The resulting timeline count must fit `u64` before it is converted
    /// through [`QuantityScalar::from_u64`] into the selected economy backend.
    pub fn encode<N>(
        &self,
        duration: SignedDuration,
    ) -> Result<AssetAmount<UnitAsset<Id>, N>, TimeBindingError>
    where
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
        Ok(self.asset.amount(quantity))
    }

    pub fn encode_between<N>(
        &self,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<AssetAmount<UnitAsset<Id>, N>, TimeBindingError>
    where
        N: QuantityScalar,
    {
        self.encode(end.duration_since(start))
    }

    pub fn encode_zoned_between<N>(
        &self,
        start: &Zoned,
        end: &Zoned,
    ) -> Result<AssetAmount<UnitAsset<Id>, N>, TimeBindingError>
    where
        N: QuantityScalar,
    {
        self.encode(end.duration_since(start))
    }
}

fn validate_atomic_duration<Id>(
    atomic: SignedDuration,
) -> Result<i128, TimelineDefinitionError<Id>> {
    let nanoseconds = atomic.as_nanos();
    if nanoseconds <= 0 {
        Err(TimelineDefinitionError::InvalidAtomicBasis)
    } else {
        Ok(nanoseconds)
    }
}

fn base_seconds_to_nanoseconds<Id>(
    seconds: &MeasureScalar,
) -> Result<i128, TimelineDefinitionError<Id>> {
    let numerator = i128::from(*seconds.numer())
        .checked_mul(1_000_000_000)
        .ok_or(TimelineDefinitionError::NumericRange)?;
    let denominator = i128::from(*seconds.denom());
    if numerator <= 0 || numerator % denominator != 0 {
        return Err(TimelineDefinitionError::InexactNanosecondBasis);
    }
    Ok(numerator / denominator)
}

/// A validated calendar-aware interval used to author timeline obligations.
///
/// It is an authoring value, not hidden runtime state. Calling
/// [`CalendarWindow::encode`] lowers its elapsed duration into a schema-defined
/// asset and quantity.
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

    pub const fn opens(&self) -> &Zoned {
        &self.opens
    }

    pub const fn closes(&self) -> &Zoned {
        &self.closes
    }

    pub fn elapsed(&self) -> SignedDuration {
        self.closes.duration_since(&self.opens)
    }

    pub fn encode<Id, N>(
        &self,
        timeline: &TimelineAsset<Id>,
    ) -> Result<AssetAmount<UnitAsset<Id>, N>, TimeBindingError>
    where
        Id: Clone + Eq + Hash,
        N: QuantityScalar,
    {
        timeline.encode(self.elapsed())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TimelineDefinitionError<Id> {
    #[error("atomic timeline basis must be greater than zero")]
    InvalidAtomicBasis,
    #[error("timeline basis is outside exact rational storage range")]
    NumericRange,
    #[error("timeline basis must resolve to a whole number of nanoseconds")]
    InexactNanosecondBasis,
    #[error(transparent)]
    Schema(#[from] AssetSchemaError<Id>),
}

#[derive(Debug, Error)]
pub enum TimeBindingError {
    #[error("elapsed time cannot become a negative economic quantity")]
    NegativeDuration,
    #[error("duration is not an exact multiple of the timeline asset's atomic basis")]
    InexactAtomicConversion,
    #[error("timeline count is outside the adapter or selected quantity backend's supported range")]
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
    use axionomy_units::si::rational64::Time;
    use axionomy_units::si::time::hour;
    use axionomy_units::{AssetSchema, Rational, UnitAsset};
    use jiff::{SignedDuration, Span, Zoned};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum AssetId {
        Elapsed,
    }

    #[test]
    fn timestamp_duration_lowers_into_timeline_assets() {
        let mut schema = AssetSchema::new();
        let timeline =
            TimelineAsset::define(&mut schema, AssetId::Elapsed, SignedDuration::from_hours(1))
                .unwrap();
        let amount: AssetAmount<UnitAsset<AssetId>> =
            timeline.encode(SignedDuration::from_hours(36)).unwrap();

        assert_eq!(amount.asset(), timeline.asset());
        assert_eq!(amount.quantity(), &Quantity::new(36));
    }

    #[test]
    fn calendar_days_respect_daylight_saving_transitions() {
        let opens: Zoned = "2024-03-09T12:00:00-05:00[America/New_York]"
            .parse()
            .unwrap();
        let window = CalendarWindow::from_span(opens, Span::new().days(1)).unwrap();
        let mut schema = AssetSchema::new();
        let timeline =
            TimelineAsset::define(&mut schema, AssetId::Elapsed, SignedDuration::from_hours(1))
                .unwrap();
        let amount: AssetAmount<UnitAsset<AssetId>> = window.encode(&timeline).unwrap();

        assert_eq!(amount.quantity(), &Quantity::new(23));
    }

    #[test]
    fn jiff_and_uom_reconstruct_the_same_time_asset() {
        let mut schema = AssetSchema::new();
        let measured = schema
            .define_measure(
                AssetId::Elapsed,
                Time::new::<hour>(Rational::from_integer(1)),
            )
            .unwrap();
        let timeline = TimelineAsset::from_schema(&schema, &AssetId::Elapsed).unwrap();

        assert_eq!(timeline.asset(), measured.asset());
        let amount: AssetAmount<UnitAsset<AssetId>> =
            timeline.encode(SignedDuration::from_hours(2)).unwrap();
        assert_eq!(amount.quantity(), &Quantity::new(2));
    }
}
