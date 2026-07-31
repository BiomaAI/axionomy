//! A two-job, two-machine scheduling problem with discrete capacity assets.

use crate::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    SearchSolution, Trace, basket, best_first,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Job {
    One,
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Machine {
    One,
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Operation {
    OneA,
    OneB,
    TwoA,
    TwoB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Job(Job),
    Slot(Machine, u8),
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    ReadyAt(Operation, u8),
    CompletedAt(Operation, u8),
    Available,
    Reserved(Operation),
    Makespan(u8),
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    PrimaryJob,
    SecondaryJob,
    Slot0,
    Slot1,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Schedule {
        operation: Operation,
        ready: u8,
        start: u8,
    },
    Finish {
        job_one_end: u8,
        job_two_end: u8,
        makespan: u8,
    },
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;

#[derive(Debug, Clone)]
pub struct OptimizedProposal {
    trace: Trace<RateId, Role, AccountId>,
    makespan: u8,
}

impl OptimizedProposal {
    pub fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub fn makespan(&self) -> u8 {
        self.makespan
    }
}

pub fn initial() -> World {
    build(5)
}

pub fn impossible() -> World {
    build(2)
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}

pub fn solve_best_first(world: &World) -> Option<Solution> {
    best_first(world, &goal(), candidates, encoded_makespan, |_| 0)
}

/// A separate depth-first branch enumerator. It understands no scheduling
/// internals: it forks the economy, asks for applicable exchanges, and reads
/// the objective asset from completed states.
pub fn independent_optimize(world: &World) -> Option<OptimizedProposal> {
    let mut visited = HashSet::new();
    let mut best: Option<OptimizedProposal> = None;
    optimize_branch(world.clone(), Trace::new(), &mut visited, &mut best);
    let proposal = best?;
    let mut replay = world.clone();
    replay.replay(&proposal.trace).ok()?;
    (replay.matches(&goal()) && encoded_makespan(&replay) == u64::from(proposal.makespan))
        .then_some(proposal)
}

pub fn encoded_makespan(world: &World) -> u64 {
    (0..=16)
        .find(|makespan| {
            !world
                .balance(
                    &AccountId::Success,
                    &Asset::Makespan(u8::try_from(*makespan).expect("bounded")),
                )
                .is_zero()
        })
        .unwrap_or(0)
}

fn optimize_branch(
    world: World,
    trace: Trace<RateId, Role, AccountId>,
    visited: &mut HashSet<Vec<(AccountId, Asset, Quantity)>>,
    best: &mut Option<OptimizedProposal>,
) {
    if !visited.insert(world.state_key()) {
        return;
    }
    if world.matches(&goal()) {
        let candidate = u8::try_from(encoded_makespan(&world)).expect("encoded u8 makespan");
        if best.as_ref().is_none_or(|known| candidate < known.makespan) {
            *best = Some(OptimizedProposal {
                trace,
                makespan: candidate,
            });
        }
        return;
    }

    for exchange in candidates(&world) {
        let mut next = world.clone();
        if next.apply(exchange.clone()).is_err() {
            continue;
        }
        let mut next_trace = trace.clone();
        next_trace.push(exchange);
        optimize_branch(next, next_trace, visited, best);
    }
}

fn build(horizon: u8) -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Job(Job::One),
            Account::from(basket([(Asset::ReadyAt(Operation::OneA, 0), 1)])),
        )
        .account(
            AccountId::Job(Job::Two),
            Account::from(basket([(Asset::ReadyAt(Operation::TwoA, 0), 1)])),
        )
        .account(AccountId::Success, Account::default());
    for machine in [Machine::One, Machine::Two] {
        for time in 0..horizon {
            builder = builder.account(
                AccountId::Slot(machine, time),
                Account::from(basket([(Asset::Available, 1)])),
            );
        }
    }

    for operation in [
        Operation::OneA,
        Operation::OneB,
        Operation::TwoA,
        Operation::TwoB,
    ] {
        let (_, _, duration, predecessor_duration) = operation_spec(operation);
        if duration > horizon {
            continue;
        }
        let ready_times: Vec<u8> = if predecessor_duration == 0 {
            vec![0]
        } else {
            (predecessor_duration..=horizon).collect()
        };
        for ready in ready_times {
            if ready > horizon - duration {
                continue;
            }
            for start in ready..=horizon - duration {
                builder = builder.rate(
                    RateId::Schedule {
                        operation,
                        ready,
                        start,
                    },
                    schedule_rate(operation, ready, start),
                );
            }
        }
    }

    for one_end in 0..=horizon {
        for two_end in 0..=horizon {
            let makespan = one_end.max(two_end);
            builder = builder.rate(
                RateId::Finish {
                    job_one_end: one_end,
                    job_two_end: two_end,
                    makespan,
                },
                Rate::new()
                    .preserve(
                        Role::PrimaryJob,
                        basket([(Asset::CompletedAt(Operation::OneB, one_end), 1)]),
                    )
                    .preserve(
                        Role::SecondaryJob,
                        basket([(Asset::CompletedAt(Operation::TwoB, two_end), 1)]),
                    )
                    .produce(
                        Role::Goal,
                        basket([(Asset::Makespan(makespan), 1), (Asset::Solved, 1)]),
                    )
                    .distinct(Role::PrimaryJob, Role::SecondaryJob)
                    .distinct(Role::PrimaryJob, Role::Goal)
                    .distinct(Role::SecondaryJob, Role::Goal),
            );
        }
    }

    let job_state = [
        Operation::OneA,
        Operation::OneB,
        Operation::TwoA,
        Operation::TwoB,
    ]
    .into_iter()
    .fold(
        LinearInvariant::new("two job state tokens"),
        |invariant, operation| {
            (0..=horizon).fold(invariant, |invariant, time| {
                invariant
                    .weight(Asset::ReadyAt(operation, time), 1)
                    .weight(Asset::CompletedAt(operation, time), 1)
            })
        },
    );
    let capacity = [
        Operation::OneA,
        Operation::OneB,
        Operation::TwoA,
        Operation::TwoB,
    ]
    .into_iter()
    .fold(
        LinearInvariant::new("machine slot capacity").weight(Asset::Available, 1),
        |invariant, operation| invariant.weight(Asset::Reserved(operation), 1),
    );

    builder.invariant(job_state).invariant(capacity).build()
}

fn schedule_rate(operation: Operation, ready: u8, start: u8) -> Rate<Role, Asset> {
    let (_, _, duration, _) = operation_spec(operation);
    let end = start + duration;
    let mut rate = Rate::new()
        .consume(
            Role::PrimaryJob,
            basket([(Asset::ReadyAt(operation, ready), 1)]),
        )
        .consume(Role::Slot0, basket([(Asset::Available, 1)]))
        .produce(Role::Slot0, basket([(Asset::Reserved(operation), 1)]))
        .distinct(Role::PrimaryJob, Role::Slot0);
    if let Some(next) = successor(operation) {
        rate = rate.produce(Role::PrimaryJob, basket([(Asset::ReadyAt(next, end), 1)]));
    } else {
        rate = rate.produce(
            Role::PrimaryJob,
            basket([(Asset::CompletedAt(operation, end), 1)]),
        );
    }
    if duration == 2 {
        rate = rate
            .consume(Role::Slot1, basket([(Asset::Available, 1)]))
            .produce(Role::Slot1, basket([(Asset::Reserved(operation), 1)]))
            .distinct(Role::PrimaryJob, Role::Slot1)
            .distinct(Role::Slot0, Role::Slot1);
    }
    rate
}

fn operation_spec(operation: Operation) -> (Job, Machine, u8, u8) {
    match operation {
        Operation::OneA => (Job::One, Machine::One, 2, 0),
        Operation::OneB => (Job::One, Machine::Two, 1, 2),
        Operation::TwoA => (Job::Two, Machine::Two, 2, 0),
        Operation::TwoB => (Job::Two, Machine::One, 1, 2),
    }
}

fn successor(operation: Operation) -> Option<Operation> {
    match operation {
        Operation::OneA => Some(Operation::OneB),
        Operation::TwoA => Some(Operation::TwoB),
        Operation::OneB | Operation::TwoB => None,
    }
}

fn action(rate: RateId) -> Action {
    match rate {
        RateId::Schedule {
            operation, start, ..
        } => {
            let (job, machine, duration, _) = operation_spec(operation);
            let mut exchange = Exchange::new(rate, Quantity::new(1))
                .bind(Role::PrimaryJob, AccountId::Job(job))
                .bind(Role::Slot0, AccountId::Slot(machine, start));
            if duration == 2 {
                exchange = exchange.bind(Role::Slot1, AccountId::Slot(machine, start + 1));
            }
            exchange
        }
        RateId::Finish { .. } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::PrimaryJob, AccountId::Job(Job::One))
            .bind(Role::SecondaryJob, AccountId::Job(Job::Two))
            .bind(Role::Goal, AccountId::Success),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_optimizer_and_best_first_agree() {
        let world = initial();
        let generic = solve_best_first(&world).expect("schedule is feasible");
        let independent = independent_optimize(&world).expect("schedule is feasible");
        assert_eq!(generic.cost(), 3);
        assert_eq!(independent.makespan(), 3);

        let mut replay = initial();
        replay
            .replay(independent.trace())
            .expect("optimizer proposal must replay");
        assert!(replay.matches(&goal()));
    }

    #[test]
    fn insufficient_horizon_is_infeasible() {
        let world = impossible();
        assert!(solve_best_first(&world).is_none());
        assert!(independent_optimize(&world).is_none());
    }
}
