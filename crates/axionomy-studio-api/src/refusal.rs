//! Why an operation is refused, the same whichever engine refuses it: the
//! Studio server answers with its status and body, and the browser Worker
//! posts both.

use crate::PointId;
use axionomy_engine::{EngineError, PackageError};
use axionomy_store::RefPath;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A refused request, in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorResponse {
    pub error: String,
}

/// A move refused because the ref was not where the mover expected it, or a
/// ref made under a name a ref already has: where the ref is, to look again
/// from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefMoved {
    pub error: String,
    pub name: RefPath,
    pub current: PointId,
}

/// Why an operation was refused, as an HTTP status says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// What the request names is not in the store: 404.
    NotFound(String),
    /// The request cannot be served as it is asked: 400.
    BadRequest(String),
    /// A ref was not where a move expected it: 409, with where it is.
    Moved(RefMoved),
    /// The engine failed: 500.
    Failed(String),
}

/// A refusal as it crosses a boundary that is not HTTP: the status HTTP
/// would answer with, and the body.
#[derive(Debug, Clone, Serialize)]
pub struct Refused {
    pub status: u16,
    pub body: RefusalBody,
}

/// What a refusal says: where a ref is, or why in words.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum RefusalBody {
    Moved(RefMoved),
    Error(ErrorResponse),
}

impl Refusal {
    pub fn status(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::BadRequest(_) => 400,
            Self::Moved(_) => 409,
            Self::Failed(_) => 500,
        }
    }

    pub fn body(self) -> RefusalBody {
        match self {
            Self::Moved(moved) => RefusalBody::Moved(moved),
            Self::NotFound(error) | Self::BadRequest(error) | Self::Failed(error) => {
                RefusalBody::Error(ErrorResponse { error })
            }
        }
    }

    pub fn refused(self) -> Refused {
        Refused {
            status: self.status(),
            body: self.body(),
        }
    }

    /// The refusal in words.
    pub fn message(&self) -> &str {
        match self {
            Self::Moved(moved) => &moved.error,
            Self::NotFound(error) | Self::BadRequest(error) | Self::Failed(error) => error,
        }
    }
}

impl From<EngineError> for Refusal {
    fn from(error: EngineError) -> Self {
        match error {
            EngineError::Moved {
                name,
                current: Some(current),
            } => Self::Moved(RefMoved {
                error: format!(
                    "ref `{name}` is not where the move expected it: another move came first, and it is at `{}`; look again from there",
                    PointId::from(current)
                ),
                name,
                current: current.into(),
            }),
            // A ref that names nothing is not a ref of this store.
            EngineError::Moved {
                name,
                current: None,
            } => Self::NotFound(format!("no ref `{name}` names a point")),
            EngineError::UnknownRef(_)
            | EngineError::UnknownPoint(_)
            | EngineError::Undefined(_)
            | EngineError::UnknownScript(_) => Self::NotFound(error.to_string()),
            EngineError::Backward { .. } => Self::BadRequest(format!(
                "{error}: a ref moves only forward, to a point whose history passes through where it is"
            )),
            EngineError::NotOnHistory { .. } => Self::BadRequest(format!(
                "{error}: ask for a point the history passes through"
            )),
            EngineError::Refused { .. } => {
                Self::BadRequest(format!("{error}; nothing was applied, so assess it first"))
            }
            EngineError::NeedsHistory(_) | EngineError::Interrupted => {
                Self::BadRequest(error.to_string())
            }
            // A file that is no package, or one this engine cannot load.
            EngineError::Package(
                PackageError::NoManifest
                | PackageError::HostApi { .. }
                | PackageError::Format { .. }
                | PackageError::Archive(_),
            ) => Self::BadRequest(error.to_string()),
            error => Self::Failed(error.to_string()),
        }
    }
}
