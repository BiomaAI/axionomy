//! Identifiers written with variables.
//!
//! A rate may have variables: a pattern is a path in which some segments are
//! variables, `{name}`, each standing for exactly one segment, as in RFC 6570
//! templates, OpenAPI paths and axum routes. A rate's identifier is a
//! pattern, and so is each asset and account it names: the rate keyed
//! `stabilize/{patient}/{location}` answers to `stabilize/1/east`, and there
//! it consumes `at/east` where it names `at/{location}`. A pattern with no
//! variable is the identifier it writes; it keeps the typed value, so a rate
//! without variables is exactly the rate it always was.

use crate::path::{self, Segments, VariablePath};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
    sync::Arc,
};
use thiserror::Error;

/// One segment of a path, written out: a variable's value, such as `east`
/// or `7`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Segment(Arc<str>);

impl Segment {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The whole number the segment writes, when it writes one canonically.
    pub fn number(&self) -> Option<u64> {
        let canonical = self.0.bytes().all(|byte| byte.is_ascii_digit())
            && (&*self.0 == "0" || !self.0.starts_with('0'));
        canonical.then(|| self.0.parse().ok()).flatten()
    }
}

impl From<u64> for Segment {
    fn from(value: u64) -> Self {
        Self(value.to_string().into())
    }
}

impl FromStr for Segment {
    type Err = PatternError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.contains('/') || !path::is_path(text) {
            return Err(PatternError::Segment(text.to_owned()));
        }
        Ok(Self(text.into()))
    }
}

impl fmt::Debug for Segment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &*self.0)
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Ord for Segment {
    fn cmp(&self, other: &Self) -> Ordering {
        path::natural_order(&self.0, &other.0)
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for Segment {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Segment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

impl JsonSchema for Segment {
    fn schema_name() -> Cow<'static, str> {
        "Segment".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": "^[a-z0-9]([a-z0-9-]*[a-z0-9])?$", "examples": ["east"] })
    }
}

/// The values a variable may take: whole numbers from `first` to `last`, or
/// the segments listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Domain {
    Range { first: u64, last: u64 },
    Values(Vec<Segment>),
}

impl Domain {
    pub fn contains(&self, value: &Segment) -> bool {
        match self {
            Self::Range { first, last } => value
                .number()
                .is_some_and(|number| (*first..=*last).contains(&number)),
            Self::Values(values) => values.contains(value),
        }
    }

    /// Every value, in order: a range's numbers ascending, listed values as
    /// listed.
    pub fn values(&self) -> Vec<Segment> {
        match self {
            Self::Range { first, last } => (*first..=*last).map(Segment::from).collect(),
            Self::Values(values) => values.clone(),
        }
    }
}

/// A rate's variable and its values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    name: VariablePath,
    domain: Domain,
}

impl Variable {
    /// A variable taking whole numbers from `first` to `last`.
    pub fn range(name: VariablePath, first: u64, last: u64) -> Self {
        Self {
            name,
            domain: Domain::Range { first, last },
        }
    }

    /// A variable taking the segments listed, in their order.
    pub fn values(name: VariablePath, values: impl IntoIterator<Item = Segment>) -> Self {
        Self {
            name,
            domain: Domain::Values(values.into_iter().collect()),
        }
    }

    pub fn name(&self) -> &VariablePath {
        &self.name
    }

    pub fn domain(&self) -> &Domain {
        &self.domain
    }
}

/// How a variable is written where people read it: `{"name": "patient",
/// "range": ["1", "3"]}` or `{"name": "location", "values": ["east"]}`.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VariableText {
    name: VariablePath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    range: Option<[String; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    values: Option<Vec<Segment>>,
}

#[derive(Serialize, Deserialize)]
enum DomainCompact {
    Range(u64, u64),
    Values(Vec<Segment>),
}

