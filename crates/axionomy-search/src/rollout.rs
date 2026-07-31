//! Core-validated speculative trajectory execution.

use axionomy::{ApplyError, Economy, Exchange, Goal, QuantityScalar, Receipt, Trace};
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
pub enum RolloutTermination<RateId, Role, AccountId, A, N = u64>
where
    N: QuantityScalar,
{
    Terminal,
    Stopped(RolloutStop),
    HorizonReached,
    Rejected(ApplyError<RateId, Role, AccountId, A, N>),
}

impl<RateId, Role, AccountId, A, N> RolloutTermination<RateId, Role, AccountId, A, N>
where
    N: QuantityScalar,
{
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal)
    }
}

#[derive(Debug, Clone)]
pub struct RolloutResult<AccountId, A, RateId, Role, N = u64>
where
    N: QuantityScalar,
{
    world: Economy<AccountId, A, RateId, Role, N>,
    trace: Option<Trace<RateId, Role, AccountId, N>>,
    receipts: Vec<Receipt<RateId, Role, AccountId, A, N>>,
    termination: RolloutTermination<RateId, Role, AccountId, A, N>,
    steps: usize,
}

impl<AccountId, A, RateId, Role, N> RolloutResult<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    pub const fn world(&self) -> &Economy<AccountId, A, RateId, Role, N> {
        &self.world
    }

    pub fn into_world(self) -> Economy<AccountId, A, RateId, Role, N> {
        self.world
    }

    pub const fn trace(&self) -> Option<&Trace<RateId, Role, AccountId, N>> {
        self.trace.as_ref()
    }

    pub fn receipts(&self) -> &[Receipt<RateId, Role, AccountId, A, N>] {
        &self.receipts
    }

    pub const fn termination(&self) -> &RolloutTermination<RateId, Role, AccountId, A, N> {
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
pub fn run<AccountId, A, RateId, Role, N, Controller, Terminal>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    config: RolloutConfig,
    mut controller: Controller,
    is_terminal: Terminal,
) -> RolloutResult<AccountId, A, RateId, Role, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Controller: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        usize,
    ) -> RolloutDecision<Exchange<RateId, Role, AccountId, N>>,
    Terminal: Fn(&Economy<AccountId, A, RateId, Role, N>) -> bool,
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

pub fn run_to_goal<AccountId, A, RateId, Role, N, Controller>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    config: RolloutConfig,
    controller: Controller,
) -> RolloutResult<AccountId, A, RateId, Role, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Controller: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        usize,
    ) -> RolloutDecision<Exchange<RateId, Role, AccountId, N>>,
{
    run(initial, config, controller, |world| world.matches(goal))
}

fn result<AccountId, A, RateId, Role, N>(
    world: Economy<AccountId, A, RateId, Role, N>,
    trace: Option<Trace<RateId, Role, AccountId, N>>,
    receipts: Vec<Receipt<RateId, Role, AccountId, A, N>>,
    termination: RolloutTermination<RateId, Role, AccountId, A, N>,
    steps: usize,
) -> RolloutResult<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    RolloutResult {
        world,
        trace,
        receipts,
        termination,
        steps,
    }
}
