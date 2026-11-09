use crate::{
    AssetDefinition, DecimalAsset, DecimalPrecision, DecimalPrecisionError, DimensionSignature,
    MeasureDefinition, MeasureDefinitionError, MeasureKind, MeasureScalar, MeasuredAsset,
    StableMeasureKind, UnitAsset,
};
use axionomy::{Economy, EconomyBuilder, Goal, ModelBuildError, Ontology};
use indexmap::IndexMap;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::Hash;
use thiserror::Error;
use uom::Conversion;
use uom::si::{Dimension, Quantity as UomQuantity, Units};

pub type UnitModelBuildResult<O, Id> = Result<Economy<O>, UnitModelBuildError<O, Id>>;

/// Declares the one canonical atomic definition for every logical asset ID.
#[derive(Debug, Clone)]
pub struct AssetSchema<Id> {
    definitions: IndexMap<Id, AssetDefinition>,
}

impl<Id> AssetSchema<Id> {
    pub fn new() -> Self {
        Self {
            definitions: IndexMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn definition(&self, id: &Id) -> Option<&AssetDefinition>
    where
        Id: Eq + Hash,
    {
        self.definitions.get(id)
    }
}

impl<Id> AssetSchema<Id>
where
    Id: Clone + Eq + Hash,
{
    /// Reconstructs a coherent schema from self-describing asset keys.
    ///
    /// Repeated equal keys are expected across accounts and rates. Reusing one
    /// logical ID with a different definition is rejected.
    pub fn from_assets<'a>(
        assets: impl IntoIterator<Item = &'a UnitAsset<Id>>,
    ) -> Result<Self, AssetSchemaError<Id>>
    where
        Id: 'a,
    {
        let mut schema = Self::new();
        for asset in assets {
            match schema.definitions.get(asset.id()) {
                None => {
                    schema
                        .definitions
                        .insert(asset.id().clone(), asset.definition().clone());
                }
                Some(existing) if existing == asset.definition() => {}
                Some(existing) => {
                    return Err(AssetSchemaError::ConflictingDefinition {
                        id: asset.id().clone(),
                        existing: existing.clone(),
                        proposed: asset.definition().clone(),
                    });
                }
            }
        }
        Ok(schema)
    }

    /// Declares a countable asset with no physical dimension.
    pub fn define_discrete(&mut self, id: Id) -> Result<UnitAsset<Id>, AssetSchemaError<Id>> {
        self.insert(id, AssetDefinition::Discrete)
    }

    /// Declares a currency-like asset whose atom is `10^-decimals` of a unit.
    ///
    /// The precision is part of the asset key, so the same logical ID cannot
    /// later be declared or deserialized with a different number of decimals.
    pub fn define_decimal(
        &mut self,
        id: Id,
        decimals: u8,
    ) -> Result<DecimalAsset<Id>, AssetSchemaError<Id>> {
        let precision = DecimalPrecision::new(decimals)?;
        let asset = self.insert(id, AssetDefinition::Decimal(precision))?;
        Ok(DecimalAsset::new(asset, precision))
    }

    /// Reconstructs a typed handle for an existing decimal definition.
    pub fn decimal(&self, id: &Id) -> Result<DecimalAsset<Id>, AssetSchemaError<Id>> {
        let Some(found) = self.definitions.get(id) else {
            return Err(AssetSchemaError::UnknownAsset { id: id.clone() });
        };
        let AssetDefinition::Decimal(precision) = found else {
            return Err(AssetSchemaError::ExpectedDecimalAsset { id: id.clone() });
        };
        Ok(DecimalAsset::new(
            UnitAsset::new(id.clone(), found.clone()),
            *precision,
        ))
    }

    /// Declares a measured asset and returns its dimensionally typed handle.
    pub fn define_measure<D, U>(
        &mut self,
        id: Id,
        atomic: UomQuantity<D, U, MeasureScalar>,
    ) -> Result<MeasuredAsset<Id, D, U>, AssetSchemaError<Id>>
    where
        D: Dimension + ?Sized,
        D::Kind: StableMeasureKind,
        U: Units<MeasureScalar> + ?Sized,
        MeasureScalar: Conversion<MeasureScalar>,
    {
        let definition = MeasureDefinition::new(
            DimensionSignature::of::<D>(),
            MeasureKind::of::<D::Kind>(),
            atomic.value,
        )?;
        let asset = self.insert(id, AssetDefinition::Measured(definition.clone()))?;
        Ok(MeasuredAsset::new(asset, &definition))
    }

    /// Reconstructs a typed handle for an existing measured definition.
    pub fn measured<D, U>(&self, id: &Id) -> Result<MeasuredAsset<Id, D, U>, AssetSchemaError<Id>>
    where
        D: Dimension + ?Sized,
        D::Kind: StableMeasureKind,
        U: Units<MeasureScalar> + ?Sized,
        MeasureScalar: Conversion<MeasureScalar>,
    {
        let Some(found) = self.definitions.get(id) else {
            return Err(AssetSchemaError::UnknownAsset { id: id.clone() });
        };
        let AssetDefinition::Measured(definition) = found else {
            return Err(AssetSchemaError::ExpectedMeasuredAsset { id: id.clone() });
        };
        let expected_dimension = DimensionSignature::of::<D>();
        let expected_kind = MeasureKind::of::<D::Kind>();
        if definition.dimension() != expected_dimension || definition.kind() != &expected_kind {
            return Err(AssetSchemaError::MeasurementTypeMismatch { id: id.clone() });
        }
        Ok(MeasuredAsset::new(
            UnitAsset::new(id.clone(), found.clone()),
            definition,
        ))
    }

    pub fn validate_asset(&self, asset: &UnitAsset<Id>) -> Result<(), AssetSchemaError<Id>> {
        match self.definitions.get(asset.id()) {
            None => Err(AssetSchemaError::UnknownAsset {
                id: asset.id().clone(),
            }),
            Some(expected) if expected != asset.definition() => {
                Err(AssetSchemaError::ConflictingDefinition {
                    id: asset.id().clone(),
                    existing: expected.clone(),
                    proposed: asset.definition().clone(),
                })
            }
            Some(_) => Ok(()),
        }
    }

    pub fn validate_goal<O: Ontology<Asset = UnitAsset<Id>>>(
        &self,
        goal: &Goal<O>,
    ) -> Result<(), AssetSchemaError<Id>>
    where
        Id: Ord,
    {
        for asset in goal.asset_keys() {
            self.validate_asset(asset)?;
        }
        Ok(())
    }

    pub fn validate_economy<O: Ontology<Asset = UnitAsset<Id>>>(
        &self,
        economy: &Economy<O>,
    ) -> Result<(), AssetSchemaError<Id>>
    where
        Id: Ord,
    {
        for asset in economy.asset_keys() {
            self.validate_asset(asset)?;
        }
        Ok(())
    }

    /// Reconstructs and validates the complete schema embedded in an economy.
    pub fn from_economy<O: Ontology<Asset = UnitAsset<Id>>>(
        economy: &Economy<O>,
    ) -> Result<Self, AssetSchemaError<Id>>
    where
        Id: Ord,
    {
        Self::from_assets(economy.asset_keys())
    }

    pub fn build_economy<O: Ontology<Asset = UnitAsset<Id>>>(
        &self,
        builder: EconomyBuilder<O>,
    ) -> UnitModelBuildResult<O, Id>
    where
        Id: Ord,
    {
        let economy = builder.build()?;
        self.validate_economy(&economy)?;
        Ok(economy)
    }

    fn insert(
        &mut self,
        id: Id,
        definition: AssetDefinition,
    ) -> Result<UnitAsset<Id>, AssetSchemaError<Id>> {
        if let Some(existing) = self.definitions.get(&id) {
            if existing == &definition {
                return Err(AssetSchemaError::DuplicateAsset { id });
            }
            return Err(AssetSchemaError::ConflictingDefinition {
                id,
                existing: existing.clone(),
                proposed: definition,
            });
        }
        self.definitions.insert(id.clone(), definition.clone());
        Ok(UnitAsset::new(id, definition))
    }
}

impl<Id> Default for AssetSchema<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> Serialize for AssetSchema<Id>
where
    Id: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.len()))?;
        for entry in &self.definitions {
            sequence.serialize_element(&entry)?;
        }
        sequence.end()
    }
}

