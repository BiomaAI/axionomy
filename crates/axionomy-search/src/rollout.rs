//! Core-validated speculative trajectory execution.
//!
//! A rollout steps whatever it is given through [`Apply`]: an economy of its
//! own, so the steps live and die in memory, or a recorder that keeps them.
//! [`Episodes`] makes the same choice for repeated trials.

use axionomy::Ontology;
use axionomy::{Apply, ApplyError, Economy, Exchange, Goal, Receipt, Trace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceRetention {
    None,
    Trace,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RolloutConfig {
    max_steps: usize,
    retention: TraceRetention,
}

impl RolloutConfig {
    pub const fn new(max_steps: usize) -> Self {
        Self {
            max_steps,
            retention: TraceRetention::Trace,
        }
    }

    pub const fn with_retention(mut self, retention: TraceRetention) -> Self {
        self.retention = retention;
        self
    }

    pub const fn max_steps(self) -> usize {
        self.max_steps
    }

    pub const fn retention(self) -> TraceRetention {
        self.retention
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutStop {
    NoProposal,
    PolicyStopped,
}

#[derive(Debug, Clone)]
pub enum RolloutDecision<Action> {
    Propose(Action),
    Stop(RolloutStop),
}

#[derive(Debug, Clone)]
pub enum RolloutTermination<O: Ontology> {
    Terminal,
    Stopped(RolloutStop),
    HorizonReached,
    Rejected(ApplyError<O>),
}

impl<O: Ontology> RolloutTermination<O> {
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal)
    }
}

/// How a rollout went. Its final state is the one it stepped.
#[derive(Debug, Clone)]
pub struct RolloutResult<O: Ontology> {
    trace: Option<Trace<O>>,
    receipts: Vec<Receipt<O>>,
    termination: RolloutTermination<O>,
    steps: usize,
}

impl<O: Ontology> RolloutResult<O> {
    pub const fn trace(&self) -> Option<&Trace<O>> {
        self.trace.as_ref()
    }

    pub fn receipts(&self) -> &[Receipt<O>] {
        &self.receipts
    }

    pub const fn termination(&self) -> &RolloutTermination<O> {
        &self.termination
    }

    pub const fn steps(&self) -> usize {
        self.steps
    }
}

/// Executes a speculative trajectory on `world` using only core-validated
/// exchanges, leaving `world` at its final state.
///
/// `is_terminal` must derive terminal truth from encoded economic state.
/// `max_steps` is an algorithmic cutoff and is reported separately.
pub fn run<O: Ontology, World, Controller, Terminal>(
    world: &mut World,
    config: RolloutConfig,
    mut controller: Controller,
    is_terminal: Terminal,
) -> RolloutResult<O>
where
    World: Apply<O> + ?Sized,
    Controller: FnMut(&Economy<O>, usize) -> RolloutDecision<Exchange<O>>,
    Terminal: Fn(&Economy<O>) -> bool,
{
    let mut trace = matches!(
        config.retention(),
        TraceRetention::Trace | TraceRetention::Full
    )
    .then(Trace::new);
    let mut receipts = Vec::new();
    let mut steps = 0;

    if is_terminal(world.economy()) {
        return result(trace, receipts, RolloutTermination::Terminal, steps);
    }

    while steps < config.max_steps() {
        let exchange = match controller(world.economy(), steps) {
            RolloutDecision::Propose(exchange) => exchange,
            RolloutDecision::Stop(reason) => {
                return result(trace, receipts, RolloutTermination::Stopped(reason), steps);
            }
        };

        match world.apply(exchange.clone()) {
            Ok(receipt) => {
                steps += 1;
                if let Some(trace) = &mut trace {
                    trace.push(exchange);
                }
                if config.retention() == TraceRetention::Full {
                    receipts.push(receipt);
                }
            }
            Err(error) => {
                return result(trace, receipts, RolloutTermination::Rejected(error), steps);
            }
        }

        if is_terminal(world.economy()) {
            return result(trace, receipts, RolloutTermination::Terminal, steps);
        }
    }

    result(trace, receipts, RolloutTermination::HorizonReached, steps)
}

/// A rollout that ends when `world` matches `goal`.
pub fn run_to_goal<O: Ontology, World, Controller>(
    world: &mut World,
    goal: &Goal<O>,
    config: RolloutConfig,
    controller: Controller,
) -> RolloutResult<O>
where
    World: Apply<O> + ?Sized,
    Controller: FnMut(&Economy<O>, usize) -> RolloutDecision<Exchange<O>>,
{
    run(world, config, controller, |economy| economy.matches(goal))
}

/// Where the rollouts of a repeated trial go, one episode per sample of
/// each policy: nowhere, as [`Unrecorded`] does, or somewhere that records
/// them, such as a store keeping one run per episode in a group per policy.
pub trait Episodes<O: Ontology, Policy, Outcome> {
    /// What an episode's exchanges are applied to: an economy of its own,
    /// or a recorder.
    type World: Apply<O>;

    /// Starts sample `sample` of `policy` from `initial`.
    fn begin(&mut self, policy: &Policy, sample: usize, initial: Economy<O>) -> Self::World;

    /// Ends the episode with what it produced. A destination that writes
    /// somewhere reports its failures when it is finished, not here, so
    /// recording never changes what an experiment observes.
    fn end(&mut self, policy: &Policy, sample: usize, world: Self::World, outcome: &Outcome);
}

/// Episodes nobody keeps: each steps its own fork of the economy, dropped
/// when it ends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unrecorded;

impl<O: Ontology, Policy, Outcome> Episodes<O, Policy, Outcome> for Unrecorded {
    type World = Economy<O>;

    fn begin(&mut self, _: &Policy, _: usize, initial: Economy<O>) -> Economy<O> {
        initial
    }

    fn end(&mut self, _: &Policy, _: usize, _: Economy<O>, _: &Outcome) {}
}

fn result<O: Ontology>(
    trace: Option<Trace<O>>,
    receipts: Vec<Receipt<O>>,
    termination: RolloutTermination<O>,
    steps: usize,
) -> RolloutResult<O> {
    RolloutResult {
        trace,
        receipts,
        termination,
        steps,
    }
}
