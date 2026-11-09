//! Identifier paths such as `loans/42` or `agents/7/wallet`, with one distinct
//! type per kind of thing identified.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    str::FromStr,
    sync::Arc,
};
use thiserror::Error;

/// What a [`Path`] identifies. Each kind is a distinct type, so an asset path
/// can never stand where an account path is expected.
pub trait PathKind {
    /// The JSON Schema title of paths of this kind, such as `AccountPath`.
    const SCHEMA: &'static str;
}

/// The kinds of identifiers in an economy.
pub mod kind {
    use super::PathKind;

    pub enum Account {}
    pub enum Asset {}
    pub enum Rate {}
    pub enum Role {}

    impl PathKind for Account {
        const SCHEMA: &'static str = "AccountPath";
    }
    impl PathKind for Asset {
        const SCHEMA: &'static str = "AssetPath";
    }
    impl PathKind for Rate {
        const SCHEMA: &'static str = "RatePath";
    }
    impl PathKind for Role {
        const SCHEMA: &'static str = "RolePath";
    }
}

pub type AccountPath = Path<kind::Account>;
pub type AssetPath = Path<kind::Asset>;
pub type RatePath = Path<kind::Rate>;
pub type RolePath = Path<kind::Role>;

/// An identifier written as a path: one or more segments separated by `/`,
/// such as `loans/42`, `agents/7/wallet` or `market/order-book`.
///
/// A segment is made of lowercase `a`–`z`, digits and `-`, and starts and ends
/// with a letter or digit. Paths therefore need no escaping in URIs, HTTP
/// headers, JSON or command lines, and leave characters such as `*` free for
/// selection patterns. The hierarchy is for people and tools; it never decides
/// whether an exchange is valid.
pub struct Path<K>(Arc<str>, PhantomData<fn() -> K>);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PathError {
    #[error("a path has at least one segment")]
    Empty,
    #[error("segment {segment} of a path is empty")]
    EmptySegment { segment: usize },
    #[error("`{character}` is not allowed in a path; use a–z, 0–9, `-` and `/`")]
    InvalidCharacter { character: char },
    #[error("segment {segment} of a path starts or ends with `-`")]
    HyphenAtEdge { segment: usize },
}

impl<K> Path<K> {
    fn new(path: Arc<str>) -> Self {
        Self(path, PhantomData)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// The path one level up: `loans` for `loans/42`, none for `loans`.
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once('/')
            .map(|(parent, _)| Self::new(parent.into()))
    }

    /// The path one level down: `loans/42` for `loans` and `42`.
    pub fn child(&self, segment: &str) -> Result<Self, PathError> {
        check_segment(segment, self.segments().count())?;
        Ok(Self::new(format!("{}/{segment}", self.0).into()))
    }
}

fn check_segment(segment: &str, index: usize) -> Result<(), PathError> {
    if segment.is_empty() {
        return Err(PathError::EmptySegment { segment: index });
    }
    if let Some(character) = segment
        .chars()
        .find(|character| !matches!(character, 'a'..='z' | '0'..='9' | '-'))
    {
        return Err(PathError::InvalidCharacter { character });
    }
    if segment.starts_with('-') || segment.ends_with('-') {
        return Err(PathError::HyphenAtEdge { segment: index });
    }
    Ok(())
}

impl<K> FromStr for Path<K> {
    type Err = PathError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        if path.is_empty() {
            return Err(PathError::Empty);
        }
        for (index, segment) in path.split('/').enumerate() {
            check_segment(segment, index)?;
        }
        Ok(Self::new(path.into()))
    }
}

impl<K> Clone for Path<K> {
    fn clone(&self) -> Self {
        Self::new(Arc::clone(&self.0))
    }
}

impl<K> PartialEq for Path<K> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K> Eq for Path<K> {}

impl<K> PartialOrd for Path<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> Ord for Path<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<K> Hash for Path<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<K> fmt::Display for Path<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<K> fmt::Debug for Path<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, formatter)
    }
}

impl<K> Serialize for Path<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de, K> Deserialize<'de> for Path<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let path = Cow::<str>::deserialize(deserializer)?;
        path.parse().map_err(serde::de::Error::custom)
    }
}

impl<K: PathKind> JsonSchema for Path<K> {
    fn schema_name() -> Cow<'static, str> {
        K::SCHEMA.into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Lowercase segments of a–z, 0–9 and `-`, separated by `/`, such as `loans/42`; each segment starts and ends with a letter or digit.",
            "type": "string",
            "pattern": "^[a-z0-9]([a-z0-9-]*[a-z0-9])?(/[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_lowercase_slug_segments() {
        for path in [
            "a",
            "loans/42",
            "agents/7/wallet",
            "market/order-book",
            "synth/ch-10",
        ] {
            assert_eq!(path.parse::<AccountPath>().unwrap().as_str(), path);
        }
        for (path, error) in [
            ("", PathError::Empty),
            ("loans/", PathError::EmptySegment { segment: 1 }),
            ("/loans", PathError::EmptySegment { segment: 0 }),
            ("a//b", PathError::EmptySegment { segment: 1 }),
            ("Loans", PathError::InvalidCharacter { character: 'L' }),
            ("order_book", PathError::InvalidCharacter { character: '_' }),
            ("a/./b", PathError::InvalidCharacter { character: '.' }),
            ("agents/*", PathError::InvalidCharacter { character: '*' }),
            ("-a", PathError::HyphenAtEdge { segment: 0 }),
            ("a/b-", PathError::HyphenAtEdge { segment: 1 }),
        ] {
            assert_eq!(path.parse::<AccountPath>(), Err(error), "{path}");
        }
    }

    #[test]
    fn parents_and_children_stay_paths() {
        let loans: AccountPath = "loans".parse().unwrap();
        let loan = loans.child("42").unwrap();
        assert_eq!(loan.as_str(), "loans/42");
        assert_eq!(loan.parent(), Some(loans.clone()));
        assert_eq!(loans.parent(), None);
        assert!(loans.child("4/2").is_err());
        assert_eq!(serde_json::to_string(&loan).unwrap(), "\"loans/42\"");
        assert!(serde_json::from_str::<AccountPath>("\"Loans\"").is_err());
    }
}
