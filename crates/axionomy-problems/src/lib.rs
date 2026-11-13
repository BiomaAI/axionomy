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

#[cfg(feature = "packages")]
mod package;
mod support;

use axionomy::{ApplyError, Ontology, ReplayError};
use axionomy_search::{sampling::SamplingError, tree::TreeSearchError};
use axionomy_study::{DuplicateProblem, Registry};

/// What writes a sample's package into an engine's store, everything named
/// under the sample's name.
#[cfg(feature = "packages")]
pub type WritePackage = fn(&axionomy_engine::Engine) -> Result<(), axionomy_engine::EngineError>;

/// The samples written as packages, by name, in catalog order.
#[cfg(feature = "packages")]
pub fn packages() -> Vec<(axionomy_engine::PackagePath, WritePackage)> {
    let packages: [(&str, WritePackage); 9] = [
        ("maze", maze::package::write),
        ("sokoban", sokoban::package::write),
        ("exact-cover", exact_cover::package::write),
        ("workshop", workshop::package::write),
        ("bridge", bridge::package::write),
        ("rescue", rescue::package::write),
        ("scheduling", scheduling::package::write),
        ("amm", amm::package::write),
        ("lending", lending::package::write),
    ];
    packages
        .into_iter()
        .map(|(name, write)| (name.parse().expect("a sample's name is a path"), write))
        .collect()
}

/// Every sample problem, in catalog order: the registry Axionomy's
/// front ends study by default.
pub fn registry() -> Registry {
    samples().expect("the sample problems have distinct keys")
}

fn samples() -> Result<Registry, DuplicateProblem> {
    let mut registry = Registry::new();
    registry
        .register(maze::Maze::default())?
        .register(sokoban::Sokoban::default())?
        .register(exact_cover::ExactCover::default())?
        .register(workshop::Workshop::default())?
        .register(scheduling::Scheduling::default())?
        .register(rescue::Rescue::default())?
        .register(bridge::Bridge::default())?
        .register(amm::Amm::default())?
        .register(lending::Lending::default())?
        .register(marketplace::Marketplace::default())?
        .register(logistics::Logistics::default())?
        .register(connect_four::ConnectFour::default())?
        .register(work_league::WorkLeague::default())?
        .register(mission::Mission::default())?
        .register(perishables::Perishables::default())?;
    Ok(registry)
}

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