impl Serialize for Variable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let (range, values) = match &self.domain {
                Domain::Range { first, last } => {
                    (Some([first.to_string(), last.to_string()]), None)
                }
                Domain::Values(values) => (None, Some(values.clone())),
            };
            VariableText {
                name: self.name.clone(),
                range,
                values,
            }
            .serialize(serializer)
        } else {
            let domain = match &self.domain {
                Domain::Range { first, last } => DomainCompact::Range(*first, *last),
                Domain::Values(values) => DomainCompact::Values(values.clone()),
            };
            (&self.name, domain).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Variable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let variable = if deserializer.is_human_readable() {
            let text = VariableText::deserialize(deserializer)?;
            let domain = match (text.range, text.values) {
                (Some([first, last]), None) => Domain::Range {
                    first: first.parse().map_err(de::Error::custom)?,
                    last: last.parse().map_err(de::Error::custom)?,
                },
                (None, Some(values)) => Domain::Values(values),
                _ => {
                    return Err(de::Error::custom(
                        "a variable has a range or values, not both",
                    ));
                }
            };
            Self {
                name: text.name,
                domain,
            }
        } else {
            let (name, domain) = <(VariablePath, DomainCompact)>::deserialize(deserializer)?;
            Self {
                name,
                domain: match domain {
                    DomainCompact::Range(first, last) => Domain::Range { first, last },
                    DomainCompact::Values(values) => Domain::Values(values),
                },
            }
        };
        if variable.name.as_str().contains('/') {
            return Err(de::Error::custom("a variable's name is one segment"));
        }
        Ok(variable)
    }
}

impl JsonSchema for Variable {
    fn schema_name() -> Cow<'static, str> {
        "Variable".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        VariableText::json_schema(generator)
    }
}

/// The value each variable has, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Arguments(Vec<(VariablePath, Segment)>);

impl Arguments {
    pub fn get(&self, name: &VariablePath) -> Option<&Segment> {
        self.0
            .iter()
            .find(|(variable, _)| variable == name)
            .map(|(_, value)| value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&VariablePath, &Segment)> {
        self.0.iter().map(|(name, value)| (name, value))
    }
}

impl FromIterator<(VariablePath, Segment)> for Arguments {
    fn from_iter<I: IntoIterator<Item = (VariablePath, Segment)>>(values: I) -> Self {
        Self(values.into_iter().collect())
    }
}

/// One segment of a pattern: written out, or a variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Piece {
    Segment(Segment),
    Variable(VariablePath),
}

/// A path whose segments may be variables, standing for the identifier of
/// type `T` it writes once each variable has a value.
#[derive(Clone)]
pub struct Pattern<T> {
    form: Form<T>,
}

#[derive(Clone)]
enum Form<T> {
    /// No variable: the identifier itself.
    Literal(T),
    /// At least one variable.
    Written(Arc<[Piece]>),
}

impl<T> Pattern<T> {
    /// The pattern that is `value` itself.
    pub const fn literal(value: T) -> Self {
        Self {
            form: Form::Literal(value),
        }
    }

    /// The identifier it is, where it has no variable.
    pub fn as_literal(&self) -> Option<&T> {
        match &self.form {
            Form::Literal(value) => Some(value),
            Form::Written(_) => None,
        }
    }

    /// Its variables, in order; none for an identifier written out.
    pub fn variables(&self) -> impl Iterator<Item = &VariablePath> {
        let pieces: &[Piece] = match &self.form {
            Form::Literal(_) => &[],
            Form::Written(pieces) => pieces,
        };
        pieces.iter().filter_map(|piece| match piece {
            Piece::Variable(name) => Some(name),
            Piece::Segment(_) => None,
        })
    }
}

