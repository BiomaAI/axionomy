//! Executed job shop with queues, setup, shared labor and recoverable disruption.
use super::*;
use axionomy::{QuantityComparison as C, QuantityExpression as Q};

pub const OPERATIONS: [Operation; 6] = [
    Operation::OneA,
    Operation::OneB,
    Operation::OneC,
    Operation::TwoA,
    Operation::TwoB,
    Operation::TwoC,
];
pub const MACHINES: [Machine; 3] = [Machine::One, Machine::Two, Machine::Three];
pub fn enabled(world: &World) -> bool {
    world.rate(&RateId::Close).is_some()
}
pub fn now(world: &World) -> u64 {
    world.balance(&AccountId::Success, &Asset::Now).to_u64()
}
fn clock() -> Q<Role, Asset> {
    Q::balance(Role::Schedule, Asset::Now)
}
fn law() -> Rate<Role, Asset> {
    Rate::new()
        .preserve(Role::Schedule, basket([(Asset::Active, 1)]))
        .condition("one scheduling event", Q::units(), C::Equal, Q::constant(1))
}
fn machine_law(machine: Machine) -> Rate<Role, Asset> {
    law()
        .preserve(
            Role::Machine,
            basket([(Asset::MachineIdentity(machine), 1)]),
        )
        .distinct(Role::Machine, Role::Schedule)
}
fn operation_law(operation: Operation, machine: Machine) -> Rate<Role, Asset> {
    machine_law(machine)
        .preserve(
            Role::PrimaryJob,
            basket([(Asset::JobIdentity(operation_spec(operation).0), 1)]),
        )
        .distinct(Role::PrimaryJob, Role::Machine)
        .distinct(Role::PrimaryJob, Role::Schedule)
}
fn clear(rate: Rate<Role, Asset>, role: Role, asset: Asset) -> Rate<Role, Asset> {
    rate.consume_computed(role, asset, Q::balance(role, asset))
}
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
                (Asset::JobIdentity(job), 1),
                (Asset::Ready(ready), 1),
                (Asset::Arrival, arrival),
            ])),
        );
    }
    for machine in MACHINES {
        builder = builder.account(
            AccountId::Machine(machine),
            Account::from(basket([
                (Asset::MachineIdentity(machine), 1),
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
            ])),
        );
    }
    for operation in OPERATIONS {
        let job = operation_spec(operation).0;
        for (machine, duration) in alternatives(operation) {
            builder = builder.rate(
                RateId::Queue { operation, machine },
                operation_law(operation, machine)
                    .condition(
                        "job has arrived",
                        clock(),
                        C::GreaterThanOrEqual,
                        Q::balance(Role::PrimaryJob, Asset::Arrival),
                    )
                    .consume(Role::PrimaryJob, basket([(Asset::Ready(operation), 1)]))
                    .produce(
                        Role::PrimaryJob,
                        basket([(Asset::Queued(operation, machine), 1)]),
                    )
                    .consume(Role::Machine, basket([(Asset::QueueFree, 1)]))
                    .produce(Role::Machine, basket([(Asset::QueueTicket(operation), 1)])),
            );
            builder = builder.rate(
                RateId::CancelQueue { operation, machine },
                operation_law(operation, machine)
                    .consume(
                        Role::PrimaryJob,
                        basket([(Asset::Queued(operation, machine), 1)]),
                    )
                    .produce(Role::PrimaryJob, basket([(Asset::Ready(operation), 1)]))
                    .consume(Role::Machine, basket([(Asset::QueueTicket(operation), 1)]))
                    .produce(Role::Machine, basket([(Asset::QueueFree, 1)])),
            );
            for configured in [Job::One, Job::Two] {
                let elapsed = duration + u64::from(configured != job);
                builder = builder.rate(
                    RateId::Start {
                        operation,
                        machine,
                        configured,
                    },
                    operation_law(operation, machine)
                        .preserve(Role::Machine, basket([(Asset::Online, 1)]))
                        .consume(
                            Role::PrimaryJob,
                            basket([(Asset::Queued(operation, machine), 1)]),
                        )
                        .produce(
                            Role::PrimaryJob,
                            basket([(Asset::Running(operation, machine), 1)]),
                        )
                        .produce_computed(
                            Role::PrimaryJob,
                            Asset::Due,
                            clock() + Q::constant(elapsed),
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
                            basket([
                                (Asset::BusyLabor, 1),
                                (Asset::Started(operation), 1),
                                (Asset::ExecutedOn(operation, machine), 1),
                            ]),
                        )
                        .produce_computed(Role::Schedule, Asset::StartTime(operation), clock()),
                );
            }
            let mut complete = clear(
                operation_law(operation, machine),
                Role::PrimaryJob,
                Asset::Due,
            )
            .preserve(Role::Machine, basket([(Asset::Online, 1)]))
            .condition(
                "execution has finished",
                clock(),
                C::GreaterThanOrEqual,
                Q::balance(Role::PrimaryJob, Asset::Due),
            )
            .consume(
                Role::PrimaryJob,
                basket([(Asset::Running(operation, machine), 1)]),
            )
            .consume(Role::Machine, basket([(Asset::Busy, 1)]))
            .produce(Role::Machine, basket([(Asset::Idle, 1)]))
            .consume(Role::Schedule, basket([(Asset::BusyLabor, 1)]))
            .produce(Role::Schedule, basket([(Asset::Labor, 1)]))
            .produce_computed(Role::Schedule, Asset::EndTime(operation), clock());
            if let Some(next) = successor(operation) {
                complete = complete.produce(Role::PrimaryJob, basket([(Asset::Ready(next), 1)]));
            } else {
                complete = complete
                    .produce(Role::PrimaryJob, basket([(Asset::Done, 1)]))
                    .produce_computed(Role::PrimaryJob, Asset::CompletionTime, clock());
            }
            builder = builder.rate(RateId::Complete { operation, machine }, complete);
            builder = builder.rate(
                RateId::Resume { operation, machine },
                clear(
                    operation_law(operation, machine),
                    Role::PrimaryJob,
                    Asset::WorkRemaining,
                )
                .preserve(
                    Role::Machine,
                    basket([(Asset::Online, 1), (Asset::Busy, 1)]),
                )
                .consume(
                    Role::PrimaryJob,
                    basket([(Asset::Paused(operation, machine), 1)]),
                )
                .produce(
                    Role::PrimaryJob,
                    basket([(Asset::Running(operation, machine), 1)]),
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
    }
    let fault = || {
        machine_law(Machine::Two)
            .condition(
                "disruption is due",
                clock(),
                C::Equal,
                Q::balance(Role::Schedule, Asset::FaultAt),
            )
            .consume(Role::Schedule, basket([(Asset::FaultPending, 1)]))
            .produce(Role::Schedule, basket([(Asset::FaultOccurred, 1)]))
            .consume(Role::Machine, basket([(Asset::Online, 1)]))
            .produce(Role::Machine, basket([(Asset::Broken, 1)]))
    };
    builder = builder.rate(
        RateId::Fault { operation: None },
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
                clear(fault(), Role::PrimaryJob, Asset::Due)
                    .preserve(
                        Role::PrimaryJob,
                        basket([(Asset::JobIdentity(operation_spec(operation).0), 1)]),
                    )
                    .condition(
                        "unfinished work remains",
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
                    )
                    .distinct(Role::PrimaryJob, Role::Machine)
                    .distinct(Role::PrimaryJob, Role::Schedule),
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
                "repair duration elapsed",
                clock(),
                C::GreaterThanOrEqual,
                Q::balance(Role::Machine, Asset::Due),
            )
            .consume(Role::Machine, basket([(Asset::Repairing, 1)]))
            .produce(Role::Machine, basket([(Asset::Online, 1)]))
            .consume(Role::Schedule, basket([(Asset::BusyLabor, 1)]))
            .produce(Role::Schedule, basket([(Asset::Labor, 1)])),
    );
    for to in 1..=horizon as u8 {
        builder = builder.rate(
            RateId::Advance { to },
            law()
                .condition(
                    "clock moves forward",
                    clock() + Q::constant(1),
                    C::LessThanOrEqual,
                    Q::constant(u64::from(to)),
                )
                .condition(
                    "cannot skip a scheduled disruption",
                    Q::constant(u64::from(to)),
                    C::LessThanOrEqual,
                    Q::balance(Role::Schedule, Asset::FaultAt)
                        + Q::balance(Role::Schedule, Asset::FaultOccurred) * Q::constant(horizon),
                )
                .consume_computed(
                    Role::Schedule,
                    Asset::TimeRemaining,
                    Q::constant(u64::from(to)) - clock(),
                )
                .produce_computed(
                    Role::Schedule,
                    Asset::Now,
                    Q::constant(u64::from(to)) - clock(),
                ),
        );
    }
    builder = builder.rate(
        RateId::Close,
        Rate::new()
            .preserve(
                Role::PrimaryJob,
                basket([(Asset::JobIdentity(Job::One), 1), (Asset::Done, 1)]),
            )
            .preserve(
                Role::SecondaryJob,
                basket([(Asset::JobIdentity(Job::Two), 1), (Asset::Done, 1)]),
            )
            .consume(Role::Schedule, basket([(Asset::Active, 1)]))
            .produce(Role::Schedule, basket([(Asset::Solved, 1)]))
            .distinct(Role::PrimaryJob, Role::SecondaryJob)
            .distinct(Role::PrimaryJob, Role::Schedule)
            .distinct(Role::SecondaryJob, Role::Schedule),
    );
    let mut jobs = LinearInvariant::new("job phase accounting").weight(Asset::Done, 1);
    let mut queues = LinearInvariant::new("three machine queues").weight(Asset::QueueFree, 1);
    let mut starts = LinearInvariant::new("each operation starts once");
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
            LinearInvariant::new("machine occupancy")
                .weight(Asset::Idle, 1)
                .weight(Asset::Busy, 1),
        )
        .invariant(
            LinearInvariant::new("machine condition")
                .weight(Asset::Online, 1)
                .weight(Asset::Broken, 1)
                .weight(Asset::Repairing, 1),
        )
        .invariant(
            LinearInvariant::new("setup configuration")
                .weight(Asset::Configured(Job::One), 1)
                .weight(Asset::Configured(Job::Two), 1),
        )
        .invariant(
            LinearInvariant::new("shared labor")
                .weight(Asset::Labor, 1)
                .weight(Asset::BusyLabor, 1),
        )
        .invariant(
            LinearInvariant::new("finite repair supply")
                .weight(Asset::RepairKit, 1)
                .weight(Asset::UsedRepairKit, 1),
        )
        .invariant(
            LinearInvariant::new("scheduled disruption")
                .weight(Asset::FaultPending, 1)
                .weight(Asset::FaultOccurred, 1),
        )
        .invariant(
            LinearInvariant::new("bounded workday")
                .weight(Asset::Now, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new("execution lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("job shop execution is closed")
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut times = vec![];
    for job in [Job::One, Job::Two] {
        times.push(
            world
                .balance(&AccountId::Job(job), &Asset::Arrival)
                .to_u64(),
        );
        times.push(world.balance(&AccountId::Job(job), &Asset::Due).to_u64());
    }
    times.push(
        world
            .balance(&AccountId::Machine(Machine::Two), &Asset::Due)
            .to_u64(),
    );
    if world
        .balance(&AccountId::Success, &Asset::FaultPending)
        .to_u64()
        > 0
    {
        times.push(world.balance(&AccountId::Success, &Asset::FaultAt).to_u64());
    }
    let next = times.into_iter().filter(|time| *time > now(world)).min();
    let value = |account, asset| world.balance(&account, &asset).to_u64();
    if value(AccountId::Success, Asset::Active) == 0 {
        return vec![];
    }
    let labor = value(AccountId::Success, Asset::Labor);
    let mut ids = vec![RateId::Close];
    let mut queued_start = None;
    for operation in OPERATIONS {
        let job = AccountId::Job(operation_spec(operation).0);
        for (machine, _) in alternatives(operation) {
            let machine_account = AccountId::Machine(machine);
            if value(job, Asset::Ready(operation)) > 0
                && labor > 0
                && value(machine_account, Asset::Idle) > 0
                && value(machine_account, Asset::Online) > 0
            {
                ids.push(RateId::Queue { operation, machine });
            }
            if value(job, Asset::Queued(operation, machine)) > 0 {
                let configured = if value(machine_account, Asset::Configured(Job::One)) > 0 {
                    Job::One
                } else {
                    Job::Two
                };
                let start = RateId::Start {
                    operation,
                    machine,
                    configured,
                };
                if world.is_applicable(&action(start)) {
                    queued_start = Some(start);
                }
            }
            if value(job, Asset::Running(operation, machine)) > 0 {
                ids.push(RateId::Complete { operation, machine });
            }
            if value(job, Asset::Paused(operation, machine)) > 0 {
                ids.push(RateId::Resume { operation, machine });
            }
        }
    }
    if value(AccountId::Machine(Machine::Two), Asset::Broken) > 0 {
        ids.push(RateId::Repair);
    }
    if value(AccountId::Machine(Machine::Two), Asset::Repairing) > 0 {
        ids.push(RateId::CompleteRepair);
    }
    if value(AccountId::Success, Asset::FaultPending) > 0
        && now(world) == value(AccountId::Success, Asset::FaultAt)
    {
        ids.push(RateId::Fault { operation: None });
        ids.extend(
            OPERATIONS
                .into_iter()
                .filter(|op| {
                    value(
                        AccountId::Job(operation_spec(*op).0),
                        Asset::Running(*op, Machine::Two),
                    ) > 0
                })
                .map(|op| RateId::Fault {
                    operation: Some(op),
                }),
        );
    }
    if let Some(to) = next {
        ids.push(RateId::Advance { to: to as u8 });
    }
    // Reserving a queue consumes no time or work. For centralized optimization,
    // defer it until the operation can start, then start immediately. This
    // removes equivalent reservation interleavings without excluding schedules.
    if let Some(start) = queued_start {
        ids.push(start);
    }
    ids.sort();
    let applicable = world.applicable(ids.into_iter().map(action));
    // Same-time completions commute with dispatch; process them before new work.
    if let Some(event) = applicable.iter().find(|a| {
        matches!(
            a.rate(),
            RateId::Complete { .. } | RateId::CompleteRepair | RateId::Close
        )
    }) {
        return vec![event.clone()];
    }
    if let Some(fault) = applicable
        .iter()
        .find(|a| matches!(a.rate(), RateId::Fault { .. }))
    {
        return vec![fault.clone()];
    }
    if let Some(start) = queued_start {
        return vec![action(start)];
    }
    applicable
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(world: &mut World, rate: RateId) {
        world.apply(action(rate)).unwrap();
    }
    #[test]
    fn reservations_are_not_execution_and_shared_labor_limits_overlap() {
        let mut world = initial_showcase();
        assert!(!world.is_applicable(&action(RateId::Queue {
            operation: Operation::TwoA,
            machine: Machine::Two
        })));
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
        assert!(!world.is_applicable(&action(RateId::Complete {
            operation: Operation::OneA,
            machine: Machine::One
        })));
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
        assert!(!world.is_applicable(&action(RateId::Advance { to: 4 })));
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
        assert!(!world.is_applicable(&action(RateId::Resume {
            operation: Operation::TwoA,
            machine: Machine::Two
        })));
        apply(&mut world, RateId::Repair);
        assert!(!world.is_applicable(&action(RateId::CompleteRepair)));
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
    #[test]
    fn exact_execution_search_replays_and_single_crew_stress_changes_the_schedule() {
        let mut elapsed = Vec::new();
        for world in [initial_showcase(), initial_stress()] {
            let solution = solve_best_first(&world).expect("job shop is recoverable");
            let replay = world.replayed(solution.trace()).unwrap();
            assert!(replay.matches(&goal()));
            assert!(candidates(&replay).is_empty());
            elapsed.push(now(&replay));
        }
        assert!(elapsed[1] > elapsed[0]);
    }
}
