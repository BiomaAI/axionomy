//! Core-validated speculative trajectory execution.

use axionomy::Ontology;
use axionomy::{ApplyError, Economy, Exchange, Goal, Receipt, Trace};

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

#[derive(Debug, Clone)]
pub struct RolloutResult<O: Ontology> {
    world: Economy<O>,
    trace: Option<Trace<O>>,
    receipts: Vec<Receipt<O>>,
    termination: RolloutTermination<O>,
    steps: usize,
}

impl<O: Ontology> RolloutResult<O> {
    pub const fn world(&self) -> &Economy<O> {
        &self.world
    }

    pub fn into_world(self) -> Economy<O> {
        self.world
    }

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

/// Executes a speculative trajectory using only core-validated exchanges.
///
/// `is_terminal` must derive terminal truth from encoded economic state.
/// `max_steps` is an algorithmic cutoff and is reported separately.
pub fn run<O: Ontology, Controller, Terminal>(
    initial: &Economy<O>,
    config: RolloutConfig,
    mut controller: Controller,
    is_terminal: Terminal,
) -> RolloutResult<O>
where
    Controller: FnMut(&Economy<O>, usize) -> RolloutDecision<Exchange<O>>,
    Terminal: Fn(&Economy<O>) -> bool,
{
    let mut world = initial.clone();
    let mut trace = matches!(
        config.retention(),
        TraceRetention::Trace | TraceRetention::Full
    )
    .then(Trace::new);
    let mut receipts = Vec::new();
    let mut steps = 0;

    if is_terminal(&world) {
        return result(world, trace, receipts, RolloutTermination::Terminal, steps);
    }

    while steps < config.max_steps() {
        let exchange = match controller(&world, steps) {
            RolloutDecision::Propose(exchange) => exchange,
            RolloutDecision::Stop(reason) => {
                return result(
                    world,
                    trace,
                    receipts,
                    RolloutTermination::Stopped(reason),
                    steps,
                );
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
                return result(
                    world,
                    trace,
                    receipts,
                    RolloutTermination::Rejected(error),
                    steps,
                );
            }
        }

        if is_terminal(&world) {
            return result(world, trace, receipts, RolloutTermination::Terminal, steps);
        }
    }

    result(
        world,
        trace,
        receipts,
        RolloutTermination::HorizonReached,
        steps,
    )
}

pub fn run_to_goal<O: Ontology, Controller>(
    initial: &Economy<O>,
    goal: &Goal<O>,
    config: RolloutConfig,
    controller: Controller,
) -> RolloutResult<O>
where
    Controller: FnMut(&Economy<O>, usize) -> RolloutDecision<Exchange<O>>,
{
    run(initial, config, controller, |world| world.matches(goal))
}

fn result<O: Ontology>(
    world: Economy<O>,
    trace: Option<Trace<O>>,
    receipts: Vec<Receipt<O>>,
    termination: RolloutTermination<O>,
    steps: usize,
) -> RolloutResult<O> {
    RolloutResult {
        world,
        trace,
        receipts,
        termination,
        steps,
    }
}
