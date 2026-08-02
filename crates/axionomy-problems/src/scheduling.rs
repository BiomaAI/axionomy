//! A two-job, two-machine scheduling problem with discrete capacity assets.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, Trace,
    basket,
};
use axionomy_search::{SearchSolution, best_first};
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
    JobIdentity(Job),
    SlotIdentity(Machine, u8),
    ReadyAt(Operation, u8),
    CompletedAt(Operation, u8),
    Available,
    Reserved(Operation),
    Makespan(u8),
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    PrimaryJob,
    SecondaryJob,
    Slot0,
    Slot1,
    Schedule,
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
pub fn branch_optimize(world: &World) -> Option<OptimizedProposal> {
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
            Account::from(basket([
                (Asset::JobIdentity(Job::One), 1),
                (Asset::ReadyAt(Operation::OneA, 0), 1),
            ])),
        )
        .account(
            AccountId::Job(Job::Two),
            Account::from(basket([
                (Asset::JobIdentity(Job::Two), 1),
                (Asset::ReadyAt(Operation::TwoA, 0), 1),
            ])),
        )
        .account(
            AccountId::Success,
            Account::from(basket([(Asset::Active, 1)])),
        );
    for machine in [Machine::One, Machine::Two] {
        for time in 0..horizon {
            builder = builder.account(
                AccountId::Slot(machine, time),
                Account::from(basket([
                    (Asset::SlotIdentity(machine, time), 1),
                    (Asset::Available, 1),
                ])),
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
                        basket([
                            (Asset::JobIdentity(Job::One), 1),
                            (Asset::CompletedAt(Operation::OneB, one_end), 1),
                        ]),
                    )
                    .preserve(
                        Role::SecondaryJob,
                        basket([
                            (Asset::JobIdentity(Job::Two), 1),
                            (Asset::CompletedAt(Operation::TwoB, two_end), 1),
                        ]),
                    )
                    .consume(Role::Schedule, basket([(Asset::Active, 1)]))
                    .produce(
                        Role::Schedule,
                        basket([(Asset::Makespan(makespan), 1), (Asset::Solved, 1)]),
                    )
                    .distinct(Role::PrimaryJob, Role::SecondaryJob)
                    .distinct(Role::PrimaryJob, Role::Schedule)
                    .distinct(Role::SecondaryJob, Role::Schedule),
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

    builder
        .invariant(job_state)
        .invariant(capacity)
        .invariant(
            LinearInvariant::new("schedule lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("scheduling model is valid")
}

fn schedule_rate(operation: Operation, ready: u8, start: u8) -> Rate<Role, Asset> {
    let (job, machine, duration, _) = operation_spec(operation);
    let end = start + duration;
    let mut rate = Rate::new()
        .preserve(Role::Schedule, basket([(Asset::Active, 1)]))
        .preserve(Role::PrimaryJob, basket([(Asset::JobIdentity(job), 1)]))
        .preserve(
            Role::Slot0,
            basket([(Asset::SlotIdentity(machine, start), 1)]),
        )
        .consume(
            Role::PrimaryJob,
            basket([(Asset::ReadyAt(operation, ready), 1)]),
        )
        .consume(Role::Slot0, basket([(Asset::Available, 1)]))
        .produce(Role::Slot0, basket([(Asset::Reserved(operation), 1)]))
        .distinct(Role::Schedule, Role::PrimaryJob)
        .distinct(Role::Schedule, Role::Slot0)
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
            .preserve(
                Role::Slot1,
                basket([(Asset::SlotIdentity(machine, start + 1), 1)]),
            )
            .consume(Role::Slot1, basket([(Asset::Available, 1)]))
            .produce(Role::Slot1, basket([(Asset::Reserved(operation), 1)]))
            .distinct(Role::Schedule, Role::Slot1)
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
    let exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::Schedule, AccountId::Success);
    match rate {
        RateId::Schedule {
            operation, start, ..
        } => {
            let (job, machine, duration, _) = operation_spec(operation);
            let mut exchange = exchange
                .bind(Role::PrimaryJob, AccountId::Job(job))
                .bind(Role::Slot0, AccountId::Slot(machine, start));
            if duration == 2 {
                exchange = exchange.bind(Role::Slot1, AccountId::Slot(machine, start + 1));
            }
            exchange
        }
        RateId::Finish { .. } => exchange
            .bind(Role::PrimaryJob, AccountId::Job(Job::One))
            .bind(Role::SecondaryJob, AccountId::Job(Job::Two)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_optimizer_and_best_first_agree() {
        let world = initial();
        let generic = solve_best_first(&world).expect("schedule is feasible");
        let independent = branch_optimize(&world).expect("schedule is feasible");
        assert_eq!(generic.cost(), 3);
        assert_eq!(independent.makespan(), 3);

        let mut replay = initial();
        replay
            .replay(independent.trace())
            .expect("optimizer proposal must replay");
        assert!(replay.matches(&goal()));
        assert!(candidates(&replay).is_empty());
    }

    #[test]
    fn insufficient_horizon_is_infeasible() {
        let world = impossible();
        assert!(solve_best_first(&world).is_none());
        assert!(branch_optimize(&world).is_none());
    }

    #[test]
    fn operation_cannot_reserve_another_machine_or_time() {
        let world = initial();
        let wrong_machine = Exchange::new(
            RateId::Schedule {
                operation: Operation::OneA,
                ready: 0,
                start: 0,
            },
            Quantity::new(1),
        )
        .bind(Role::PrimaryJob, AccountId::Job(Job::One))
        .bind(Role::Slot0, AccountId::Slot(Machine::Two, 0))
        .bind(Role::Slot1, AccountId::Slot(Machine::Two, 1));

        assert!(!world.is_applicable(&wrong_machine));
    }

    #[test]
    fn small_horizons_match_a_direct_job_shop_oracle() {
        for horizon in 0..=5 {
            let expected = brute_force_makespan(horizon);
            let world = build(horizon);
            let generic = solve_best_first(&world).map(|solution| solution.cost() as u8);
            let branch = branch_optimize(&world).map(|proposal| proposal.makespan());

            assert_eq!(generic, expected, "generic search at horizon {horizon}");
            assert_eq!(branch, expected, "branch search at horizon {horizon}");
        }
    }

    fn brute_force_makespan(horizon: u8) -> Option<u8> {
        let starts = 0..=horizon;
        let mut best = None;
        for one_a in starts.clone() {
            for one_b in starts.clone() {
                for two_a in starts.clone() {
                    for two_b in starts.clone() {
                        let one_a_end = one_a.checked_add(2)?;
                        let one_b_end = one_b.checked_add(1)?;
                        let two_a_end = two_a.checked_add(2)?;
                        let two_b_end = two_b.checked_add(1)?;
                        if one_a_end > horizon
                            || one_b_end > horizon
                            || two_a_end > horizon
                            || two_b_end > horizon
                            || one_b < one_a_end
                            || two_b < two_a_end
                            || overlaps(one_a, one_a_end, two_b, two_b_end)
                            || overlaps(two_a, two_a_end, one_b, one_b_end)
                        {
                            continue;
                        }
                        let makespan = one_b_end.max(two_b_end);
                        best = Some(best.map_or(makespan, |known: u8| known.min(makespan)));
                    }
                }
            }
        }
        best
    }

    const fn overlaps(left_start: u8, left_end: u8, right_start: u8, right_end: u8) -> bool {
        left_start < right_end && right_start < left_end
    }
}
