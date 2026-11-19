//! The portable source tree. Its keys are relative paths with a fixed
//! extension and a different identifier type for each kind of document.
use super::*;
use serde::{Deserializer, Serializer};
use std::{fmt, str::FromStr};

pub enum Test {}
impl PathKind for Test {
    const SCHEMA: &'static str = "PackageTestPath";
}
pub type TestPath = Path<Test>;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum SourceFile {
    Package,
    Definitions,
    Ontology(OntologyPath),
    Laws(LawSetPath),
    Instance(RefPath),
    Script(ScriptPath),
    Test(TestPath),
}
impl fmt::Display for SourceFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package => f.write_str("package.json"),
            Self::Definitions => f.write_str("definitions.json"),
            Self::Ontology(p) => write!(f, "ontologies/{p}.json"),
            Self::Laws(p) => write!(f, "laws/{p}.json"),
            Self::Instance(p) => write!(f, "instances/{p}.json"),
            Self::Script(p) => write!(f, "scripts/{p}.rhai"),
            Self::Test(p) => write!(f, "tests/{p}.json"),
        }
    }
}
impl FromStr for SourceFile {
    type Err = Diagnostic;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let parsed = match text {
            "package.json" => return Ok(Self::Package),
            "definitions.json" => return Ok(Self::Definitions),
            _ => match text.split_once('/') {
                Some(("scripts", p)) => p
                    .strip_suffix(".rhai")
                    .and_then(|p| p.parse().ok())
                    .map(Self::Script),
                Some((kind, p)) => p.strip_suffix(".json").and_then(|p| match kind {
                    "ontologies" => p.parse().ok().map(Self::Ontology),
                    "laws" => p.parse().ok().map(Self::Laws),
                    "instances" => p.parse().ok().map(Self::Instance),
                    "tests" => p.parse().ok().map(Self::Test),
                    _ => None,
                }),
                None => None,
            },
        };
        parsed.ok_or_else(|| Diagnostic::new(text, "unknown package source file or invalid path"))
    }
}
impl Serialize for SourceFile {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for SourceFile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        std::borrow::Cow::<str>::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// A JSON object of relative file names and their UTF-8 contents. All
/// transports pass this same tree to the compiler.
pub type SourceFiles = BTreeMap<SourceFile, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{file} {field} {reason}")]
pub struct Diagnostic {
    /// Source location, including rejected file names.
    pub file: std::path::PathBuf,
    /// JSON field path reported by the parser, when available.
    pub field: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub reason: String,
}
impl Diagnostic {
    pub(super) fn new(file: impl ToString, reason: impl ToString) -> Self {
        Self {
            file: file.to_string().into(),
            field: String::new(),
            line: None,
            column: None,
            reason: reason.to_string(),
        }
    }
}
pub(super) fn parse<T: serde::de::DeserializeOwned>(
    file: &SourceFile,
    text: &str,
) -> Result<T, Diagnostic> {
    let mut reader = serde_json::Deserializer::from_str(text);
    let value = serde_path_to_error::deserialize(&mut reader).map_err(|error| {
        let mut diagnostic = Diagnostic::new(file, error.inner());
        diagnostic.field = error.path().to_string();
        diagnostic.line = Some(error.inner().line());
        diagnostic.column = Some(error.inner().column());
        diagnostic
    })?;
    reader.end().map_err(|error| Diagnostic::new(file, error))?;
    Ok(value)
}

impl schemars::JsonSchema for SourceFile {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PackageSourceFile".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string","description":"package.json, definitions.json, ontologies/<path>.json, laws/<path>.json, instances/<ref>.json, tests/<path>.json or scripts/<path>.rhai"})
    }
}

/// Original, editable source documents carried by a compiled package.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceTree {
    pub files: SourceFiles,
}
