//! Handles the server mints for what it stores, and the path kind for
//! invariants.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, fmt, str::FromStr, sync::Arc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{0}` is not a handle this server issues")]
pub struct HandleError(String);

macro_rules! handle {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        ///
        #[doc = concat!("Written `", $prefix, "` followed by 64 lowercase hexadecimal digits: a keyed digest the server mints, opaque and infeasible to guess.")]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Arc<str>);

        impl $name {
            pub(crate) fn from_digest(digest: &blake3::Hash) -> Self {
                Self(format!(concat!($prefix, "{}"), digest.to_hex()).into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = HandleError;

            fn from_str(handle: &str) -> Result<Self, Self::Err> {
                let digest = handle.strip_prefix($prefix).unwrap_or_default();
                if digest.len() == 64 && digest.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
                    Ok(Self(handle.into()))
                } else {
                    Err(HandleError(handle.to_owned()))
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&*self.0, formatter)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Cow::<str>::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({
                    "description": concat!("A handle the server issued: `", $prefix, "` and 64 hexadecimal digits."),
                    "type": "string",
                    "pattern": concat!("^", $prefix, "[0-9a-f]{64}$")
                })
            }
        }
    };
}

handle!(
    /// A stored economy.
    EconomyId,
    "eco-"
);
handle!(
    /// A stored assessment of an exchange.
    AssessmentId,
    "asm-"
);
handle!(
    /// A stored study of a problem.
    StudyId,
    "stu-"
);
handle!(
    /// A ref the server made: a name for the economy an application is at,
    /// which moves only forward.
    RefId,
    "ref-"
);

/// What a kind of handle names, and how a client makes it again.
pub(crate) trait Handle: fmt::Display {
    /// What the handle names, in words.
    const NAMES: &'static str;
    /// The calls that make what the handle names.
    const MADE_AGAIN: &'static str;
}

impl Handle for EconomyId {
    const NAMES: &'static str = "economy";
    const MADE_AGAIN: &'static str = "make the economy again with the call that made it (axionomy_economy_create, axionomy_economy_extend, axionomy_exchange_apply, axionomy_trace_replay, axionomy_trace_step, axionomy_search, axionomy_problem_study, axionomy_package_list, axionomy_experiment_run, axionomy_experiment_results, axionomy_ref_read or axionomy_ref_frames)";
}

impl Handle for AssessmentId {
    const NAMES: &'static str = "assessment";
    const MADE_AGAIN: &'static str = "assess the exchange again with axionomy_exchange_assess";
}

impl Handle for StudyId {
    const NAMES: &'static str = "study";
    const MADE_AGAIN: &'static str = "study the problem again with axionomy_problem_study";
}

impl Handle for RefId {
    const NAMES: &'static str = "ref";
    const MADE_AGAIN: &'static str = "create a ref again with axionomy_ref_create";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_accept_only_what_the_server_mints() {
        let minted = EconomyId::from_digest(&blake3::hash(b"economy"));
        assert_eq!(minted.as_str().parse::<EconomyId>(), Ok(minted.clone()));
        assert!(minted.as_str().parse::<AssessmentId>().is_err());
        for forged in [
            "",
            "eco-",
            "eco-xyz",
            "ECO-00",
            &format!("eco-{}", "A".repeat(64)),
        ] {
            assert!(forged.parse::<EconomyId>().is_err(), "{forged}");
        }
    }
}
