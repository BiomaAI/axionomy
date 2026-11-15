#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern, Quantity,
    QuantityExpression as Q, Rate, Relation, Segment, Slot, basket,
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
    ReadyAt(Operation, u8),
    CompletedAt(Operation, u8),
    Available,
    Reserved(Operation),
    Makespan(u8),
    Active,
    Solved,
    /// How many ticks a machine takes to run an operation: a fact of the
    /// machine.
    ProcessingTime(Operation),
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
            Account::from(basket([(Asset::ReadyAt(Operation::OneA, 0), 1)])),
        )
        .account(
            AccountId::Job(Job::Two),
            Account::from(basket([(Asset::ReadyAt(Operation::TwoA, 0), 1)])),
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
                Account::from(basket([(Asset::Available, 1)])),
            );
        }
    }

    // Each operation is reserved ready at `{ready}` and started at
    // `{start}`, no earlier, its last slot within the horizon.
    for &operation in operations {
        let (_, _, duration, predecessor_duration) = operation_spec(operation);
        let Some(latest) = horizon.checked_sub(duration) else {
            continue;
        };
        let ready_times: Vec<u8> = if predecessor_duration == 0 {
            vec![0]
        } else {
            (predecessor_duration..=latest).collect()
        };
        if ready_times.is_empty() {
            continue;
        }
        builder = builder.rate_at(
            RateIdPattern::Schedule {
                operation: Slot::Is(operation),
                ready: ready().var(),
                start: start().var(),
            }
            .into(),
            schedule_rate(
                operation,
                &ready_times,
                latest,
                operation == final_operations.0 || operation == final_operations.1,
            ),
        );
    }

    // Finishing reads when each job completed; the makespan is the later.
    let (one_end, two_end, makespan) = (
        Param::<u8>::new(path!("job-one-end")),
        Param::<u8>::new(path!("job-two-end")),
        Param::<u8>::new(path!("makespan")),
    );
    let ends = (0..=horizon).flat_map(|one| {
        (0..=horizon)
            .map(move |two| [one, two, one.max(two)].map(|end| Segment::from(u64::from(end))))
    });
    builder = builder.rate_at(
        RateIdPattern::Finish {
            job_one_end: one_end.var(),
            job_two_end: two_end.var(),
            makespan: makespan.var(),
        }
        .into(),
        planned()
            .variable(one_end.range(0, horizon))
            .variable(two_end.range(0, horizon))
            .variable(makespan.range(0, horizon))
            .relation(Relation::new(
                [one_end.name(), two_end.name(), makespan.name()].map(Clone::clone),
                ends,
            ))
            .account(Role::PrimaryJob, Pattern::literal(AccountId::Job(Job::One)))
            .account(
                Role::SecondaryJob,
                Pattern::literal(AccountId::Job(Job::Two)),
            )
            .preserve_at(
                Role::PrimaryJob,
                one([
                    AssetPattern::CompletedAt(Slot::Is(final_operations.0), one_end.var()).into(),
                ]),
            )
            .preserve_at(
                Role::SecondaryJob,
                one([
                    AssetPattern::CompletedAt(Slot::Is(final_operations.1), two_end.var()).into(),
                ]),
            )
            .consume(Role::Schedule, basket([(Asset::Active, 1)]))
            .produce_at(
                Role::Schedule,
                one([
                    AssetPattern::Makespan(makespan.var()).into(),
                    Pattern::literal(Asset::Solved),
                ]),
            ),
    );

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

/// The time a reservation's operation is ready at, `{ready}`.
fn ready() -> Param<u8> {
    Param::new(path!("ready"))
}

/// The slot a reservation starts at, `{start}`.
fn start() -> Param<u8> {
    Param::new(path!("start"))
}

/// One unit of each asset `assets` names.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}

/// An event of a planned schedule, its schedule role fixed to the schedule.
fn planned() -> Rate<Ontology> {
    Rate::new().account(Role::Schedule, Pattern::literal(AccountId::Success))
}

