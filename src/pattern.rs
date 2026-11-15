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

use crate::{
    Quantity,
    path::{self, Segments, VariablePath},
};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};
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
        self.canonical().then(|| self.0.parse().ok()).flatten()
    }

    /// The quantity the segment writes, when it writes a whole number
    /// canonically that a quantity holds.
    pub fn quantity(&self) -> Option<Quantity> {
        self.canonical().then(|| self.0.parse().ok()).flatten()
    }

    fn canonical(&self) -> bool {
        self.0.bytes().all(|byte| byte.is_ascii_digit())
            && (&*self.0 == "0" || !self.0.starts_with('0'))
    }
}

impl From<Quantity> for Segment {
    fn from(value: Quantity) -> Self {
        Self(value.to_string().into())
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

/// The values some of a rate's variables may take together, one row each,
/// and the whole numbers each row gives: law structure that never changes,
/// such as which cells of a board adjoin and how far apart they are. A row
/// holds its variables' values, its key, then the numbers it gives, named
/// as `gives` names them; a rate names those as it names derived values.
/// What exchanges can change belongs in the economy's accounts instead.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    variables: Vec<VariablePath>,
    #[serde(default)]
    gives: Vec<VariablePath>,
    rows: Vec<Vec<Segment>>,
}

/// A relation giving nothing is written as it always was.
impl Serialize for Relation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let readable = serializer.is_human_readable();
        let mut state = serializer.serialize_struct("Relation", 3)?;
        state.serialize_field("variables", &self.variables)?;
        if readable && self.gives.is_empty() {
            state.skip_field("gives")?;
        } else {
            state.serialize_field("gives", &self.gives)?;
        }
        state.serialize_field("rows", &self.rows)?;
        state.end()
    }
}

impl Relation {
    /// The rows `variables` may take together.
    pub fn new(
        variables: impl IntoIterator<Item = VariablePath>,
        rows: impl IntoIterator<Item = impl IntoIterator<Item = Segment>>,
    ) -> Self {
        Self::giving(variables, [], rows)
    }

    /// The rows `variables` may take together, each followed by the whole
    /// numbers it gives the names `gives` lists.
    pub fn giving(
        variables: impl IntoIterator<Item = VariablePath>,
        gives: impl IntoIterator<Item = VariablePath>,
        rows: impl IntoIterator<Item = impl IntoIterator<Item = Segment>>,
    ) -> Self {
        Self {
            variables: variables.into_iter().collect(),
            gives: gives.into_iter().collect(),
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().collect())
                .collect(),
        }
    }

    pub fn variables(&self) -> &[VariablePath] {
        &self.variables
    }

    /// The names of the numbers each row gives, in their columns' order.
    pub fn gives(&self) -> &[VariablePath] {
        &self.gives
    }

    pub fn rows(&self) -> &[Vec<Segment>] {
        &self.rows
    }

    /// The row keyed by the values `arguments` give the relation's
    /// variables, if one is.
    pub fn row(&self, arguments: &Arguments) -> Option<&[Segment]> {
        let values: Vec<&Segment> = self
            .variables
            .iter()
            .map(|name| arguments.get(name))
            .collect::<Option<_>>()?;
        self.rows
            .iter()
            .find(|row| {
                row.len() >= values.len() && row[..values.len()].iter().eq(values.iter().copied())
            })
            .map(Vec::as_slice)
    }

    /// The numbers `row` gives, by name.
    pub fn given<'a>(
        &'a self,
        row: &'a [Segment],
    ) -> impl Iterator<Item = (&'a VariablePath, &'a Segment)> {
        self.gives.iter().zip(row.iter().skip(self.variables.len()))
    }
}

/// The value each variable has, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Arguments(Vec<(VariablePath, Segment)>);

impl Arguments {
    /// The values with `name`'s after them.
    pub(crate) fn with(mut self, name: VariablePath, value: Segment) -> Self {
        self.0.push((name, value));
        self
    }

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

impl<T: Segments> Pattern<T> {
    /// The pattern `pieces` write: the identifier itself where none is a
    /// variable.
    pub fn from_pieces(pieces: Vec<Piece>) -> Result<Self, PatternError> {
        if pieces
            .iter()
            .all(|piece| matches!(piece, Piece::Segment(_)))
        {
            let text = pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Segment(segment) => segment.as_str(),
                    Piece::Variable(_) => unreachable!("every piece is a segment"),
                })
                .collect::<Vec<_>>()
                .join("/");
            return path::read_text(&text)
                .map(Self::literal)
                .map_err(|_| PatternError::Unreadable(text));
        }
        Ok(Self {
            form: Form::Written(pieces.into()),
        })
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

/// A type a typed pattern can stand for: what each field of a derived
/// `…Pattern` holds. `#[derive(Identifier)]` and `#[derive(Segments)]`
/// implement it, writing each pattern as its type writes a value.
pub trait Patterned: Segments {
    /// Its typed patterns: the derived `…Pattern` mirror, or
    /// [`Infallible`](std::convert::Infallible) for a type written in one
    /// segment, whose patterns are its values and its variables.
    type Pattern;

