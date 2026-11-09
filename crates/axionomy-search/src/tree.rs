//! Pieces shared by Monte Carlo tree search and its information-set variant.

use crate::{mcts::MctsConfig, sampling::SamplingError};
use axionomy::ApplyError;
use axionomy::Ontology;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Lifecycle of an incremental tree search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TreeStatus {
    Running,
    Completed,
    Interrupted,
}

/// Why a tree search could not produce a decision.
#[derive(Debug, Clone)]
pub enum TreeSearchError<O: Ontology> {
    NoPlayers,
    ZeroIterations,
    ZeroDepth,
    InvalidExploration,
    TerminalRoot,
    /// Monte Carlo tree search: the root is a chance node.
    ChanceRoot,
    NoRootActions,
    NoExploredAction,
    /// Information-set search: no sampled world matched the root information.
    NoDeterminization {
        iteration: usize,
    },
    /// Information-set search: a sampled world disagreed with the root information.
    InconsistentDeterminization,
    InvalidActor {
        actor: usize,
        players: usize,
    },
    InvalidValueDimensions {
        expected: usize,
        actual: usize,
    },
    NonFiniteValue {
        player: usize,
    },
    Sampling(SamplingError),
    Rejected(ApplyError<O>),
    SelectedActionRejected(ApplyError<O>),
}

pub(crate) fn validate_config<O: Ontology>(
    config: MctsConfig,
    players: usize,
) -> Result<(), TreeSearchError<O>> {
    if players == 0 {
        return Err(TreeSearchError::NoPlayers);
    }
    if config.iterations() == 0 {
        return Err(TreeSearchError::ZeroIterations);
    }
    if config.max_depth() == 0 {
        return Err(TreeSearchError::ZeroDepth);
    }
    if !config.exploration().is_finite() || config.exploration() < 0.0 {
        return Err(TreeSearchError::InvalidExploration);
    }
    Ok(())
}

pub(crate) fn validate_actor<O: Ontology>(
    actor: usize,
    players: usize,
) -> Result<(), TreeSearchError<O>> {
    if actor >= players {
        Err(TreeSearchError::InvalidActor { actor, players })
    } else {
        Ok(())
    }
}

/// Accepts one finite value per player.
pub(crate) fn validate_values<O: Ontology>(
    values: Vec<f64>,
    players: usize,
) -> Result<Vec<f64>, TreeSearchError<O>> {
    if values.len() != players {
        return Err(TreeSearchError::InvalidValueDimensions {
            expected: players,
            actual: values.len(),
        });
    }
    if let Some(player) = values.iter().position(|value| !value.is_finite()) {
        return Err(TreeSearchError::NonFiniteValue { player });
    }
    Ok(values)
}

/// Upper confidence bound for trees: the average value plus an exploration
/// bonus that shrinks as the edge is visited. Unvisited edges come first.
pub(crate) fn uct_score(
    value_total: f64,
    visits: u64,
    parent_visits: f64,
    exploration: f64,
) -> f64 {
    if visits == 0 {
        return f64::INFINITY;
    }
    let visits = visits as f64;
    value_total / visits + exploration * (parent_visits.ln() / visits).sqrt()
}

/// Average value per player, zero before the first visit.
pub(crate) fn mean_values(value_totals: &[f64], visits: u64) -> Vec<f64> {
    if visits == 0 {
        return vec![0.0; value_totals.len()];
    }
    value_totals
        .iter()
        .map(|total| total / visits as f64)
        .collect()
}
