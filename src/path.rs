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
    pub enum Ontology {}
    pub enum Parameter {}
    pub enum Condition {}
    pub enum Invariant {}

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
    impl PathKind for Ontology {
        const SCHEMA: &'static str = "OntologyPath";
    }
    impl PathKind for Parameter {
        const SCHEMA: &'static str = "ParameterPath";
    }
    impl PathKind for Condition {
        const SCHEMA: &'static str = "ConditionPath";
    }
    impl PathKind for Invariant {
        const SCHEMA: &'static str = "InvariantPath";
    }
}

pub type AccountPath = Path<kind::Account>;
pub type AssetPath = Path<kind::Asset>;
pub type RatePath = Path<kind::Rate>;
pub type RolePath = Path<kind::Role>;
/// The name of an [`Ontology`](crate::Ontology), such as `maze` or
/// `logistics/fleet`.
pub type OntologyPath = Path<kind::Ontology>;
/// The name of a parameter an exchange supplies to its rate, such as
/// `minimum-output`.
pub type ParameterPath = Path<kind::Parameter>;
/// The name of a rate's condition, such as `within-loan-to-value-limit`.
pub type ConditionPath = Path<kind::Condition>;
/// The name of a declared invariant, such as `money-accounting`.
pub type InvariantPath = Path<kind::Invariant>;

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
        if segment.contains('/') {
            return Err(PathError::InvalidCharacter { character: '/' });
        }
        let child = format!("{}/{segment}", self.0);
        check(&child)?;
        Ok(Self::new(child.into()))
    }
}

impl OntologyPath {
    /// The name of the ontology `O`. A name that is not a path fails to
    /// compile wherever this is called.
    pub fn of<O: crate::Ontology>() -> Self {
        const { assert!(check(O::NAME).is_ok(), "an ontology's NAME must be a path") };
        Self::new(O::NAME.into())
    }
}

/// Whether `text` is a path. It runs in constant evaluation, so
/// [`path!`](crate::path!) checks literals when compiling.
pub const fn is_path(text: &str) -> bool {
    check(text).is_ok()
}

/// Writes text as a path of any kind: each `/`-separated segment lowercased,
/// with every run of characters other than ASCII letters and digits made one
/// `-`. `Road:Depot/North Gate` becomes `road-depot/north-gate`.
///
/// # Panics
///
/// If a segment has no ASCII letter or digit. Rust code writes keys from its
/// own values, so such a key is a bug in that code.
pub fn slug<K>(text: impl AsRef<str>) -> Path<K> {
    let text = text.as_ref();
    let path = text
        .split('/')
        .map(|segment| {
            slug_segment(segment).unwrap_or_else(|| {
                panic!("segment `{segment}` of `{text}` has no ASCII letter or digit")
            })
        })
        .collect::<Vec<_>>()
        .join("/");
    Path::new(path.into())
}

/// `text` as one path segment, as [`slug`] writes it; none if it has no ASCII
/// letter or digit.
pub fn slug_segment(text: &str) -> Option<String> {
    let segment = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    (!segment.is_empty()).then_some(segment)
}

/// Checks a whole path, segment by segment, in the order its errors are
/// reported. It runs in constant evaluation, so names known when compiling
/// are checked when compiling.
const fn check(path: &str) -> Result<(), PathError> {
    let bytes = path.as_bytes();
    if bytes.is_empty() {
        return Err(PathError::Empty);
    }
    let (mut index, mut segment, mut start) = (0, 0, 0);
    while index <= bytes.len() {
        if index == bytes.len() || bytes[index] == b'/' {
            if index == start {
                return Err(PathError::EmptySegment { segment });
            }
            if bytes[start] == b'-' || bytes[index - 1] == b'-' {
                return Err(PathError::HyphenAtEdge { segment });
            }
            segment += 1;
            start = index + 1;
        } else if !matches!(bytes[index], b'a'..=b'z' | b'0'..=b'9' | b'-') {
            return Err(PathError::InvalidCharacter {
                character: character_at(bytes, index),
            });
        }
        index += 1;
    }
    Ok(())
}

/// The character whose UTF-8 encoding starts at `index`.
const fn character_at(bytes: &[u8], index: usize) -> char {
    let lead = bytes[index] as u32;
    let (length, mut code) = match lead {
        0x00..=0x7f => (1, lead),
        0xf0.. => (4, lead & 0x07),
        0xe0.. => (3, lead & 0x0f),
        _ => (2, lead & 0x1f),
    };
    let mut offset = 1;
    while offset < length {
        code = code << 6 | (bytes[index + offset] as u32 & 0x3f);
        offset += 1;
    }
    match char::from_u32(code) {
        Some(character) => character,
        None => char::REPLACEMENT_CHARACTER,
    }
}

