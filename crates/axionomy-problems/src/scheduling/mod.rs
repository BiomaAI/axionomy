#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, basket,
};

pub mod execution;
#[cfg(feature = "packages")]
pub mod package;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Job {
    One,
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Machine {
    One,
    Two,
    Three,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Operation {
    OneA,
    OneB,
    OneC,
    TwoA,
    TwoB,
    TwoC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Job(Job),
    Slot(Machine, u8),
    Success,
    Machine(Machine),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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
    MachineIdentity(Machine),
    Ready(Operation),
    Queued(Operation, Machine),
    Running(Operation, Machine),
    Paused(Operation, Machine),
    QueueFree,
    QueueTicket(Operation),
    Idle,
    Busy,
    Online,
    Broken,
    Repairing,
    Configured(Job),
    Labor,
    BusyLabor,
    RepairKit,
    UsedRepairKit,
    Arrival,
    Due,
    WorkRemaining,
    Done,
    CompletionTime,
    Now,
    TimeRemaining,
    FaultAt,
    FaultPending,
    FaultOccurred,
    RepairDuration,
    NotStarted(Operation),
    Started(Operation),
    StartTime(Operation),
    EndTime(Operation),
    ExecutedOn(Operation, Machine),
    Interrupted(Operation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    PrimaryJob,
    SecondaryJob,
    Slot0,
    Slot1,
    Schedule,
    Machine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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
    Queue {
        operation: Operation,
        machine: Machine,
    },
    CancelQueue {
        operation: Operation,
        machine: Machine,
    },
    Start {
        operation: Operation,
        machine: Machine,
        configured: Job,
    },
    Complete {
        operation: Operation,
        machine: Machine,
    },
    Resume {
        operation: Operation,
        machine: Machine,
    },
    Fault {
        operation: Option<Operation>,
    },
    Repair,
    CompleteRepair,
    Advance {
        to: u8,
    },
    Close,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `scheduling` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "scheduling";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub fn initial() -> World {
    build(5, false)
}

pub fn impossible() -> World {
    build(2, false)
}

/// Two three-operation jobs competing across three discrete-capacity machines.
pub fn initial_showcase() -> World {
    execution::build(false)
}

pub fn initial_stress() -> World {
    execution::build(true)
}

pub fn impossible_showcase() -> World {
    build(3, true)
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

fn build(horizon: u8, extended: bool) -> World {
    let operations: &[Operation] = if extended {
        &[
            Operation::OneA,
            Operation::OneB,
            Operation::OneC,
            Operation::TwoA,
            Operation::TwoB,
            Operation::TwoC,
        ]
    } else {
        &[
            Operation::OneA,
            Operation::OneB,
            Operation::TwoA,
            Operation::TwoB,
        ]
    };
    let final_operations = if extended {
        (Operation::OneC, Operation::TwoC)
    } else {
        (Operation::OneB, Operation::TwoB)
    };
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
    let machines: &[Machine] = if extended {
        &[Machine::One, Machine::Two, Machine::Three]
    } else {
        &[Machine::One, Machine::Two]
    };
    for &machine in machines {
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

    for &operation in operations {
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
                    schedule_rate(
                        operation,
                        ready,
                        start,
                        operation == final_operations.0 || operation == final_operations.1,
                    ),
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
                            (Asset::CompletedAt(final_operations.0, one_end), 1),
                        ]),
                    )
                    .preserve(
                        Role::SecondaryJob,
                        basket([
                            (Asset::JobIdentity(Job::Two), 1),
                            (Asset::CompletedAt(final_operations.1, two_end), 1),
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

    let job_state = operations.iter().copied().fold(
        LinearInvariant::new(path!("two-job-state-tokens")),
        |invariant, operation| {
            (0..=horizon).fold(invariant, |invariant, time| {
                invariant
                    .weight(Asset::ReadyAt(operation, time), 1)
                    .weight(Asset::CompletedAt(operation, time), 1)
            })
        },
    );
    let capacity = operations.iter().copied().fold(
        LinearInvariant::new(path!("machine-slot-capacity")).weight(Asset::Available, 1),
        |invariant, operation| invariant.weight(Asset::Reserved(operation), 1),
    );

    builder
        .invariant(job_state)
        .invariant(capacity)
        .invariant(
            LinearInvariant::new(path!("schedule-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("scheduling model is valid")
}

fn schedule_rate(
    operation: Operation,
    ready: u8,
    start: u8,
    final_operation: bool,
) -> Rate<Ontology> {
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
    if !final_operation {
        let next = successor(operation).expect("non-final operation has a successor");
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
        Operation::OneC => (Job::One, Machine::Three, 1, 1),
        Operation::TwoA => (Job::Two, Machine::One, 2, 0),
        Operation::TwoB => (Job::Two, Machine::Two, 1, 2),
        Operation::TwoC => (Job::Two, Machine::Three, 1, 1),
    }
}

fn successor(operation: Operation) -> Option<Operation> {
    match operation {
        Operation::OneA => Some(Operation::OneB),
        Operation::OneB => Some(Operation::OneC),
        Operation::TwoA => Some(Operation::TwoB),
        Operation::TwoB => Some(Operation::TwoC),
        Operation::OneC | Operation::TwoC => None,
    }
}

/// The exchange applying `rate` once, bound to the schedule and to the jobs,
/// slots and machine the rate names.
pub fn action(rate: RateId) -> Action {
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
        RateId::Queue { operation, machine }
        | RateId::CancelQueue { operation, machine }
        | RateId::Start {
            operation, machine, ..
        }
        | RateId::Complete { operation, machine }
        | RateId::Resume { operation, machine } => exchange
            .bind(
                Role::PrimaryJob,
                AccountId::Job(operation_spec(operation).0),
            )
            .bind(Role::Machine, AccountId::Machine(machine)),
        RateId::Fault { operation } => {
            let exchange = exchange.bind(Role::Machine, AccountId::Machine(Machine::Two));
            if let Some(operation) = operation {
                exchange.bind(
                    Role::PrimaryJob,
                    AccountId::Job(operation_spec(operation).0),
                )
            } else {
                exchange
            }
        }
        RateId::Repair | RateId::CompleteRepair => {
            exchange.bind(Role::Machine, AccountId::Machine(Machine::Two))
        }
        RateId::Advance { .. } => exchange,
        RateId::Close => exchange
            .bind(Role::PrimaryJob, AccountId::Job(Job::One))
            .bind(Role::SecondaryJob, AccountId::Job(Job::Two)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The makespan `world` records once it accepts scheduling each job's two
    /// operations at `starts` (`[one_a, one_b, two_a, two_b]`), each second
    /// operation ready when its first ends, and finishing; nothing if it
    /// refuses any of it.
    fn accepts(world: &World, [one_a, one_b, two_a, two_b]: [u8; 4]) -> Option<u8> {
        let (job_one_end, job_two_end) = (one_b + 1, two_b + 1);
        let schedule = [
            RateId::Schedule {
                operation: Operation::OneA,
                ready: 0,
                start: one_a,
            },
            RateId::Schedule {
                operation: Operation::OneB,
                ready: one_a + 2,
                start: one_b,
            },
            RateId::Schedule {
                operation: Operation::TwoA,
                ready: 0,
                start: two_a,
            },
            RateId::Schedule {
                operation: Operation::TwoB,
                ready: two_a + 2,
                start: two_b,
            },
            RateId::Finish {
                job_one_end,
                job_two_end,
                makespan: job_one_end.max(job_two_end),
            },
        ];
        let end = world
            .replayed(&schedule.into_iter().map(action).collect())
            .ok()?;
        (0..=16).find(|makespan| {
            !end.balance(&AccountId::Success, &Asset::Makespan(*makespan))
                .is_zero()
        })
    }

    /// The oracle, independent of the economy: the makespan of starting the
    /// operations at `starts` within `horizon`, if that schedule is feasible.
    fn oracle(horizon: u8, [one_a, one_b, two_a, two_b]: [u8; 4]) -> Option<u8> {
        let (one_a_end, one_b_end) = (one_a + 2, one_b + 1);
        let (two_a_end, two_b_end) = (two_a + 2, two_b + 1);
        let feasible = one_a_end <= horizon
            && one_b_end <= horizon
            && two_a_end <= horizon
            && two_b_end <= horizon
            && one_b >= one_a_end
            && two_b >= two_a_end
            && !overlaps(one_a, one_a_end, two_a, two_a_end)
            && !overlaps(one_b, one_b_end, two_b, two_b_end);
        feasible.then_some(one_b_end.max(two_b_end))
    }

    const fn overlaps(left_start: u8, left_end: u8, right_start: u8, right_end: u8) -> bool {
        left_start < right_end && right_start < left_end
    }

    #[test]
    fn a_five_slot_schedule_finishes_and_is_quiescent() {
        let world = initial();
        let solved = world
            .replayed(
                &[
                    RateId::Schedule {
                        operation: Operation::OneA,
                        ready: 0,
                        start: 0,
                    },
                    RateId::Schedule {
                        operation: Operation::OneB,
                        ready: 2,
                        start: 2,
                    },
                    RateId::Schedule {
                        operation: Operation::TwoA,
                        ready: 0,
                        start: 2,
                    },
                    RateId::Schedule {
                        operation: Operation::TwoB,
                        ready: 4,
                        start: 4,
                    },
                    RateId::Finish {
                        job_one_end: 3,
                        job_two_end: 5,
                        makespan: 5,
                    },
                ]
                .into_iter()
                .map(action)
                .collect(),
            )
            .expect("the schedule replays");
        assert!(solved.matches(&goal()));
        assert_eq!(
            solved
                .balance(&AccountId::Success, &Asset::Makespan(5))
                .to_u64(),
            1
        );
        assert!(
            solved
                .rate_ids()
                .all(|rate| !solved.is_applicable(&action(*rate)))
        );
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
            // Horizon 2 is `impossible()`'s.
            let world = build(horizon, false);
            let mut least = None;
            for one_a in 0..=horizon {
                for one_b in 0..=horizon {
                    for two_a in 0..=horizon {
                        for two_b in 0..=horizon {
                            let starts = [one_a, one_b, two_a, two_b];
                            let expected = oracle(horizon, starts);
                            assert_eq!(
                                accepts(&world, starts),
                                expected,
                                "horizon {horizon}: {starts:?}"
                            );
                            least = least.into_iter().chain(expected).min();
                        }
                    }
                }
            }
            assert_eq!(least, (horizon >= 5).then_some(5), "horizon {horizon}");
        }
    }
}