impl<T: Segments> Pattern<T> {
    /// Its pieces: each segment of an identifier written out, or the
    /// pattern's own.
    pub fn pieces(&self) -> Cow<'_, [Piece]> {
        match &self.form {
            Form::Literal(value) => {
                let mut segments = Vec::new();
                value.write_segments(&mut segments);
                Cow::Owned(
                    segments
                        .into_iter()
                        .map(|segment| Piece::Segment(Segment(segment.into())))
                        .collect(),
                )
            }
            Form::Written(pieces) => Cow::Borrowed(pieces),
        }
    }

    /// The identifier it writes with `arguments`' values.
    pub fn instantiate(&self, arguments: &Arguments) -> Result<T, InstanceError>
    where
        T: Clone,
    {
        let pieces = match &self.form {
            Form::Literal(value) => return Ok(value.clone()),
            Form::Written(pieces) => pieces,
        };
        let mut text = String::new();
        for (index, piece) in pieces.iter().enumerate() {
            if index > 0 {
                text.push('/');
            }
            match piece {
                Piece::Segment(segment) => text.push_str(segment.as_str()),
                Piece::Variable(name) => {
                    let value = arguments
                        .get(name)
                        .ok_or_else(|| InstanceError::Unwritten(name.clone()))?;
                    text.push_str(value.as_str());
                }
            }
        }
        path::read_text(&text).map_err(|_| InstanceError::Unreadable(text))
    }

    /// The values `segments` give its variables, if they match it: as many
    /// segments, each written one repeated.
    pub fn read(&self, segments: &[&str]) -> Option<Arguments> {
        let pieces = self.pieces();
        if pieces.len() != segments.len() {
            return None;
        }
        let mut arguments = Vec::new();
        for (piece, segment) in pieces.iter().zip(segments) {
            match piece {
                Piece::Segment(written) if written.as_str() == *segment => {}
                Piece::Segment(_) => return None,
                Piece::Variable(name) => arguments.push((name.clone(), Segment((*segment).into()))),
            }
        }
        Some(Arguments(arguments))
    }

    /// Whether no identifier matches both: their lengths differ, or they
    /// write different segments where both write one.
    pub fn disjoint(&self, other: &Self) -> bool {
        let (pieces, others) = (self.pieces(), other.pieces());
        pieces.len() != others.len()
            || pieces.iter().zip(others.iter()).any(|pair| match pair {
                (Piece::Segment(left), Piece::Segment(right)) => left != right,
                _ => false,
            })
    }

    /// How it is written: segments joined with `/`, a variable as `{name}`.
    pub fn text(&self) -> String {
        match &self.form {
            Form::Literal(value) => path::written(value),
            Form::Written(pieces) => pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Segment(segment) => segment.as_str().to_owned(),
                    Piece::Variable(name) => format!("{{{name}}}"),
                })
                .collect::<Vec<_>>()
                .join("/"),
        }
    }
}

impl<T> From<T> for Pattern<T> {
    fn from(value: T) -> Self {
        Self::literal(value)
    }
}

impl<T: Segments> FromStr for Pattern<T> {
    type Err = PatternError;

    /// `at/{location}`: a variable in braces, every other segment a path's.
    /// Text with no variable reads as the identifier it writes.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !text.contains('{') {
            return path::read_text(text)
                .map(Self::literal)
                .map_err(|_| PatternError::Unreadable(text.to_owned()));
        }
        let pieces = text
            .split('/')
            .map(|segment| {
                match segment
                    .strip_prefix('{')
                    .and_then(|rest| rest.strip_suffix('}'))
                {
                    Some(name) if !name.contains('/') && path::is_path(name) => {
                        Ok(Piece::Variable(name.parse().expect("checked as a path")))
                    }
                    Some(_) => Err(PatternError::Variable(segment.to_owned())),
                    None => segment.parse().map(Piece::Segment),
                }
            })
            .collect::<Result<Arc<[Piece]>, _>>()?;
        Ok(Self {
            form: Form::Written(pieces),
        })
    }
}

impl<T: PartialEq> PartialEq for Pattern<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.form, &other.form) {
            (Form::Literal(left), Form::Literal(right)) => left == right,
            (Form::Written(left), Form::Written(right)) => left == right,
            _ => false,
        }
    }
}

impl<T: Eq> Eq for Pattern<T> {}

impl<T: Hash> Hash for Pattern<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.form {
            Form::Literal(value) => {
                state.write_u8(0);
                value.hash(state);
            }
            Form::Written(pieces) => {
                state.write_u8(1);
                pieces.hash(state);
            }
        }
    }
}

/// Patterns in the natural order of how they are written.
impl<T: Segments + Eq> Ord for Pattern<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.form, &other.form) {
            (Form::Literal(left), Form::Literal(right)) if left == right => Ordering::Equal,
            _ => path::natural_order(&self.text(), &other.text()),
        }
    }
}

impl<T: Segments + Eq> PartialOrd for Pattern<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: fmt::Debug> fmt::Debug for Pattern<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.form {
            Form::Literal(value) => value.fmt(formatter),
            Form::Written(pieces) => formatter.debug_tuple("Pattern").field(pieces).finish(),
        }
    }
}

impl<T: Segments> fmt::Display for Pattern<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text())
    }
}

