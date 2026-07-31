//! Runtime-neutral control and progress contracts for bounded computation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;

/// A deterministic amount of algorithm work requested from a resumable session.
///
/// Work units are algorithm-specific: graph searches count expanded states,
/// Monte Carlo counts samples, and tree searches count iterations. They are
/// deliberately unrelated to wall-clock time or economic quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkBudget {
    units: usize,
}

impl WorkBudget {
    pub const fn new(units: usize) -> Self {
        Self { units }
    }

    pub const fn units(self) -> usize {
        self.units
    }
}

/// Lifecycle state reported by one bounded call to a search session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchStatus {
    Running,
    Solved,
    Exhausted,
    Interrupted,
}

impl SearchStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Solved | Self::Exhausted)
    }
}

/// Progress and lifecycle information returned after a bounded advance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdvanceReport<Progress> {
    status: SearchStatus,
    work_completed: usize,
    progress: Progress,
}

impl<Progress> AdvanceReport<Progress> {
    pub const fn new(status: SearchStatus, work_completed: usize, progress: Progress) -> Self {
        Self {
            status,
            work_completed,
            progress,
        }
    }

    pub const fn status(&self) -> SearchStatus {
        self.status
    }

    pub const fn work_completed(&self) -> usize {
        self.work_completed
    }

    pub const fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn into_progress(self) -> Progress {
        self.progress
    }
}

/// Caller-owned hook for progress reporting and cooperative interruption.
///
/// Returning `ControlFlow::Break(())` ends the current `advance` call at a safe
/// point. It does not make the session terminal; a caller may advance it again.
pub trait SearchObserver<Progress> {
    fn observe(&mut self, progress: &Progress) -> ControlFlow<()>;
}

impl<Progress, F> SearchObserver<Progress> for F
where
    F: FnMut(&Progress) -> ControlFlow<()>,
{
    fn observe(&mut self, progress: &Progress) -> ControlFlow<()> {
        self(progress)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Continue;

impl<Progress> SearchObserver<Progress> for Continue {
    fn observe(&mut self, _progress: &Progress) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_budgets_are_plain_algorithm_units() {
        let budget = WorkBudget::new(42);
        assert_eq!(budget.units(), 42);
        assert_eq!(
            serde_json::to_value(budget).unwrap(),
            serde_json::json!({ "units": 42 })
        );
    }
}
