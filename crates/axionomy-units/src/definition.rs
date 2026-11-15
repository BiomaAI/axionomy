use crate::MeasureScalar;
use axionomy::{
    AssetAmount, Quantity,
    path::{IdentifierError, SegmentReader, Segments},
};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use uom::si::{Dimension, marker};
use uom::typenum::Integer;

/// Stable runtime identity for the seven ISQ base-dimension exponents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DimensionSignature {
    length: i8,
    mass: i8,
    time: i8,
    electric_current: i8,
    thermodynamic_temperature: i8,
    amount_of_substance: i8,
    luminous_intensity: i8,
}

impl DimensionSignature {
    pub fn of<D>() -> Self
    where
        D: Dimension + ?Sized,
    {
        Self {
            length: D::L::I8,
            mass: D::M::I8,
            time: D::T::I8,
            electric_current: D::I::I8,
            thermodynamic_temperature: D::Th::I8,
            amount_of_substance: D::N::I8,
            luminous_intensity: D::J::I8,
        }
    }

    pub const fn exponents(self) -> [i8; 7] {
        [
            self.length,
            self.mass,
            self.time,
            self.electric_current,
            self.thermodynamic_temperature,
            self.amount_of_substance,
            self.luminous_intensity,
        ]
    }
}

/// Supplies a stable serialized identity for a `uom` quantity kind.
///
/// `uom` distinguishes quantities such as energy and torque even when their
/// dimension exponents match. Axionomy stores this identifier instead of an
/// unstable Rust type name. Custom `uom` kinds can implement this trait with a
/// namespaced, durable identifier: one path segment, such as `lab-viscosity`.
pub trait StableMeasureKind {
    const ID: &'static str;
}

impl StableMeasureKind for dyn uom::Kind {
    const ID: &'static str = "uom-plain";
}

macro_rules! stable_kinds {
    ($($kind:ident => $id:literal),+ $(,)?) => {
        $(impl StableMeasureKind for dyn marker::$kind {
            const ID: &'static str = $id;
        })+
    };
}

stable_kinds! {
    AngleKind => "uom-angle",
    SolidAngleKind => "uom-solid-angle",
    InformationKind => "uom-information",
    TemperatureKind => "uom-temperature",
    ConstituentConcentrationKind => "uom-constituent-concentration",
    SurfaceTensionKind => "uom-surface-tension",
    KinematicViscosityKind => "uom-kinematic-viscosity",
    IlluminanceKind => "uom-illuminance",
}

/// Stable identity for a dimensionally distinct `uom` quantity kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct MeasureKind(String);

impl MeasureKind {
    pub fn of<K>() -> Self
    where
        K: StableMeasureKind + ?Sized,
    {
        Self(K::ID.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn try_new(id: String) -> Result<Self, MeasureDefinitionError> {
        let valid = id.len() <= 128 && !id.contains('/') && axionomy::path::is_path(&id);
        if valid {
            Ok(Self(id))
        } else {
            Err(MeasureDefinitionError::InvalidKind)
        }
    }
}

impl<'de> Deserialize<'de> for MeasureKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Exact physical meaning of one atom of a measured asset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct MeasureDefinition {
    dimension: DimensionSignature,
    kind: MeasureKind,
    atomic_base_value: MeasureScalar,
}

impl MeasureDefinition {
    pub(crate) fn new(
        dimension: DimensionSignature,
        kind: MeasureKind,
        atomic_base_value: MeasureScalar,
    ) -> Result<Self, MeasureDefinitionError> {
        MeasureKind::try_new(kind.0.clone())?;
        if atomic_base_value <= MeasureScalar::from_integer(0) {
            return Err(MeasureDefinitionError::InvalidAtomicBasis);
        }
        Ok(Self {
            dimension,
            kind,
            atomic_base_value,
        })
    }

    pub const fn dimension(&self) -> DimensionSignature {
        self.dimension
    }

    pub const fn kind(&self) -> &MeasureKind {
        &self.kind
    }

    pub const fn atomic_base_value(&self) -> &MeasureScalar {
        &self.atomic_base_value
    }
}

#[derive(Deserialize)]
struct MeasureDefinitionData {
    dimension: DimensionSignature,
    kind: MeasureKind,
    atomic_base_value: MeasureScalar,
}

impl<'de> Deserialize<'de> for MeasureDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = MeasureDefinitionData::deserialize(deserializer)?;
        Self::new(data.dimension, data.kind, data.atomic_base_value)
            .map_err(serde::de::Error::custom)
    }
}

