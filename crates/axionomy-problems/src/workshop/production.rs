//! A single workbench with competing contracts and persistent tooling costs.

#[cfg(test)]
mod tests {
    use crate::workshop::*;
    #[test]
    fn tool_depletion_requires_recovery_and_contracts_cannot_share_a_workbench() {
        let mut world = initial_showcase();
        for rate in [
            RateId::Accept(0),
            RateId::Prepare {
                order: 0,
                precise: false,
                upgraded: false,
            },
            RateId::Assemble(0),
            RateId::Deliver(0),
            RateId::Accept(1),
        ] {
            world.apply(action(rate)).unwrap();
        }
        let before = world.state_key();
        assert!(world.apply(action(RateId::Accept(2))).is_err());
        assert!(
            world
                .apply(action(RateId::Prepare {
                    order: 1,
                    precise: true,
                    upgraded: false
                }))
                .is_err()
        );
        assert_eq!(before, world.state_key());
        world.apply(action(RateId::Repair)).unwrap();
        world
            .apply(action(RateId::Prepare {
                order: 1,
                precise: true,
                upgraded: false,
            }))
            .unwrap();
        world.apply(action(RateId::Assemble(1))).unwrap();
        let rebound = action(RateId::Deliver(1)).bind(Role::Customer, AccountId::Customer(2));
        assert!(!world.is_applicable(&rebound));
        world.apply(action(RateId::Deliver(1))).unwrap();
        world.apply(action(RateId::Finish)).unwrap();
        assert!(world.matches(&goal()));
        assert_eq!(
            world.balance(&AccountId::Workshop, &Asset::Credit).to_u64(),
            29
        );
        assert_eq!(
            world
                .balance(&AccountId::Customer(1), &Asset::Table)
                .to_u64(),
            1
        );
        assert!(
            world
                .rate_ids()
                .filter_map(|rate| rate.as_literal())
                .all(|rate| !world.is_applicable(&action(*rate)))
        );
    }
    #[test]
    fn upgraded_chain_and_salvage_remain_mass_conserving() {
        let mut world = initial_showcase();
        for rate in [
            RateId::Accept(0),
            RateId::Prepare {
                order: 0,
                precise: false,
                upgraded: false,
            },
            RateId::Assemble(0),
            RateId::Deliver(0),
            RateId::Repair,
            RateId::Accept(2),
            RateId::Prepare {
                order: 2,
                precise: false,
                upgraded: false,
            },
            RateId::Assemble(2),
            RateId::Deliver(2),
            RateId::Recycle,
        ] {
            world.apply(action(rate)).unwrap();
        }
        assert_eq!(
            world.balance(&AccountId::Workshop, &Asset::Waste).to_u64(),
            1
        );
        assert!(
            world
                .balance(&AccountId::Workshop, &Asset::Offcut)
                .is_zero()
        );
        let mut upgraded = initial_showcase();
        for rate in [
            RateId::Upgrade,
            RateId::Accept(1),
            RateId::Prepare {
                order: 1,
                precise: false,
                upgraded: true,
            },
            RateId::Assemble(1),
            RateId::Deliver(1),
        ] {
            upgraded.apply(action(rate)).unwrap();
        }
        assert!(
            upgraded
                .balance(&AccountId::Workshop, &Asset::Offcut)
                .is_zero()
        );
        assert_eq!(
            upgraded
                .balance(&AccountId::Workshop, &Asset::Durability)
                .to_u64(),
            2
        );
    }
}
