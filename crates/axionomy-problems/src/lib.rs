#![doc = include_str!("../README.md")]

pub mod amm;
pub mod bridge;
pub mod connect_four;
pub mod exact_cover;
pub mod lending;
pub mod logistics;
pub mod marketplace;
pub mod maze;
pub mod mission;
pub mod perishables;
pub mod rescue;
pub mod scheduling;
pub mod sokoban;
pub mod work_league;
pub mod workshop;

use axionomy::{ApplyError, Ontology, ReplayError};
use axionomy_search::{sampling::SamplingError, tree::TreeSearchError};

/// Why a planned run (Nature sampled, decisions planned, every exchange
/// applied) could not be played to its end.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PlannedRunError<O: Ontology> {
    #[error("Nature could not be sampled: {0}")]
    Sampling(#[from] SamplingError),
    #[error("planning failed: {0:?}")]
    Planning(TreeSearchError<O>),
    #[error("an exchange was rejected: {0}")]
    Rejected(#[from] ApplyError<O>),
    #[error("the run does not replay: {0}")]
    Replay(#[from] ReplayError<O>),
    #[error("the run's replay reached a different state")]
    Diverged,
}

impl<O: Ontology> From<TreeSearchError<O>> for PlannedRunError<O> {
    fn from(error: TreeSearchError<O>) -> Self {
        Self::Planning(error)
    }
}
