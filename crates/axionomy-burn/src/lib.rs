#![doc = include_str!("../README.md")]

mod dqn;
mod environment;
mod silent;

pub use dqn::{
    Chosen, Dqn, DqnLearner, Explore, Observed, Played, QNetwork, Settings, UNOFFERED, Values,
    play, train,
};
pub use environment::{Action, Environment, State};