    /// Appends the pieces `pattern` writes.
    fn write_pattern(pattern: &Self::Pattern, pieces: &mut Vec<Piece>);
}

/// A type written in exactly one segment, which a variable can therefore
/// stand for: unsigned integers, `bool`, and enums whose variants have no
/// fields.
pub trait OneSegment: Patterned {
    /// The segment `value` is written as.
    fn segment(&self) -> Segment {
        let mut segments = Vec::new();
        self.write_segments(&mut segments);
        debug_assert_eq!(segments.len(), 1, "a one-segment type writes one segment");
        Segment(segments.concat().into())
    }
}

/// A one-segment type with finitely many values, each listed.
pub trait Finite: OneSegment {
    /// Every value, in the order declared.
    fn values() -> Vec<Self>;
}

/// A one-segment type written as a whole number.
pub trait Natural: OneSegment + Copy + Into<u64> {}

/// One field of a typed pattern: a value written out, a variable standing
/// for one segment, or a pattern of the field's own type.
pub enum Slot<T: Patterned> {
    Is(T),
    Var(Param<T>),
    Of(T::Pattern),
}

impl<T: Patterned> Slot<T> {
    /// Appends the pieces it writes.
    pub fn write(&self, pieces: &mut Vec<Piece>) {
        match self {
            Self::Is(value) => {
                let mut segments = Vec::new();
                value.write_segments(&mut segments);
                pieces.extend(
                    segments
                        .into_iter()
                        .map(|segment| Piece::Segment(Segment(segment.into()))),
                );
            }
            Self::Var(param) => pieces.push(Piece::Variable(param.name.clone())),
            Self::Of(pattern) => T::write_pattern(pattern, pieces),
        }
    }
}

impl<T: Patterned + Clone> Clone for Slot<T>
where
    T::Pattern: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Is(value) => Self::Is(value.clone()),
            Self::Var(param) => Self::Var(param.clone()),
            Self::Of(pattern) => Self::Of(pattern.clone()),
        }
    }
}

impl<T: Patterned + fmt::Debug> fmt::Debug for Slot<T>
where
    T::Pattern: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Is(value) => formatter.debug_tuple("Is").field(value).finish(),
            Self::Var(param) => formatter.debug_tuple("Var").field(param).finish(),
            Self::Of(pattern) => formatter.debug_tuple("Of").field(pattern).finish(),
        }
    }
}

impl<T: Patterned + PartialEq> PartialEq for Slot<T>
where
    T::Pattern: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Is(left), Self::Is(right)) => left == right,
            (Self::Var(left), Self::Var(right)) => left == right,
            (Self::Of(left), Self::Of(right)) => left == right,
            _ => false,
        }
    }
}

impl<T: Patterned + Eq> Eq for Slot<T> where T::Pattern: Eq {}

/// A rate's variable whose values are values of `T`, which is written in one
/// segment.
pub struct Param<T> {
    name: VariablePath,
    kind: PhantomData<fn() -> T>,
}

impl<T> Param<T> {
    pub fn name(&self) -> &VariablePath {
        &self.name
    }
}

impl<T: OneSegment> Param<T> {
    /// The variable `name`, taking values of `T`.
    pub fn new(name: VariablePath) -> Self {
        Self {
            name,
            kind: PhantomData,
        }
    }

    /// It as a field of a typed pattern: `{name}`.
    pub fn var(&self) -> Slot<T> {
        Slot::Var(self.clone())
    }

    /// The variable taking the values listed, in their order.
    pub fn values(&self, values: impl IntoIterator<Item = T>) -> Variable {
        Variable::values(
            self.name.clone(),
            values.into_iter().map(|value| value.segment()),
        )
    }
}

impl<T: Finite> Param<T> {
    /// The variable taking every value of `T`.
    pub fn every(&self) -> Variable {
        self.values(T::values())
    }
}

impl<T: Natural> Param<T> {
    /// The variable taking the whole numbers from `first` to `last`.
    pub fn range(&self, first: T, last: T) -> Variable {
        Variable::range(self.name.clone(), first.into(), last.into())
    }

    /// Its value, as a quantity.
    pub fn quantity<O: crate::Ontology>(&self) -> crate::QuantityExpression<O> {
        crate::QuantityExpression::variable(self.name.clone())
    }
}

