use crate::MeasureScalar;
use axionomy::{AssetAmount, Quantity};
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
/// namespaced, durable identifier.
pub trait StableMeasureKind {
    const ID: &'static str;
}

impl StableMeasureKind for dyn uom::Kind {
    const ID: &'static str = "uom:plain";
}

macro_rules! stable_kinds {
    ($($kind:ident => $id:literal),+ $(,)?) => {
        $(impl StableMeasureKind for dyn marker::$kind {
            const ID: &'static str = $id;
        })+
    };
}

stable_kinds! {
    AngleKind => "uom:angle",
    SolidAngleKind => "uom:solid-angle",
    InformationKind => "uom:information",
    TemperatureKind => "uom:temperature",
    ConstituentConcentrationKind => "uom:constituent-concentration",
    SurfaceTensionKind => "uom:surface-tension",
    KinematicViscosityKind => "uom:kinematic-viscosity",
    IlluminanceKind => "uom:illuminance",
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
        let valid =
            !id.is_empty() && id.len() <= 128 && id.bytes().all(|byte| byte.is_ascii_graphic());
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

/// The authoritative atomic definition carried by a unit-aware asset key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AssetDefinition {
    Discrete,
    Measured(MeasureDefinition),
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
    pub fn amount<N>(&self, atoms: Quantity<N>) -> AssetAmount<Self, N>
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
pub enum MeasureDefinitionError {
    #[error("atomic physical basis must be greater than zero")]
    InvalidAtomicBasis,
    #[error("measurement kind identifier must be 1-128 printable ASCII characters")]
    InvalidKind,
}
