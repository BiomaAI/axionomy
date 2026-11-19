//! Coupled power, equipment, and persistent injury in the larger mazes.

#[cfg(test)]
mod tests {
    use crate::maze::*;
    fn move_to(world: &mut World, to: Node) {
        let from = position(world).unwrap();
        let move_id = world
            .rate_ids()
            .filter_map(|rate| rate.as_literal())
            .find(|id| matches!(id, RateId::Move { from: f, to: t, .. } if *f == from && *t == to))
            .copied()
            .unwrap();
        world.apply(action(move_id)).unwrap();
    }
    #[test]
    fn open_door_requires_shared_power_and_diversion_disables_the_other_crossing() {
        let mut world = initial_showcase();
        for at in [Node::Atrium, Node::Library, Node::KeyRoom] {
            move_to(&mut world, at);
        }
        world
            .apply(action(RateId::TakeKey { at: Node::KeyRoom }))
            .unwrap();
        move_to(&mut world, Node::Library);
        let mut powered = world.clone();
        powered
            .apply(action(RateId::DivertPower {
                at: Node::Library,
                to_vault: true,
            }))
            .unwrap();
        for state in [&mut world, &mut powered] {
            move_to(state, Node::Gallery);
            move_to(state, Node::Gate);
            state
                .apply(action(RateId::UnlockDoor { at: Node::Gate }))
                .unwrap();
        }
        let crossing = action(RateId::Move {
            from: Node::Gate,
            to: Node::Vault,
            energy: 1,
            needs_open_door: true,
        });
        let before = world.state_key();
        assert!(world.apply(crossing.clone()).is_err());
        assert_eq!(world.state_key(), before);
        powered.apply(crossing).unwrap();
        assert!(
            powered
                .balance(&AccountId::World, &Asset::PowerTransit)
                .is_zero()
        );
    }
    #[test]
    fn protection_changes_elapsed_time_and_recovery_is_finite() {
        let initial = initial_showcase();
        let mut injured = initial.clone();
        move_to(&mut injured, Node::Detour);
        move_to(&mut injured, Node::Exit);
        injured.apply(action(RateId::Finish)).unwrap();
        assert_eq!(spent_time(&injured), 5);
        assert_eq!(
            injured.balance(&AccountId::Agent, &Asset::Injury).to_u64(),
            2
        );

        let mut protected = initial.clone();
        protected
            .apply(action(RateId::TakeProtection { at: Node::Start }))
            .unwrap();
        move_to(&mut protected, Node::Detour);
        protected
            .apply(action(RateId::ProtectedMove {
                from: Node::Detour,
                to: Node::Exit,
                energy: 7,
            }))
            .unwrap();
        protected.apply(action(RateId::Finish)).unwrap();
        assert_eq!(spent_time(&protected), 4);
        assert_eq!(spent_energy(&injured), spent_energy(&protected));
        assert_eq!(
            protected
                .balance(&AccountId::Agent, &Asset::UsedShield)
                .to_u64(),
            2
        );
        assert!(
            protected
                .balance(&AccountId::Agent, &Asset::Injury)
                .is_zero()
        );

        let mut recovery = initial;
        for at in [Node::Tunnel, Node::Workshop, Node::Garden] {
            move_to(&mut recovery, at);
        }
        assert_eq!(
            recovery.balance(&AccountId::Agent, &Asset::Injury).to_u64(),
            1
        );
        recovery
            .apply(action(RateId::Treat { at: Node::Garden }))
            .unwrap();
        assert!(
            recovery
                .balance(&AccountId::Agent, &Asset::Injury)
                .is_zero()
        );
        assert_eq!(
            recovery.balance(&AccountId::Agent, &Asset::Health).to_u64(),
            3
        );
        assert_eq!(
            recovery
                .balance(&AccountId::World, &Asset::UsedMedicine)
                .to_u64(),
            1
        );
        let before = recovery.state_key();
        assert!(
            recovery
                .apply(action(RateId::Treat { at: Node::Garden }))
                .is_err()
        );
        assert_eq!(recovery.state_key(), before);
    }
}
