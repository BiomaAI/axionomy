//! What the store identifies: runs and groups by minted IDs, points by run
//! and step, and metadata by path.

use axionomy::{PathKind, path::Path};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{0}` is not an identifier this store mints")]
pub struct IdError(String);

macro_rules! id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        ///
        #[doc = concat!("Written `", $prefix, "` followed by 32 lowercase hexadecimal digits: the minting store's random 64-bit prefix, then its counter.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u128);

        impl $name {
            pub(crate) fn new(store: u64, counter: u64) -> Self {
                Self(u128::from(store) << 64 | u128::from(counter))
            }

            pub(crate) fn key(self) -> u128 {
                self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                text.strip_prefix($prefix)
                    .filter(|digits| {
                        digits.len() == 32
                            && digits.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
                    })
                    .and_then(|digits| u128::from_str_radix(digits, 16).ok())
                    .map(Self)
                    .ok_or_else(|| IdError(text.to_owned()))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, concat!($prefix, "{:032x}"), self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, formatter)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Cow::<str>::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

id!(
    /// A recorded run, unique across every store, so stores merge without
    /// conflicts.
    RunId,
    "run-"
);
id!(
    /// A group of runs or samples produced together, unique across every
    /// store.
    GroupId,
    "grp-"
);

impl RunId {
    pub(crate) fn from_key(key: u128) -> Self {
        Self(key)
    }
}

/// A state a run reached: the state after `step` exchanges, counting from the
/// first state of the run it forked from, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Point {
    pub run: RunId,
    pub step: u64,
}

impl Point {
    pub(crate) fn key(self) -> (u128, u64) {
        (self.run.key(), self.step)
    }
}

/// The kinds of metadata identified by path.
pub mod kind {
    use super::PathKind;

    pub enum Tag {}
    pub enum Metric {}
    pub enum Ref {}
    pub enum Group {}

    impl PathKind for Tag {
        const SCHEMA: &'static str = "TagPath";
    }
    impl PathKind for Metric {
        const SCHEMA: &'static str = "MetricPath";
    }
    impl PathKind for Ref {
        const SCHEMA: &'static str = "RefPath";
    }
    impl PathKind for Group {
        const SCHEMA: &'static str = "GroupKindPath";
    }
}

/// A tag's key, such as `experiment` or `split`.
pub type TagPath = Path<kind::Tag>;
/// A metric's name, such as `return` or `importance-weight`.
pub type MetricPath = Path<kind::Metric>;
/// A ref's name, such as `baseline` or `best/ppo`.
pub type RefPath = Path<kind::Ref>;
/// What produced a group, such as `monte-carlo`, `mppi` or `eval-suite`.
pub type GroupKindPath = Path<kind::Group>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_read_back_as_written() {
        let run = RunId::new(0xfeed, 7);
        assert_eq!(run.to_string(), "run-000000000000feed0000000000000007");
        assert_eq!(run.to_string().parse::<RunId>(), Ok(run));
        assert!(run.to_string().parse::<GroupId>().is_err());
        for forged in [
            "",
            "run-",
            "run-xyz",
            "RUN-000000000000feed0000000000000007",
        ] {
            assert!(forged.parse::<RunId>().is_err(), "{forged}");
        }
    }
}