impl<'de, Id> Deserialize<'de> for AssetSchema<Id>
where
    Id: Deserialize<'de> + Clone + Eq + Hash,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(Id, AssetDefinition)>::deserialize(deserializer)?;
        let mut schema = Self::new();
        for (id, definition) in entries {
            schema
                .insert(id, definition)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(schema)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssetSchemaError<Id> {
    #[error("asset schema contains a duplicate logical identifier")]
    DuplicateAsset { id: Id },
    #[error("one logical asset cannot have conflicting atomic definitions")]
    ConflictingDefinition {
        id: Id,
        existing: AssetDefinition,
        proposed: AssetDefinition,
    },
    #[error("asset is not declared by this schema")]
    UnknownAsset { id: Id },
    #[error("asset is not measured but a measured binding was requested")]
    ExpectedMeasuredAsset { id: Id },
    #[error("asset is not decimal but a decimal binding was requested")]
    ExpectedDecimalAsset { id: Id },
    #[error("requested measurement type does not match the asset definition")]
    MeasurementTypeMismatch { id: Id },
    #[error(transparent)]
    InvalidDefinition(#[from] MeasureDefinitionError),
    #[error(transparent)]
    InvalidPrecision(#[from] DecimalPrecisionError),
}

#[derive(Debug, Error)]
pub enum UnitModelBuildError<O: Ontology, Id> {
    #[error(transparent)]
    Model(#[from] ModelBuildError<O>),
    #[error(transparent)]
    Schema(#[from] AssetSchemaError<Id>),
}
