//! Coupled power, equipment, and persistent injury in the larger mazes.
use super::*;
use axionomy::path;
use axionomy::{QuantityComparison as C, QuantityExpression as Q};

fn risk(from: Node, to: Node) -> u64 {
    match (from, to) {
        (Node::Detour, Node::Exit) | (Node::Foundry, Node::Tower) | (Node::Ridge, Node::Ruins) => 2,
        (Node::Tunnel, Node::Bridge | Node::Workshop) => 1,
        _ => 0,
    }
}

pub(super) fn build(source: World) -> World {
    let mut builder = EconomyBuilder::new();
    for (id, account) in source.accounts() {
        let mut account = account.clone();
        account
            .deposit(&match id {
                AccountId::Agent => basket([(Asset::Health, 3)]),
                AccountId::World => basket([
                    (Asset::PowerTransit, 1),
                    (Asset::Shield, 2),
                    (Asset::Medicine, 2),
                ]),
            })
            .unwrap();
        builder = builder.account(*id, account);
    }
    for id in source.rate_ids() {
        let mut rate = source.rate(id).unwrap().clone();
        // Injuries persist and slow all ordinary work, including the last step.
        rate = rate
            .consume_computed(
                Role::Actor,
                Asset::Time,
                Q::balance(Role::Actor, Asset::Injury),
            )
            .produce_computed(
                Role::Actor,
                Asset::SpentTime,
                Q::balance(Role::Actor, Asset::Injury),
            );
        if let RateId::Move {
            from,
            to,
            energy,
            needs_open_door,
        } = *id
        {
            if needs_open_door {
                rate = rate.preserve(Role::Environment, basket([(Asset::PowerVault, 1)]));
            }
            if (from, to) == (Node::Canal, Node::Bridge) {
                rate = rate.preserve(Role::Environment, basket([(Asset::PowerTransit, 1)]));
            }
            let damage = risk(from, to);
            if damage > 0 {
                builder = builder.rate(
                    RateId::ProtectedMove { from, to, energy },
                    rate.clone()
                        .consume(Role::Actor, basket([(Asset::Shield, damage)]))
                        .produce(Role::Actor, basket([(Asset::UsedShield, damage)])),
                );
                rate = rate
                    .condition(
                        path!("retain-at-least-one-health"),
                        Q::balance(Role::Actor, Asset::Health),
                        C::GreaterThanOrEqual,
                        Q::constant(damage + 1),
                    )
                    .consume(Role::Actor, basket([(Asset::Health, damage)]))
                    .produce(Role::Actor, basket([(Asset::Injury, damage)]));
            }
        }
        builder = builder.rate(*id, rate);
    }
    let law = |at, time| {
        Rate::new()
            .preserve(
                Role::Actor,
                basket([(Asset::At(at), 1), (Asset::Active, 1)]),
            )
            .consume(Role::Actor, basket([(Asset::Time, time)]))
            .produce(Role::Actor, basket([(Asset::SpentTime, time)]))
            .distinct(Role::Actor, Role::Environment)
    };
    for at in [Node::Library, Node::Workshop] {
        for to_vault in [false, true] {
            let (from, to) = if to_vault {
                (Asset::PowerTransit, Asset::PowerVault)
            } else {
                (Asset::PowerVault, Asset::PowerTransit)
            };
            builder = builder.rate(
                RateId::DivertPower { at, to_vault },
                law(at, 1)
                    .consume(Role::Environment, basket([(from, 1)]))
                    .produce(Role::Environment, basket([(to, 1)]))
                    .consume(Role::Actor, basket([(Asset::Energy, 1)]))
                    .produce(Role::Actor, basket([(Asset::SpentEnergy, 1)])),
            );
        }
    }
    for at in [Node::Start, Node::Workshop] {
        builder = builder.rate(
            RateId::TakeProtection { at },
            law(at, 1)
                .consume(Role::Environment, basket([(Asset::Shield, 2)]))
                .produce(Role::Actor, basket([(Asset::Shield, 2)])),
        );
    }
    builder = builder.rate(
        RateId::Treat { at: Node::Garden },
        law(Node::Garden, 2)
            .condition(
                path!("an-injury-needs-treatment"),
                Q::balance(Role::Actor, Asset::Injury),
                C::GreaterThanOrEqual,
                Q::constant(1),
            )
            .consume(Role::Environment, basket([(Asset::Medicine, 1)]))
            .produce(Role::Environment, basket([(Asset::UsedMedicine, 1)]))
            .consume_computed(
                Role::Actor,
                Asset::Injury,
                Q::balance(Role::Actor, Asset::Injury),
            )
            .produce_computed(
                Role::Actor,
                Asset::Health,
                Q::balance(Role::Actor, Asset::Injury),
            ),
    );
    for invariant in source.invariants() {
        builder = builder.invariant(invariant.clone());
    }
    builder
        .invariant(
            LinearInvariant::new(path!("one-shared-power-circuit"))
                .weight(Asset::PowerTransit, 1)
                .weight(Asset::PowerVault, 1),
        )
        .invariant(
            LinearInvariant::new(path!("health-and-injury"))
                .weight(Asset::Health, 1)
                .weight(Asset::Injury, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-protective-equipment"))
                .weight(Asset::Shield, 1)
                .weight(Asset::UsedShield, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-medicine"))
                .weight(Asset::Medicine, 1)
                .weight(Asset::UsedMedicine, 1),
        )
        .build()
        .expect("maze dynamics preserve the source model")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn move_to(world: &mut World, to: Node) {
        let from = position(world).unwrap();
        let move_id = world
            .rate_ids()
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