impl<T> Clone for Param<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            kind: PhantomData,
        }
    }
}

impl<T> fmt::Debug for Param<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{{{}}}", self.name)
    }
}

impl<T> PartialEq for Param<T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<T> Eq for Param<T> {}

impl Relation {
    /// The pairs of values `left` and `right` may take together.
    pub fn pairs<A: OneSegment, B: OneSegment>(
        left: &Param<A>,
        right: &Param<B>,
        rows: impl IntoIterator<Item = (A, B)>,
    ) -> Self {
        Self::new(
            [left.name.clone(), right.name.clone()],
            rows.into_iter()
                .map(|(left, right)| [left.segment(), right.segment()]),
        )
    }

    /// The values `key` may take, each with the whole numbers it gives the
    /// names `gives` lists: each road and how long it is.
    pub fn each_giving<A: OneSegment, V: Into<Quantity>, const N: usize>(
        key: &Param<A>,
        gives: [VariablePath; N],
        rows: impl IntoIterator<Item = (A, [V; N])>,
    ) -> Self {
        Self::giving(
            [key.name.clone()],
            gives,
            rows.into_iter().map(|(key, numbers)| {
                std::iter::once(key.segment())
                    .chain(numbers.map(|number| Segment::from(number.into())))
            }),
        )
    }

    /// The pairs of values `left` and `right` may take together, each with
    /// the whole numbers it gives the names `gives` lists: each pair of
    /// places a road joins and how long it is.
    pub fn pairs_giving<A: OneSegment, B: OneSegment, V: Into<Quantity>, const N: usize>(
        left: &Param<A>,
        right: &Param<B>,
        gives: [VariablePath; N],
        rows: impl IntoIterator<Item = (A, B, [V; N])>,
    ) -> Self {
        Self::giving(
            [left.name.clone(), right.name.clone()],
            gives,
            rows.into_iter().map(|(left, right, numbers)| {
                [left.segment(), right.segment()]
                    .into_iter()
                    .chain(numbers.map(|number| Segment::from(number.into())))
            }),
        )
    }
}

/// A pattern of an `Option`: `none`, or `some` and a pattern of the value.
pub enum OptionPattern<T: Patterned> {
    None,
    Some(Slot<T>),
}

impl<T: Patterned + Clone> Clone for OptionPattern<T>
where
    T::Pattern: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Some(slot) => Self::Some(slot.clone()),
        }
    }
}

impl<T: Patterned + fmt::Debug> fmt::Debug for OptionPattern<T>
where
    T::Pattern: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Some(slot) => formatter.debug_tuple("Some").field(slot).finish(),
        }
    }
}

impl<T: Patterned + PartialEq> PartialEq for OptionPattern<T>
where
    T::Pattern: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Some(left), Self::Some(right)) => left == right,
            _ => false,
        }
    }
}

impl<T: Patterned + Eq> Eq for OptionPattern<T> where T::Pattern: Eq {}

impl<T: Patterned> Patterned for Option<T> {
    type Pattern = OptionPattern<T>;

    fn write_pattern(pattern: &Self::Pattern, pieces: &mut Vec<Piece>) {
        match pattern {
            OptionPattern::None => pieces.push(Piece::Segment(Segment("none".into()))),
            OptionPattern::Some(slot) => {
                pieces.push(Piece::Segment(Segment("some".into())));
                slot.write(pieces);
            }
        }
    }
}

impl<K> Patterned for path::Path<K> {
    type Pattern = Pattern<path::Path<K>>;

    fn write_pattern(pattern: &Self::Pattern, pieces: &mut Vec<Piece>) {
        pieces.extend(pattern.pieces().iter().cloned());
    }
}

macro_rules! one_segment {
    ($($kind:ty),*) => {$(
        impl Patterned for $kind {
            type Pattern = std::convert::Infallible;

            fn write_pattern(pattern: &Self::Pattern, _: &mut Vec<Piece>) {
                match *pattern {}
            }
        }

        impl OneSegment for $kind {}
    )*};
}

one_segment!(u8, u16, u32, u64, bool);

impl Natural for u8 {}
impl Natural for u16 {}
impl Natural for u32 {}
impl Natural for u64 {}

impl Finite for bool {
    fn values() -> Vec<Self> {
        vec![false, true]
    }
}

impl<T: Patterned> From<OptionPattern<T>> for Pattern<Option<T>> {
    fn from(pattern: OptionPattern<T>) -> Self {
        let mut pieces = Vec::new();
        Option::<T>::write_pattern(&pattern, &mut pieces);
        Self::from_pieces(pieces).expect("a typed pattern writes a value of its type")
    }
}
