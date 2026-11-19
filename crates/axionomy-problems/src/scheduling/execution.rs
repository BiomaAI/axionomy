//! Executed job shop with queues, setup, shared labor and recoverable disruption.
//! Each operation's laws serve both machines it may run on, `{machine}`, and
//! the clock advances to any tick, `{to}`; every role is fixed to the account
//! its identifier names. When each job arrives is its operations' queue laws'
//! rows; when Machine Two fails and how long a repair takes are the shop's
//! constants.
use super::*;

pub const OPERATIONS: [Operation; 6] = [
    Operation::OneA,
    Operation::OneB,
    Operation::OneC,
    Operation::TwoA,
    Operation::TwoB,
    Operation::TwoC,
];
pub const MACHINES: [Machine; 3] = [Machine::One, Machine::Two, Machine::Three];
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
