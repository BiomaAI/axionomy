//! Executed job shop with queues, setup, shared labor and recoverable disruption.
//! Each operation's laws serve both machines it may run on, `{machine}`, and
//! the clock advances to any tick, `{to}`; every role is fixed to the account
//! its identifier names.
use super::*;
use axionomy::path;
use axionomy::{OptionPattern, QuantityComparison as C};

pub const OPERATIONS: [Operation; 6] = [
    Operation::OneA,
    Operation::OneB,
    Operation::OneC,
    Operation::TwoA,
    Operation::TwoB,
    Operation::TwoC,
];
pub const MACHINES: [Machine; 3] = [Machine::One, Machine::Two, Machine::Three];
fn clock() -> Q<Ontology> {
    Q::balance(Role::Schedule, Asset::Now)
}
fn law() -> Rate<Ontology> {
    planned()
        .preserve(Role::Schedule, basket([(Asset::Active, 1)]))
        .condition(
            path!("one-scheduling-event"),
            Q::units(),
            C::Equal,
            Q::constant(1),
        )
}
/// The machine a rate's identifier names, `{machine}`.
fn machine() -> Param<Machine> {
    Param::new(path!("machine"))
}
/// A law at `machine`, the machine role fixed to it.
fn machine_law(machine: Machine) -> Rate<Ontology> {
    law().account(Role::Machine, Pattern::literal(AccountId::Machine(machine)))
}
/// `rate` for `operation`, its job role fixed to the operation's job.
fn of_operation(rate: Rate<Ontology>, operation: Operation) -> Rate<Ontology> {
    rate.account(
        Role::PrimaryJob,
        Pattern::literal(AccountId::Job(operation_spec(operation).0)),
    )
}
/// `operation`'s law on either machine it may run on, `{machine}`, the
/// preferred first, the machine role fixed to it.
fn operation_law(operation: Operation) -> Rate<Ontology> {
    of_operation(law(), operation)
        .variable(machine().values(alternatives(operation).map(|(machine, _)| machine)))
        .account(
            Role::Machine,
            AccountIdPattern::Machine(machine().var()).into(),
        )
}
/// `operation` at the stage `stage` writes, on the machine `{machine}`.
fn on_machine(
    stage: fn(Slot<Operation>, Slot<Machine>) -> AssetPattern,
    operation: Operation,
) -> Vec<(Pattern<Asset>, Quantity)> {
    one([stage(Slot::Is(operation), machine().var()).into()])
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, Q::balance(role, asset))
}
/// The machines `operation` may run on, the preferred first, each with how
/// many ticks it takes there: law structure, which a start's relation gives
/// as `{processing-time}`.
pub fn alternatives(operation: Operation) -> [(Machine, u64); 2] {
    match operation {
        Operation::OneA | Operation::TwoA => [(Machine::One, 2), (Machine::Two, 3)],
        Operation::OneB | Operation::TwoB => [(Machine::Two, 1), (Machine::Three, 3)],
        Operation::OneC | Operation::TwoC => [(Machine::Three, 1), (Machine::One, 2)],
    }
}
pub(super) fn build(stress: bool) -> World {
    let horizon = if stress { 24 } else { 16 };
    let mut schedule = basket([
        (Asset::Active, 1),
        (Asset::Labor, if stress { 1 } else { 2 }),
        (Asset::RepairKit, 1),
        (Asset::TimeRemaining, horizon),
        (Asset::FaultAt, if stress { 4 } else { 3 }),
        (Asset::FaultPending, 1),
        (Asset::RepairDuration, if stress { 3 } else { 2 }),
    ]);
    for operation in OPERATIONS {
        schedule.insert(Asset::NotStarted(operation), Quantity::new(1));
    }
    let mut builder = EconomyBuilder::new().account(AccountId::Success, Account::from(schedule));
    for (job, ready, arrival) in [
        (Job::One, Operation::OneA, 0),
        (Job::Two, Operation::TwoA, if stress { 3 } else { 1 }),
    ] {
        builder = builder.account(
            AccountId::Job(job),
            Account::from(basket([
                (Asset::Ready(ready), 1),
                (Asset::Arrival, arrival),
            ])),
        );
    }
    for machine in MACHINES {
        let held = basket([
            (Asset::Idle, 1),
            (Asset::Online, 1),
            (Asset::QueueFree, 1),
            (
                Asset::Configured(if machine == Machine::Two {
                    Job::Two
                } else {
                    Job::One
                }),
                1,
            ),
        ]);
        builder = builder.account(AccountId::Machine(machine), Account::from(held));
    }
    for operation in OPERATIONS {
        let job = operation_spec(operation).0;
        builder = builder.rate_at(
            RateIdPattern::Queue {
                operation: Slot::Is(operation),
                machine: machine().var(),
            }
            .into(),
            operation_law(operation)
                .condition(
                    path!("job-has-arrived"),
                    clock(),
                    C::GreaterThanOrEqual,
                    Q::balance(Role::PrimaryJob, Asset::Arrival),
                )
                .consume(Role::PrimaryJob, basket([(Asset::Ready(operation), 1)]))
                .produce_at(
                    Role::PrimaryJob,
                    on_machine(AssetPattern::Queued, operation),
                )
                .consume(Role::Machine, basket([(Asset::QueueFree, 1)]))
                .produce(Role::Machine, basket([(Asset::QueueTicket(operation), 1)])),
        );
        builder = builder.rate_at(
            RateIdPattern::CancelQueue {
                operation: Slot::Is(operation),
                machine: machine().var(),
            }
            .into(),
            operation_law(operation)
                .consume_at(
                    Role::PrimaryJob,
                    on_machine(AssetPattern::Queued, operation),
                )
                .produce(Role::PrimaryJob, basket([(Asset::Ready(operation), 1)]))
                .consume(Role::Machine, basket([(Asset::QueueTicket(operation), 1)]))
                .produce(Role::Machine, basket([(Asset::QueueFree, 1)])),
        );
        // A start is due when the machine has run the operation for as long
        // as it takes there, and a tick more to switch a setup for the other
        // job.
        let processing = Param::<u64>::new(path!("processing-time"));
        for configured in [Job::One, Job::Two] {
            let setup = u64::from(configured != job);
            builder = builder.rate_at(
                RateIdPattern::Start {
                    operation: Slot::Is(operation),
                    machine: machine().var(),
                    configured: Slot::Is(configured),
                }
                .into(),
                operation_law(operation)
                    .relation(Relation::each_giving(
                        &machine(),
                        [processing.name().clone()],
                        alternatives(operation).map(|(runs, ticks)| (runs, [ticks])),
                    ))
                    .preserve(Role::Machine, basket([(Asset::Online, 1)]))
                    .consume_at(
                        Role::PrimaryJob,
                        on_machine(AssetPattern::Queued, operation),
                    )
                    .produce_at(
                        Role::PrimaryJob,
                        on_machine(AssetPattern::Running, operation),
                    )
                    .produce_computed(
                        Role::PrimaryJob,
                        Asset::Due,
                        clock() + processing.quantity() + Q::constant(setup),
                    )
                    .consume(
                        Role::Machine,
                        basket([
                            (Asset::Idle, 1),
                            (Asset::QueueTicket(operation), 1),
                            (Asset::Configured(configured), 1),
                        ]),
                    )
                    .produce(
                        Role::Machine,
                        basket([
                            (Asset::Busy, 1),
                            (Asset::QueueFree, 1),
                            (Asset::Configured(job), 1),
                        ]),
                    )
                    .consume(
                        Role::Schedule,
                        basket([(Asset::Labor, 1), (Asset::NotStarted(operation), 1)]),
                    )
                    .produce(
                        Role::Schedule,
                        basket([(Asset::BusyLabor, 1), (Asset::Started(operation), 1)]),
                    )
                    .produce_at(
                        Role::Schedule,
                        on_machine(AssetPattern::ExecutedOn, operation),
                    )
                    .produce_computed(Role::Schedule, Asset::StartTime(operation), clock()),
            );
        }
        let complete = clear(operation_law(operation), Role::PrimaryJob, Asset::Due)
            .preserve(Role::Machine, basket([(Asset::Online, 1)]))
            .condition(
                path!("execution-has-finished"),
                clock(),
                C::GreaterThanOrEqual,
                Q::balance(Role::PrimaryJob, Asset::Due),
            )
            .consume_at(
                Role::PrimaryJob,
                on_machine(AssetPattern::Running, operation),
            )
            .consume(Role::Machine, basket([(Asset::Busy, 1)]))
            .produce(Role::Machine, basket([(Asset::Idle, 1)]))
            .consume(Role::Schedule, basket([(Asset::BusyLabor, 1)]))
            .produce(Role::Schedule, basket([(Asset::Labor, 1)]))
            .produce_computed(Role::Schedule, Asset::EndTime(operation), clock());
        let complete = match successor(operation) {
            Some(next) => complete.produce(Role::PrimaryJob, basket([(Asset::Ready(next), 1)])),
            None => complete
                .produce(Role::PrimaryJob, basket([(Asset::Done, 1)]))
                .produce_computed(Role::PrimaryJob, Asset::CompletionTime, clock()),
        };
        builder = builder.rate_at(
            RateIdPattern::Complete {
                operation: Slot::Is(operation),
                machine: machine().var(),
            }
            .into(),
            complete,
        );
        builder = builder.rate_at(
            RateIdPattern::Resume {
                operation: Slot::Is(operation),
                machine: machine().var(),
            }
            .into(),
            clear(
                operation_law(operation),
                Role::PrimaryJob,
                Asset::WorkRemaining,
            )
            .preserve(
                Role::Machine,
                basket([(Asset::Online, 1), (Asset::Busy, 1)]),
            )
            .consume_at(
                Role::PrimaryJob,
                on_machine(AssetPattern::Paused, operation),
            )
            .produce_at(
                Role::PrimaryJob,
                on_machine(AssetPattern::Running, operation),
            )
            .produce_computed(
                Role::PrimaryJob,
                Asset::Due,
                clock() + Q::balance(Role::PrimaryJob, Asset::WorkRemaining),
            )
            .consume(Role::Schedule, basket([(Asset::Labor, 1)]))
            .produce(Role::Schedule, basket([(Asset::BusyLabor, 1)])),
        );
    }
    let fault = || {
        machine_law(Machine::Two)
            .condition(
                path!("disruption-is-due"),
                clock(),
                C::Equal,
                Q::balance(Role::Schedule, Asset::FaultAt),
            )
            .consume(Role::Schedule, basket([(Asset::FaultPending, 1)]))
            .produce(Role::Schedule, basket([(Asset::FaultOccurred, 1)]))
            .consume(Role::Machine, basket([(Asset::Online, 1)]))
            .produce(Role::Machine, basket([(Asset::Broken, 1)]))
    };
    builder = builder.rate_at(
        RateIdPattern::Fault {
            operation: Slot::Of(OptionPattern::None),
        }
        .into(),
        fault().preserve(Role::Machine, basket([(Asset::Idle, 1)])),
    );
    for operation in OPERATIONS {
        if alternatives(operation)
            .iter()
            .any(|(m, _)| *m == Machine::Two)
        {
            builder = builder.rate(
                RateId::Fault {
                    operation: Some(operation),
                },
                clear(
                    of_operation(fault(), operation),
                    Role::PrimaryJob,
                    Asset::Due,
                )
                .condition(
                    path!("unfinished-work-remains"),
                    Q::balance(Role::PrimaryJob, Asset::Due),
                    C::GreaterThanOrEqual,
                    clock() + Q::constant(1),
                )
                .consume(
                    Role::PrimaryJob,
                    basket([(Asset::Running(operation, Machine::Two), 1)]),
                )
                .produce(
                    Role::PrimaryJob,
                    basket([(Asset::Paused(operation, Machine::Two), 1)]),
                )
                .produce_computed(
                    Role::PrimaryJob,
                    Asset::WorkRemaining,
                    Q::balance(Role::PrimaryJob, Asset::Due) - clock(),
                )
                .consume(Role::Schedule, basket([(Asset::BusyLabor, 1)]))
                .produce(
                    Role::Schedule,
                    basket([(Asset::Labor, 1), (Asset::Interrupted(operation), 1)]),
                ),
            );
        }
    }
    builder = builder.rate(
        RateId::Repair,
        machine_law(Machine::Two)
            .consume(Role::Machine, basket([(Asset::Broken, 1)]))
            .produce(Role::Machine, basket([(Asset::Repairing, 1)]))
            .produce_computed(
                Role::Machine,
                Asset::Due,
                clock() + Q::balance(Role::Schedule, Asset::RepairDuration),
            )
            .consume(
                Role::Schedule,
                basket([(Asset::RepairKit, 1), (Asset::Labor, 1)]),
            )
            .produce(
                Role::Schedule,
                basket([(Asset::UsedRepairKit, 1), (Asset::BusyLabor, 1)]),
            ),
    );
    builder = builder.rate(
        RateId::CompleteRepair,
        clear(machine_law(Machine::Two), Role::Machine, Asset::Due)
            .condition(
                path!("repair-duration-elapsed"),
                clock(),
                C::GreaterThanOrEqual,
                Q::balance(Role::Machine, Asset::Due),
            )
            .consume(Role::Machine, basket([(Asset::Repairing, 1)]))
            .produce(Role::Machine, basket([(Asset::Online, 1)]))
            .consume(Role::Schedule, basket([(Asset::BusyLabor, 1)]))
            .produce(Role::Schedule, basket([(Asset::Labor, 1)])),
    );
    let to = Param::<u8>::new(path!("to"));
    builder = builder.rate_at(
        RateIdPattern::Advance { to: to.var() }.into(),
        law()
            .variable(to.range(1, horizon as u8))
            .condition(
                path!("clock-moves-forward"),
                clock() + Q::constant(1),
                C::LessThanOrEqual,
                to.quantity(),
            )
            .condition(
                path!("cannot-skip-a-scheduled-disruption"),
                to.quantity(),
                C::LessThanOrEqual,
                Q::balance(Role::Schedule, Asset::FaultAt)
                    + Q::balance(Role::Schedule, Asset::FaultOccurred) * Q::constant(horizon),
            )
            .consume_computed(
                Role::Schedule,
                Asset::TimeRemaining,
                to.quantity() - clock(),
            )
            .produce_computed(Role::Schedule, Asset::Now, to.quantity() - clock()),
    );
    builder = builder.rate(
        RateId::Close,
        planned()
            .account(Role::PrimaryJob, Pattern::literal(AccountId::Job(Job::One)))
            .account(
                Role::SecondaryJob,
                Pattern::literal(AccountId::Job(Job::Two)),
            )
            .preserve(Role::PrimaryJob, basket([(Asset::Done, 1)]))
            .preserve(Role::SecondaryJob, basket([(Asset::Done, 1)]))
            .consume(Role::Schedule, basket([(Asset::Active, 1)]))
            .produce(Role::Schedule, basket([(Asset::Solved, 1)])),
    );
    let mut jobs = LinearInvariant::new(path!("job-phase-accounting")).weight(Asset::Done, 1);
    let mut queues =
        LinearInvariant::new(path!("three-machine-queues")).weight(Asset::QueueFree, 1);
    let mut starts = LinearInvariant::new(path!("each-operation-starts-once"));
    for operation in OPERATIONS {
        jobs = jobs.weight(Asset::Ready(operation), 1);
        queues = queues.weight(Asset::QueueTicket(operation), 1);
        starts = starts
            .weight(Asset::NotStarted(operation), 1)
            .weight(Asset::Started(operation), 1);
        for (machine, _) in alternatives(operation) {
            jobs = jobs
                .weight(Asset::Queued(operation, machine), 1)
                .weight(Asset::Running(operation, machine), 1)
                .weight(Asset::Paused(operation, machine), 1);
        }
    }
    builder
        .invariant(jobs)
        .invariant(queues)
        .invariant(starts)
        .invariant(
            LinearInvariant::new(path!("machine-occupancy"))
                .weight(Asset::Idle, 1)
                .weight(Asset::Busy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("machine-condition"))
                .weight(Asset::Online, 1)
                .weight(Asset::Broken, 1)
                .weight(Asset::Repairing, 1),
        )
        .invariant(
            LinearInvariant::new(path!("setup-configuration"))
                .weight(Asset::Configured(Job::One), 1)
                .weight(Asset::Configured(Job::Two), 1),
        )
        .invariant(
            LinearInvariant::new(path!("shared-labor"))
                .weight(Asset::Labor, 1)
                .weight(Asset::BusyLabor, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-repair-supply"))
                .weight(Asset::RepairKit, 1)
                .weight(Asset::UsedRepairKit, 1),
        )
        .invariant(
            LinearInvariant::new(path!("scheduled-disruption"))
                .weight(Asset::FaultPending, 1)
                .weight(Asset::FaultOccurred, 1),
        )
        .invariant(
            LinearInvariant::new(path!("bounded-workday"))
                .weight(Asset::Now, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new(path!("execution-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("job shop execution is closed")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(world: &mut World, rate: RateId) {
        world.apply(action(world, rate)).unwrap();
    }
    #[test]
    fn reservations_are_not_execution_and_shared_labor_limits_overlap() {
        let mut world = initial_showcase();
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Queue {
                operation: Operation::TwoA,
                machine: Machine::Two
            }
        )));
        apply(
            &mut world,
            RateId::Queue {
                operation: Operation::OneA,
                machine: Machine::One,
            },
        );
        assert!(
            world
                .balance(&AccountId::Job(Job::One), &Asset::Ready(Operation::OneB))
                .is_zero()
        );
        apply(
            &mut world,
            RateId::Start {
                operation: Operation::OneA,
                machine: Machine::One,
                configured: Job::One,
            },
        );
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Complete {
                operation: Operation::OneA,
                machine: Machine::One
            }
        )));
        apply(&mut world, RateId::Advance { to: 1 });
        apply(
            &mut world,
            RateId::Queue {
                operation: Operation::TwoA,
                machine: Machine::Two,
            },
        );
        apply(
            &mut world,
            RateId::Start {
                operation: Operation::TwoA,
                machine: Machine::Two,
                configured: Job::Two,
            },
        );
        assert_eq!(
            world
                .balance(&AccountId::Success, &Asset::BusyLabor)
                .to_u64(),
            2
        );
        assert!(world.balance(&AccountId::Success, &Asset::Labor).is_zero());
    }
    #[test]
    fn disruption_pauses_unfinished_work_and_repair_preserves_progress() {
        let mut world = initial_showcase();
        apply(&mut world, RateId::Advance { to: 1 });
        apply(
            &mut world,
            RateId::Queue {
                operation: Operation::TwoA,
                machine: Machine::Two,
            },
        );
        apply(
            &mut world,
            RateId::Start {
                operation: Operation::TwoA,
                machine: Machine::Two,
                configured: Job::Two,
            },
        );
        assert!(!world.is_applicable(&action(&world, RateId::Advance { to: 4 })));
        apply(&mut world, RateId::Advance { to: 3 });
        apply(
            &mut world,
            RateId::Fault {
                operation: Some(Operation::TwoA),
            },
        );
        assert_eq!(
            world
                .balance(&AccountId::Job(Job::Two), &Asset::WorkRemaining)
                .to_u64(),
            1
        );
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Resume {
                operation: Operation::TwoA,
                machine: Machine::Two
            }
        )));
        apply(&mut world, RateId::Repair);
        assert!(!world.is_applicable(&action(&world, RateId::CompleteRepair)));
        apply(&mut world, RateId::Advance { to: 5 });
        apply(&mut world, RateId::CompleteRepair);
        apply(
            &mut world,
            RateId::Resume {
                operation: Operation::TwoA,
                machine: Machine::Two,
            },
        );
        apply(&mut world, RateId::Advance { to: 6 });
        apply(
            &mut world,
            RateId::Complete {
                operation: Operation::TwoA,
                machine: Machine::Two,
            },
        );
        assert_eq!(
            world
                .balance(&AccountId::Success, &Asset::EndTime(Operation::TwoA))
                .to_u64(),
            6
        );
        assert_eq!(
            world
                .balance(&AccountId::Success, &Asset::UsedRepairKit)
                .to_u64(),
            1
        );
    }
}