/// Largest decimal precision whose whole display unit still fits one `u64`
/// atom count (`10^18 < u64::MAX < 10^19`).
pub const MAX_DECIMALS: u8 = 18;

/// Fixed decimal precision of a currency-like asset.
///
/// One atom is `10^-decimals` of a whole display unit, matching token
/// `decimals` metadata. Unlike that metadata, the precision participates in
/// the asset identity: a two-decimal and a six-decimal amount of the same
/// logical asset can never alias in an account or basket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct DecimalPrecision {
    decimals: u8,
}

impl DecimalPrecision {
    pub const fn new(decimals: u8) -> Result<Self, DecimalPrecisionError> {
        if decimals > MAX_DECIMALS {
            return Err(DecimalPrecisionError::TooManyDecimals { decimals });
        }
        Ok(Self { decimals })
    }

    pub const fn decimals(self) -> u8 {
        self.decimals
    }

    /// Number of atoms in one whole display unit.
    pub const fn atoms_per_unit(self) -> u64 {
        10_u64.pow(self.decimals as u32)
    }
}

#[derive(Deserialize)]
struct DecimalPrecisionData {
    decimals: u8,
}

impl<'de> Deserialize<'de> for DecimalPrecision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = DecimalPrecisionData::deserialize(deserializer)?;
        Self::new(data.decimals).map_err(serde::de::Error::custom)
    }
}

/// The authoritative atomic definition carried by a unit-aware asset key.
///
/// New variants are appended so existing keys keep their relative order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AssetDefinition {
    Discrete,
    Measured(MeasureDefinition),
    Decimal(DecimalPrecision),
}

/// A logical asset identity paired with its canonical atomic definition.
///
/// The definition participates in equality, ordering, hashing, and Serde. Two
/// bindings with different dimensions, kinds, or atomic bases therefore can
/// never silently alias in an account or basket.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(bound(serialize = "Id: Serialize", deserialize = "Id: Deserialize<'de>"))]
pub struct UnitAsset<Id> {
    id: Id,
    definition: AssetDefinition,
}

impl<Id> UnitAsset<Id> {
    pub(crate) const fn new(id: Id, definition: AssetDefinition) -> Self {
        Self { id, definition }
    }

    pub const fn id(&self) -> &Id {
        &self.id
    }

    pub const fn definition(&self) -> &AssetDefinition {
        &self.definition
    }

    /// Creates an amount from an already explicit quantity of asset atoms.
    pub fn amount(&self, atoms: Quantity) -> AssetAmount<Self>
    where
        Id: Clone,
    {
        AssetAmount::new(self.clone(), atoms)
    }

