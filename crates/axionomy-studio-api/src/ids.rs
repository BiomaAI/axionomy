//! How Studio writes what JSON cannot carry as it is: a point, a 64-bit
//! number, a package's version and a list of objectives in a query.

use axionomy_engine::{ObjectivePath, Version};
use axionomy_store::{Point, RunId};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, fmt, str::FromStr};

/// A point of the engine's store: the state after `step` steps of the
/// history of a run, written `run-`, 32 hexadecimal digits, `:` and the
/// step, such as `run-000000000000feed0000000000000007:3`. A point has one
/// name, in the run whose step reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointId(Point);

impl PointId {
    pub fn point(self) -> Point {
        self.0
    }
}

impl From<Point> for PointId {
    fn from(point: Point) -> Self {
        Self(point)
    }
}

/// Text that is not a point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointIdError(String);

impl fmt::Display for PointIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is not a point such as `run-000000000000feed0000000000000007:3`",
            self.0
        )
    }
}

impl std::error::Error for PointIdError {}

impl FromStr for PointId {
    type Err = PointIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || PointIdError(text.to_owned());
        let (run, step) = text.split_once(':').ok_or_else(malformed)?;
        // One spelling per point: decimal digits, without leading zeros.
        if !decimal(step) {
            return Err(malformed());
        }
        Ok(Self(Point {
            run: run.parse::<RunId>().map_err(|_| malformed())?,
            step: step.parse().map_err(|_| malformed())?,
        }))
    }
}

impl fmt::Display for PointId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.0.run, self.0.step)
    }
}

impl Serialize for PointId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PointId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for PointId {
    fn schema_name() -> Cow<'static, str> {
        "PointId".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A point of the engine's store: the state after `step` steps of a run's history, written `run-`, 32 hexadecimal digits, `:` and the step. A point has one name, in the run whose step reached it.",
            "type": "string",
            "pattern": "^run-[0-9a-f]{32}:(0|[1-9][0-9]*)$"
        })
    }
}

/// A whole number below 2^64, such as a seed or a budget, written in
/// decimal: JSON's numbers cannot carry every such number exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Natural(pub u64);

impl From<u64> for Natural {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for Natural {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for Natural {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Natural {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        if !decimal(&text) {
            return Err(serde::de::Error::custom(format!(
                "`{text}` is not a whole number written in decimal, without leading zeros"
            )));
        }
        text.parse()
            .map(Self)
            .map_err(|_| serde::de::Error::custom(format!("`{text}` is 2^64 or more")))
    }
}

impl JsonSchema for Natural {
    fn schema_name() -> Cow<'static, str> {
        "Natural".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A whole number below 2^64, such as a seed or a budget, written in decimal, since JSON numbers cannot carry every such number exactly.",
            "type": "string",
            "pattern": "^(0|[1-9][0-9]*)$"
        })
    }
}

/// Digits, without a leading zero unless the number is zero.
fn decimal(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text.len() == 1 || !text.starts_with('0'))
}

/// A package's version, written `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageVersion(pub Version);

impl Serialize for PackageVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PackageVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for PackageVersion {
    fn schema_name() -> Cow<'static, str> {
        "PackageVersion".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A package's version: three whole numbers, `major.minor.patch`, such as `1.0.2`.",
            "type": "string",
            "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$"
        })
    }
}

/// Objective paths separated by commas, as a query writes a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectiveList(pub Vec<ObjectivePath>);

impl<'de> Deserialize<'de> for ObjectiveList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        if text.is_empty() {
            return Ok(Self::default());
        }
        text.split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl Serialize for ObjectiveList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let written: Vec<&str> = self.0.iter().map(|objective| objective.as_str()).collect();
        serializer.serialize_str(&written.join(","))
    }
}

impl JsonSchema for ObjectiveList {
    fn schema_name() -> Cow<'static, str> {
        "ObjectiveList".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Objective paths separated by commas, such as `paid,payments`.",
            "type": "string"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_read_back_as_written() {
        let run: RunId = "run-000000000000feed0000000000000007".parse().unwrap();
        let point = PointId(Point { run, step: 12 });
        assert_eq!(point.to_string(), "run-000000000000feed0000000000000007:12");
        assert_eq!(point.to_string().parse::<PointId>(), Ok(point));
        for forged in [
            "",
            "run-000000000000feed0000000000000007",
            "run-000000000000feed0000000000000007:",
            "run-000000000000feed0000000000000007:012",
            "run-000000000000feed0000000000000007:+1",
            "run-000000000000feed0000000000000007:-1",
            "run-feed:1",
        ] {
            assert!(forged.parse::<PointId>().is_err(), "{forged}");
        }
    }

    #[test]
    fn a_natural_is_every_64_bit_number_written_once() {
        let largest: Natural = serde_json::from_str("\"18446744073709551615\"").unwrap();
        assert_eq!(largest, Natural(u64::MAX));
        assert_eq!(
            serde_json::to_string(&largest).unwrap(),
            "\"18446744073709551615\""
        );
        for forged in [
            "\"18446744073709551616\"",
            "\"017\"",
            "\"-1\"",
            "\"\"",
            "\"1e3\"",
            "17",
        ] {
            assert!(serde_json::from_str::<Natural>(forged).is_err(), "{forged}");
        }
    }
}