/// Reserving `operation`, ready at one of `ready_times`, from `{start}` no
/// earlier and no later than `latest`: it takes its machine's slots for its
/// duration, a constant of the law, and the operation it unlocks, or its
/// completion, is ready at the end it derives, `{end}`.
fn schedule_rate(
    operation: Operation,
    ready_times: &[u8],
    latest: u8,
    final_operation: bool,
) -> Rate<Ontology> {
    let (job, machine, duration, _) = operation_spec(operation);
    let end = Param::<u8>::new(path!("end"));
    let slot = |time: Slot<u8>| AccountIdPattern::Slot(Slot::Is(machine), time).into();
    let starts = ready_times
        .iter()
        .flat_map(|&ready| (ready..=latest).map(move |start| (ready, start)));
    let mut rate = planned()
        .variable(ready().values(ready_times.iter().copied()))
        .variable(start().range(ready_times[0], latest))
        .relation(Relation::pairs(&ready(), &start(), starts))
        .derive(
            end.name().clone(),
            start().quantity() + Q::constant(u64::from(duration)),
        )
        .account(Role::PrimaryJob, Pattern::literal(AccountId::Job(job)))
        .account(Role::Slot0, slot(start().var()))
        .preserve(Role::Schedule, basket([(Asset::Active, 1)]))
        .consume_at(
            Role::PrimaryJob,
            one([AssetPattern::ReadyAt(Slot::Is(operation), ready().var()).into()]),
        )
        .consume(Role::Slot0, basket([(Asset::Available, 1)]))
        .produce(Role::Slot0, basket([(Asset::Reserved(operation), 1)]));
    let unlocked = match successor(operation) {
        Some(next) if !final_operation => AssetPattern::ReadyAt(Slot::Is(next), end.var()),
        _ => AssetPattern::CompletedAt(Slot::Is(operation), end.var()),
    };
    rate = rate.produce_at(Role::PrimaryJob, one([unlocked.into()]));
    if duration == 2 {
        let second = Param::<u8>::new(path!("second"));
        rate = rate
            .derive(second.name().clone(), start().quantity() + Q::constant(1))
            .account(Role::Slot1, slot(second.var()))
            .consume(Role::Slot1, basket([(Asset::Available, 1)]))
            .produce(Role::Slot1, basket([(Asset::Reserved(operation), 1)]));
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

/// The exchange of one unit of `rate`, each role bound to the account
/// `world`'s rate fixes it to; bound to nothing where `rate` names no
/// instance of `world`'s rates, so that `world` refuses it.
pub fn action(world: &World, rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    let Ok(instance) = world.instance(&rate) else {
        return exchange;
    };
    instance
        .fixed_accounts()
        .fold(exchange, |exchange, (role, account)| {
            exchange.bind(
                *role,
                *account
                    .as_literal()
                    .expect("an instance's accounts are written out"),
            )
        })
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
            .replayed(
                &schedule
                    .into_iter()
                    .map(|rate| action(world, rate))
                    .collect(),
            )
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
                .map(|rate| action(&world, rate))
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
                .flat_map(|key| solved.instances_of(key))
                .all(|rate| !solved.is_applicable(&action(&solved, rate)))
        );
    }

    #[test]
    fn operation_cannot_reserve_another_machine_or_time() {
        let world = initial();
        let reservation = action(
            &world,
            RateId::Schedule {
                operation: Operation::OneA,
                ready: 0,
                start: 0,
            },
        );
        assert!(world.is_applicable(&reservation));
        for (role, slot) in [
            (Role::Slot0, AccountId::Slot(Machine::Two, 0)),
            (Role::Slot1, AccountId::Slot(Machine::Two, 1)),
            (Role::Slot0, AccountId::Slot(Machine::One, 2)),
        ] {
            assert!(!world.is_applicable(&reservation.clone().bind(role, slot)));
        }
    }

    #[test]
    fn an_operation_ends_its_duration_after_it_starts() {
        let world = initial();
        let started = world
            .replayed(
                &[RateId::Schedule {
                    operation: Operation::OneA,
                    ready: 0,
                    start: 1,
                }]
                .into_iter()
                .map(|rate| action(&world, rate))
                .collect(),
            )
            .unwrap();
        let job = |asset| started.balance(&AccountId::Job(Job::One), &asset).to_u64();
        assert_eq!(job(Asset::ReadyAt(Operation::OneB, 3)), 1);
        // No operation starts before it is ready.
        assert!(
            world
                .instance(&RateId::Schedule {
                    operation: Operation::OneB,
                    ready: 3,
                    start: 2,
                })
                .is_err()
        );
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
