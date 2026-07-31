use crate::{
    AssetDefinition, DimensionSignature, MeasureDefinition, MeasureDefinitionError, MeasureKind,
    MeasureScalar, MeasuredAsset, StableMeasureKind, UnitAsset,
};
use axionomy::{Economy, EconomyBuilder, Goal, ModelBuildError, QuantityScalar};
use indexmap::IndexMap;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::Hash;
use thiserror::Error;
use uom::Conversion;
use uom::si::{Dimension, Quantity as UomQuantity, Units};

pub type UnitEconomy<AccountId, Id, RateId, Role, N = u64> =
    Economy<AccountId, UnitAsset<Id>, RateId, Role, N>;

pub type UnitModelBuildResult<AccountId, Id, RateId, Role, N = u64> =
    Result<UnitEconomy<AccountId, Id, RateId, Role, N>, UnitModelBuildError<AccountId, RateId, Id>>;

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
    /// Declares a countable asset with no physical dimension.
    pub fn define_discrete(&mut self, id: Id) -> Result<UnitAsset<Id>, AssetSchemaError<Id>> {
        self.insert(id, AssetDefinition::Discrete)
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

    pub fn validate_goal<AccountId, N>(
        &self,
        goal: &Goal<AccountId, UnitAsset<Id>, N>,
    ) -> Result<(), AssetSchemaError<Id>>
    where
        AccountId: Ord,
        Id: Ord,
    {
        for asset in goal.asset_keys() {
            self.validate_asset(asset)?;
        }
        Ok(())
    }

    pub fn validate_economy<AccountId, RateId, Role, N>(
        &self,
        economy: &Economy<AccountId, UnitAsset<Id>, RateId, Role, N>,
    ) -> Result<(), AssetSchemaError<Id>>
    where
        AccountId: Clone + Eq + Hash + Ord,
        Id: Ord,
        RateId: Clone + Eq + Hash + Ord,
        Role: Clone + Ord,
        N: QuantityScalar,
    {
        for asset in economy.asset_keys() {
            self.validate_asset(asset)?;
        }
        Ok(())
    }

    pub fn build_economy<AccountId, RateId, Role, N>(
        &self,
        builder: EconomyBuilder<AccountId, UnitAsset<Id>, RateId, Role, N>,
    ) -> UnitModelBuildResult<AccountId, Id, RateId, Role, N>
    where
        AccountId: Clone + Eq + Hash + Ord,
        Id: Ord,
        RateId: Clone + Eq + Hash + Ord,
        Role: Clone + Ord,
        N: QuantityScalar,
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
    #[error("asset is discrete but a measured binding was requested")]
    ExpectedMeasuredAsset { id: Id },
    #[error("requested measurement type does not match the asset definition")]
    MeasurementTypeMismatch { id: Id },
    #[error(transparent)]
    InvalidDefinition(#[from] MeasureDefinitionError),
}

#[derive(Debug, Error)]
pub enum UnitModelBuildError<AccountId, RateId, Id> {
    #[error(transparent)]
    Model(#[from] ModelBuildError<AccountId, RateId>),
    #[error(transparent)]
    Schema(#[from] AssetSchemaError<Id>),
}
