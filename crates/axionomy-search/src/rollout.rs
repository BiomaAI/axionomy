//! Core-validated speculative trajectory execution.

use axionomy::{ApplyError, Economy, Exchange, Goal, Receipt, Trace};
use std::hash::Hash;

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
pub enum RolloutTermination<RateId, Role, AccountId, A> {
    Terminal,
    Stopped(RolloutStop),
    HorizonReached,
    Rejected(ApplyError<RateId, Role, AccountId, A>),
}

impl<RateId, Role, AccountId, A> RolloutTermination<RateId, Role, AccountId, A> {
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal)
    }
}

#[derive(Debug, Clone)]
pub struct RolloutResult<AccountId, A, RateId, Role> {
    world: Economy<AccountId, A, RateId, Role>,
    trace: Option<Trace<RateId, Role, AccountId>>,
    receipts: Vec<Receipt<RateId, Role, AccountId, A>>,
    termination: RolloutTermination<RateId, Role, AccountId, A>,
    steps: usize,
}

impl<AccountId, A, RateId, Role> RolloutResult<AccountId, A, RateId, Role> {
    pub const fn world(&self) -> &Economy<AccountId, A, RateId, Role> {
        &self.world
    }

    pub fn into_world(self) -> Economy<AccountId, A, RateId, Role> {
        self.world
    }

    pub const fn trace(&self) -> Option<&Trace<RateId, Role, AccountId>> {
        self.trace.as_ref()
    }

    pub fn receipts(&self) -> &[Receipt<RateId, Role, AccountId, A>] {
        &self.receipts
    }

    pub const fn termination(&self) -> &RolloutTermination<RateId, Role, AccountId, A> {
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
pub fn run<AccountId, A, RateId, Role, Controller, Terminal>(
    initial: &Economy<AccountId, A, RateId, Role>,
    config: RolloutConfig,
    mut controller: Controller,
    is_terminal: Terminal,
) -> RolloutResult<AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Controller: FnMut(
        &Economy<AccountId, A, RateId, Role>,
        usize,
    ) -> RolloutDecision<Exchange<RateId, Role, AccountId>>,
    Terminal: Fn(&Economy<AccountId, A, RateId, Role>) -> bool,
{
    let mut world = initial.fork();
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

pub fn run_to_goal<AccountId, A, RateId, Role, Controller>(
    initial: &Economy<AccountId, A, RateId, Role>,
    goal: &Goal<AccountId, A>,
    config: RolloutConfig,
    controller: Controller,
) -> RolloutResult<AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Controller: FnMut(
        &Economy<AccountId, A, RateId, Role>,
        usize,
    ) -> RolloutDecision<Exchange<RateId, Role, AccountId>>,
{
    run(initial, config, controller, |world| world.matches(goal))
}

fn result<AccountId, A, RateId, Role>(
    world: Economy<AccountId, A, RateId, Role>,
    trace: Option<Trace<RateId, Role, AccountId>>,
    receipts: Vec<Receipt<RateId, Role, AccountId, A>>,
    termination: RolloutTermination<RateId, Role, AccountId, A>,
    steps: usize,
) -> RolloutResult<AccountId, A, RateId, Role> {
    RolloutResult {
        world,
        trace,
        receipts,
        termination,
        steps,
    }
}