#[derive(Serialize, Deserialize)]
enum Compact<T> {
    Literal(T),
    Written(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Readable<T> {
    Literal(T),
    Written(String),
}

/// Where people read it, an identifier written out is written as its type
/// writes it, and a pattern with variables as its text; elsewhere each is
/// tagged.
impl<T: Serialize + Segments> Serialize for Pattern<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match (&self.form, serializer.is_human_readable()) {
            (Form::Literal(value), true) => value.serialize(serializer),
            (Form::Written(_), true) => serializer.serialize_str(&self.text()),
            (Form::Literal(value), false) => Compact::Literal(value).serialize(serializer),
            (Form::Written(_), false) => Compact::<&T>::Written(self.text()).serialize(serializer),
        }
    }
}

impl<'de, T: Deserialize<'de> + Segments> Deserialize<'de> for Pattern<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (literal, written) = if deserializer.is_human_readable() {
            match Readable::<T>::deserialize(deserializer)? {
                Readable::Literal(value) => (Some(value), None),
                Readable::Written(text) => (None, Some(text)),
            }
        } else {
            match Compact::<T>::deserialize(deserializer)? {
                Compact::Literal(value) => (Some(value), None),
                Compact::Written(text) => (None, Some(text)),
            }
        };
        match (literal, written) {
            (Some(value), _) => Ok(Self::literal(value)),
            (None, Some(text)) => text.parse().map_err(de::Error::custom),
            (None, None) => unreachable!("a pattern is read as one or the other"),
        }
    }
}

impl<T: JsonSchema> JsonSchema for Pattern<T> {
    fn schema_name() -> Cow<'static, str> {
        format!("Pattern_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let literal = generator.subschema_for::<T>();
        json_schema!({
            "anyOf": [literal, { "type": "string", "description": "A path whose segments may be variables, `{name}`." }]
        })
    }
}

/// Text that is not a pattern.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PatternError {
    #[error("`{0}` is not a path segment")]
    Segment(String),
    #[error("`{0}` is not a variable: a variable is one segment in braces, such as `{{location}}`")]
    Variable(String),
    #[error("`{0}` does not name an identifier of this kind")]
    Unreadable(String),
}

/// Why a pattern writes no identifier for some values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InstanceError {
    #[error("the variable `{0}` has no value")]
    Unwritten(VariablePath),
    #[error("`{0}` names no identifier of this kind")]
    Unreadable(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::AssetPath;

    #[test]
    fn a_pattern_reads_an_identifier_and_writes_it_back() {
        let pattern: Pattern<AssetPath> = "stabilize/{patient}/east/{restore}".parse().unwrap();
        let arguments = pattern.read(&["stabilize", "1", "east", "true"]).unwrap();
        assert_eq!(
            arguments.get(&"patient".parse().unwrap()).unwrap().as_str(),
            "1"
        );
        assert_eq!(
            pattern.instantiate(&arguments).unwrap().as_str(),
            "stabilize/1/east/true"
        );
        assert!(pattern.read(&["stabilize", "1", "west", "true"]).is_none());
        assert!(pattern.read(&["stabilize", "1", "east"]).is_none());
        assert_eq!(pattern.text(), "stabilize/{patient}/east/{restore}");
    }

    #[test]
    fn text_without_a_variable_is_the_identifier() {
        let pattern: Pattern<AssetPath> = "at/east".parse().unwrap();
        assert_eq!(pattern.as_literal().unwrap().as_str(), "at/east");
        assert!("at/{East}".parse::<Pattern<AssetPath>>().is_err());
        assert!("at/{a/b}".parse::<Pattern<AssetPath>>().is_err());
        assert!("At/east".parse::<Pattern<AssetPath>>().is_err());
    }

    #[test]
    fn patterns_apart_match_no_identifier_in_common() {
        let pattern = |text: &str| text.parse::<Pattern<AssetPath>>().unwrap();
        assert!(!pattern("advance/{to}").disjoint(&pattern("advance/final")));
        assert!(pattern("move/{to}/fast").disjoint(&pattern("move/{to}/slow")));
        assert!(pattern("move/{to}").disjoint(&pattern("move/{to}/{how}")));
    }

    #[test]
    fn domains_hold_canonical_values() {
        let row = Domain::Range { first: 0, last: 5 };
        assert!(row.contains(&"5".parse().unwrap()));
        assert!(!row.contains(&"6".parse().unwrap()));
        assert!(!row.contains(&"05".parse().unwrap()));
        assert!(!row.contains(&"east".parse().unwrap()));
    }
}
