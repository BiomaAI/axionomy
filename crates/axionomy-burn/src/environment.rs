//! A task's episode behind Burn's [`burn::rl::Environment`].

use crate::dqn::{Chosen, Observed};
use axionomy_rl::{Episode, Observation, Task};
use burn::rl::{StepResult, ToAction, ToObservation};
use burn::tensor::Device;
use std::sync::Arc;

/// What a learner sees at an episode's point, as Burn's runner keeps it.
#[derive(Debug, Clone)]
pub struct State(pub Observation);

impl ToObservation<Observed> for State {
    fn to_observation(&self, device: &Device) -> Observed {
        Observed::new(&self.0, device)
    }
}

/// The task's exchange a learner chose, by its place in
/// [`Task::actions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action(pub usize);

impl From<Chosen> for Action {
    fn from(chosen: Chosen) -> Self {
        Self(chosen.first())
    }
}

impl ToAction<Chosen> for Action {
    fn to_action(&self, device: &Device) -> Chosen {
        Chosen::new(&[self.0], device)
    }
}

/// An episode of a task as Burn's runners step it, cut off after
/// `MAX_STEPS` steps.
pub struct Environment<const MAX_STEPS: usize> {
    episode: Episode,
    steps: usize,
}

impl<const MAX_STEPS: usize> Environment<MAX_STEPS> {
    /// An episode of `task`, at its start.
    pub fn new(task: Arc<Task>) -> Self {
        Self {
            episode: Episode::new(task),
            steps: 0,
        }
    }
}

impl<const MAX_STEPS: usize> burn::rl::Environment for Environment<MAX_STEPS> {
    type State = State;
    type Action = Action;

    const MAX_STEPS: usize = MAX_STEPS;

    fn state(&self) -> State {
        State(self.episode.observation().clone())
    }

    /// Applies the chosen exchange.
    ///
    /// # Panics
    ///
    /// Where the engine fails, or the candidate set does not offer the
    /// action: Burn's environments cannot fail, and a learner choosing an
    /// exchange that is not offered is a learner's bug, so training stops
    /// with the error rather than going on.
    fn step(&mut self, action: Action) -> StepResult<State> {
        let step = self
            .episode
            .step(action.0)
            .unwrap_or_else(|error| panic!("{error}"));
        self.steps += 1;
        StepResult {
            next_state: self.state(),
            reward: step.reward,
            done: step.ended(),
            truncated: self.steps >= MAX_STEPS,
        }
    }

    fn reset(&mut self) {
        self.episode.reset();
        self.steps = 0;
    }
}