impl<K> FromStr for Path<K> {
    type Err = PathError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        check(path)?;
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

/// Natural order: segment by segment, numbers by value (`job/2` before
/// `job/10`) and before words, and a path before the longer paths it starts.
/// An identifier's `#[derive(Identifier)]` order is this order of its path.
impl<K> Ord for Path<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self.0.split('/');
        let mut right = other.0.split('/');
        loop {
            match (left.next(), right.next()) {
                // Equal values written differently, such as `07` and `7`,
                // still differ as paths.
                (None, None) => return self.0.cmp(&other.0),
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(left), Some(right)) => match natural_segment(left, right) {
                    Ordering::Equal => {}
                    order => return order,
                },
            }
        }
    }
}

fn natural_segment(left: &str, right: &str) -> Ordering {
    let number = |segment: &str| segment.bytes().all(|byte| byte.is_ascii_digit());
    match (number(left), number(right)) {
        (true, true) => {
            let left = left.trim_start_matches('0');
            let right = right.trim_start_matches('0');
            left.len().cmp(&right.len()).then_with(|| left.cmp(right))
        }
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.cmp(right),
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

/// A value written as path segments: what an identifier's path is made of.
/// `#[derive(Identifier)]` writes enums and structs (a variant's kebab-case
/// name, then its fields in order; a struct's fields in order) and orders them
/// as their paths order; unsigned integers, `bool` and `Option` are written
/// here, so their order is their paths' order too.
pub trait Segments: Sized {
    /// Appends this value's segments.
    fn write_segments(&self, segments: &mut Vec<String>);

    /// Reads this value from the segments that follow.
    fn read_segments(segments: &mut SegmentReader<'_>) -> Result<Self, IdentifierError>;
}

/// The segments of a path still to be read.
pub struct SegmentReader<'a>(std::str::Split<'a, char>);

impl<'a> SegmentReader<'a> {
    /// Reads the next segment.
    pub fn segment(&mut self) -> Result<&'a str, IdentifierError> {
        self.0.next().ok_or(IdentifierError::Short)
    }
}

/// A path that does not name a value of the identifier type it was read as.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentifierError {
    #[error("the path ends before the identifier does")]
    Short,
    #[error("`{0}` is not part of this identifier")]
    Unexpected(String),
}

/// An identifier's path: its segments joined with `/`.
pub fn write<K, T: Segments>(value: &T) -> Path<K> {
    let path = written(value);
    debug_assert!(is_path(&path), "`{path}` is not a path");
    Path::new(path.into())
}

/// The identifier a path names; every segment must be read.
pub fn read<K, T: Segments>(path: &Path<K>) -> Result<T, IdentifierError> {
    read_text(path.as_str())
}

fn written<T: Segments>(value: &T) -> String {
    let mut segments = Vec::new();
    value.write_segments(&mut segments);
    segments.join("/")
}

fn read_text<T: Segments>(text: &str) -> Result<T, IdentifierError> {
    let mut segments = SegmentReader(text.split('/'));
    let value = T::read_segments(&mut segments)?;
    match segments.0.next() {
        None => Ok(value),
        Some(rest) => Err(IdentifierError::Unexpected(rest.to_owned())),
    }
}

macro_rules! unsigned_segments {
    ($($integer:ty),*) => {$(
        impl Segments for $integer {
            fn write_segments(&self, segments: &mut Vec<String>) {
                segments.push(self.to_string());
            }

            fn read_segments(segments: &mut SegmentReader<'_>) -> Result<Self, IdentifierError> {
                let segment = segments.segment()?;
                segment
                    .parse()
                    .map_err(|_| IdentifierError::Unexpected(segment.to_owned()))
            }
        }
    )*};
}

unsigned_segments!(u8, u16, u32, u64, usize);

impl Segments for bool {
    fn write_segments(&self, segments: &mut Vec<String>) {
        segments.push(if *self { "true" } else { "false" }.to_owned());
    }

    fn read_segments(segments: &mut SegmentReader<'_>) -> Result<Self, IdentifierError> {
        match segments.segment()? {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(IdentifierError::Unexpected(other.to_owned())),
        }
    }
}

/// `none`, or `some` and the value's own segments, so `None` comes first as
/// it does in `Option`'s order.
impl<T: Segments> Segments for Option<T> {
    fn write_segments(&self, segments: &mut Vec<String>) {
        match self {
            None => segments.push("none".to_owned()),
            Some(value) => {
                segments.push("some".to_owned());
                value.write_segments(segments);
            }
        }
    }

    fn read_segments(segments: &mut SegmentReader<'_>) -> Result<Self, IdentifierError> {
        match segments.segment()? {
            "none" => Ok(None),
            "some" => T::read_segments(segments).map(Some),
            other => Err(IdentifierError::Unexpected(other.to_owned())),
        }
    }
}

/// An identifier written as its path, as `#[derive(Identifier)]` serializes it.
#[doc(hidden)]
pub fn serialize_identifier<T: Segments, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&written(value))
}

/// An identifier read from its path, as `#[derive(Identifier)]` deserializes it.
#[doc(hidden)]
pub fn deserialize_identifier<'de, T: Segments, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    let text = Cow::<str>::deserialize(deserializer)?;
    read_text(&text).map_err(serde::de::Error::custom)
}