    /// Creates an ordinary `u64` amount of explicitly counted asset atoms.
    pub fn atoms(&self, atoms: u64) -> AssetAmount<Self>
    where
        Id: Clone,
    {
        self.amount(Quantity::new(atoms))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DecimalPrecisionError {
    #[error("decimal precision {decimals} exceeds the supported maximum of 18")]
    TooManyDecimals { decimals: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MeasureDefinitionError {
    #[error("atomic physical basis must be greater than zero")]
    InvalidAtomicBasis,
    #[error("measurement kind identifier must be one path segment of at most 128 characters")]
    InvalidKind,
}

/// An exponent, a non-negative one as itself and a negative one as `n` and
/// its magnitude, so every exponent is one path segment.
fn write_exponent(exponent: i8, segments: &mut Vec<String>) {
    segments.push(if exponent < 0 {
        format!("n{}", exponent.unsigned_abs())
    } else {
        exponent.to_string()
    });
}

fn read_exponent(segments: &mut SegmentReader<'_>) -> Result<i8, IdentifierError> {
    let segment = segments.segment()?;
    let unexpected = || IdentifierError::Unexpected(segment.to_owned());
    let (negative, magnitude) = match segment.strip_prefix('n') {
        Some(magnitude) => (true, magnitude),
        None => (false, segment),
    };
    let canonical = magnitude.bytes().all(|byte| byte.is_ascii_digit())
        && (magnitude == "0" && !negative || !magnitude.starts_with('0'));
    let magnitude: u8 = canonical
        .then(|| magnitude.parse().ok())
        .flatten()
        .ok_or_else(unexpected)?;
    let exponent = i16::from(magnitude) * if negative { -1 } else { 1 };
    i8::try_from(exponent).map_err(|_| unexpected())
}

/// A unit asset's path: its definition, then its identity's own path, last
/// so an identity of any length still reads back. A definition is
/// `discrete`, `decimal` and its decimals, or `measured`, its seven
/// dimension exponents, its atomic base value as numerator and denominator,
/// and its kind: `measured/1/0/n2/0/0/0/0/1/1000/uom-plain/fuel`.
impl<Id: Segments> Segments for UnitAsset<Id> {
    fn write_segments(&self, segments: &mut Vec<String>) {
        match &self.definition {
            AssetDefinition::Discrete => segments.push("discrete".into()),
            AssetDefinition::Decimal(precision) => {
                segments.push("decimal".into());
                precision.decimals.write_segments(segments);
            }
            AssetDefinition::Measured(measure) => {
                segments.push("measured".into());
                for exponent in measure.dimension.exponents() {
                    write_exponent(exponent, segments);
                }
                // A positive basis, so both parts are positive.
                measure
                    .atomic_base_value
                    .numer()
                    .unsigned_abs()
                    .write_segments(segments);
                measure
                    .atomic_base_value
                    .denom()
                    .unsigned_abs()
                    .write_segments(segments);
                segments.push(measure.kind.0.clone());
            }
        }
        self.id.write_segments(segments);
    }

    fn read_segments(segments: &mut SegmentReader<'_>) -> Result<Self, IdentifierError> {
        let definition = match segments.segment()? {
            "discrete" => AssetDefinition::Discrete,
            "decimal" => {
                let decimals = u8::read_segments(segments)?;
                AssetDefinition::Decimal(
                    DecimalPrecision::new(decimals)
                        .map_err(|error| IdentifierError::Unexpected(error.to_string()))?,
                )
            }
            "measured" => {
                let mut exponents = [0; 7];
                for exponent in &mut exponents {
                    *exponent = read_exponent(segments)?;
                }
                let [
                    length,
                    mass,
                    time,
                    electric_current,
                    thermodynamic_temperature,
                    amount_of_substance,
                    luminous_intensity,
                ] = exponents;
                let dimension = DimensionSignature {
                    length,
                    mass,
                    time,
                    electric_current,
                    thermodynamic_temperature,
                    amount_of_substance,
                    luminous_intensity,
                };
                let part = |segments: &mut SegmentReader<'_>| {
                    let value = u64::read_segments(segments)?;
                    i64::try_from(value).map_err(|_| IdentifierError::Unexpected(value.to_string()))
                };
                let numerator = part(segments)?;
                let denominator = part(segments)?;
                if denominator == 0 {
                    return Err(IdentifierError::Unexpected("0".into()));
                }
                let kind = MeasureKind::try_new(segments.segment()?.to_owned())
                    .map_err(|error| IdentifierError::Unexpected(error.to_string()))?;
                AssetDefinition::Measured(
                    MeasureDefinition::new(
                        dimension,
                        kind,
                        MeasureScalar::new(numerator, denominator),
                    )
                    .map_err(|error| IdentifierError::Unexpected(error.to_string()))?,
                )
            }
            other => return Err(IdentifierError::Unexpected(other.to_owned())),
        };
        Ok(Self {
            id: Id::read_segments(segments)?,
            definition,
        })
    }
}
