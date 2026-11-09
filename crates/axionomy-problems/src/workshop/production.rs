//! A single workbench with competing contracts and persistent tooling costs.
use super::*;
use axionomy::{QuantityComparison as C, QuantityExpression as Q};

fn law(time: u64, labor: u64) -> Rate<Ontology> {
    Rate::new()
        .preserve(Role::Shop, basket([(Asset::Active, 1)]))
        .consume(
            Role::Shop,
            basket([(Asset::Time, time), (Asset::Labor, labor)]),
        )
        .produce(
            Role::Shop,
            basket([(Asset::SpentTime, time), (Asset::SpentLabor, labor)]),
        )
}
fn tooling(rate: Rate<Ontology>, wear: u64) -> Rate<Ontology> {
    rate.consume(Role::Shop, basket([(Asset::Durability, wear)]))
        .produce(Role::Shop, basket([(Asset::Wear, wear)]))
}
fn product(order: u8) -> (Asset, u64, u64) {
    if order.is_multiple_of(2) {
        (Asset::Chair, 3, 8 + 3 * order as u64)
    } else {
        (Asset::Table, 5, 18 + 3 * order as u64)
    }
}

pub(super) fn build(stress: bool) -> World {
    let (orders, target, wood, time, labor) = if stress {
        (5, 3, 16, 22, 14)
    } else {
        (3, 2, 10, 14, 8)
    };
    let mut stock = basket([
        (Asset::Wood, wood),
        (Asset::Time, time),
        (Asset::Labor, labor),
        (Asset::Tool, 1),
        (Asset::Durability, 3),
        (Asset::ToolPotential, 2),
        (Asset::RepairKit, 2),
        (Asset::UpgradeKit, 1),
        (Asset::Idle, 1),
        (Asset::Active, 1),
    ]);
    for order in 0..orders {
        stock.insert(Asset::Contract(order), Quantity::new(1));
    }
    let mut builder = EconomyBuilder::new().account(AccountId::Workshop, Account::from(stock));
    let mut occupancy = LinearInvariant::new("one workbench")
        .weight(Asset::Idle, 1)
        .weight(Asset::Solved, 1);
    for order in 0..orders {
        let (item, mass, price) = product(order);
        builder = builder
            .account(
                AccountId::Customer(order),
                Account::from(basket([
                    (Asset::CustomerIdentity(order), 1),
                    (Asset::Credit, price),
                ])),
            )
            .rate(
                RateId::Accept(order),
                law(1, 0)
                    .consume(
                        Role::Shop,
                        basket([(Asset::Idle, 1), (Asset::Contract(order), 1)]),
                    )
                    .produce(Role::Shop, basket([(Asset::Raw(order), 1)])),
            );
        for (precise, upgraded) in [(false, false), (true, false), (false, true)] {
            let wear = if precise || upgraded { 1 } else { 2 };
            let scrap = u64::from(!precise && !upgraded);
            let mut prepare = tooling(
                law(if precise { 3 } else { 1 }, if precise { 2 } else { 1 }),
                wear,
            )
            .consume(
                Role::Shop,
                basket([(Asset::Raw(order), 1), (Asset::Wood, mass + scrap)]),
            )
            .produce(
                Role::Shop,
                basket([
                    (Asset::Prepared(order), 1),
                    (Asset::Frame, 1),
                    (Asset::Panel, mass - 1),
                    (Asset::Offcut, scrap),
                ]),
            );
            if upgraded {
                prepare = prepare.preserve(Role::Shop, basket([(Asset::Upgraded, 1)]));
            }
            builder = builder.rate(
                RateId::Prepare {
                    order,
                    precise,
                    upgraded,
                },
                prepare,
            );
        }
        builder = builder
            .rate(
                RateId::Assemble(order),
                tooling(
                    law(
                        if item == Asset::Table { 2 } else { 1 },
                        if item == Asset::Table { 2 } else { 1 },
                    ),
                    if item == Asset::Table { 2 } else { 1 },
                )
                .consume(
                    Role::Shop,
                    basket([
                        (Asset::Prepared(order), 1),
                        (Asset::Frame, 1),
                        (Asset::Panel, mass - 1),
                    ]),
                )
                .produce(
                    Role::Shop,
                    basket([(Asset::Assembled(order), 1), (item, 1)]),
                ),
            )
            .rate(
                RateId::Deliver(order),
                law(1, 0)
                    .consume(
                        Role::Shop,
                        basket([(Asset::Assembled(order), 1), (item, 1)]),
                    )
                    .produce(
                        Role::Shop,
                        basket([
                            (Asset::Idle, 1),
                            (Asset::Fulfilled(order), 1),
                            (Asset::Completed, 1),
                            (Asset::Credit, price),
                        ]),
                    )
                    .preserve(
                        Role::Customer,
                        basket([(Asset::CustomerIdentity(order), 1)]),
                    )
                    .consume(Role::Customer, basket([(Asset::Credit, price)]))
                    .produce(Role::Customer, basket([(item, 1)]))
                    .distinct(Role::Shop, Role::Customer),
            )
            .invariant(
                LinearInvariant::new(format!("contract {order} lifecycle"))
                    .weight(Asset::Contract(order), 1)
                    .weight(Asset::Raw(order), 1)
                    .weight(Asset::Prepared(order), 1)
                    .weight(Asset::Assembled(order), 1)
                    .weight(Asset::Fulfilled(order), 1),
            );
        occupancy = occupancy
            .weight(Asset::Raw(order), 1)
            .weight(Asset::Prepared(order), 1)
            .weight(Asset::Assembled(order), 1);
    }
    // Maintenance is possible during a job: a depleted tool is recoverable,
    // but finite kits, labor and elapsed time remain real constraints.
    builder
        .rate(
            RateId::Repair,
            law(2, 1)
                .condition(
                    "tool has wear",
                    Q::balance(Role::Shop, Asset::Wear),
                    C::GreaterThanOrEqual,
                    Q::constant(1),
                )
                .consume(Role::Shop, basket([(Asset::RepairKit, 1)]))
                .produce(Role::Shop, basket([(Asset::UsedRepairKit, 1)]))
                .consume_computed(Role::Shop, Asset::Wear, Q::balance(Role::Shop, Asset::Wear))
                .produce_computed(
                    Role::Shop,
                    Asset::Durability,
                    Q::balance(Role::Shop, Asset::Wear),
                ),
        )
        .rate(
            RateId::Upgrade,
            law(4, 3)
                .preserve(Role::Shop, basket([(Asset::Idle, 1)]))
                .consume(
                    Role::Shop,
                    basket([(Asset::UpgradeKit, 1), (Asset::ToolPotential, 2)]),
                )
                .produce(
                    Role::Shop,
                    basket([(Asset::Upgraded, 1), (Asset::Durability, 2)]),
                ),
        )
        .rate(
            RateId::Recycle,
            law(2, 1)
                .preserve(Role::Shop, basket([(Asset::Idle, 1)]))
                .consume(Role::Shop, basket([(Asset::Offcut, 2)]))
                .produce(Role::Shop, basket([(Asset::Wood, 1), (Asset::Waste, 1)])),
        )
        .rate(
            RateId::CounterfeitChair,
            law(1, 0)
                .consume(Role::Shop, basket([(Asset::Wood, 1)]))
                .produce(Role::Shop, basket([(Asset::Chair, 2)])),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Shop, basket([(Asset::Completed, target)]))
                .consume(Role::Shop, basket([(Asset::Active, 1), (Asset::Idle, 1)]))
                .produce(Role::Shop, basket([(Asset::Solved, 1)]))
                .consume_computed(
                    Role::Shop,
                    Asset::Offcut,
                    Q::balance(Role::Shop, Asset::Offcut),
                )
                .produce_computed(
                    Role::Shop,
                    Asset::Waste,
                    Q::balance(Role::Shop, Asset::Offcut),
                ),
        )
        .invariant(occupancy)
        .invariant(
            LinearInvariant::new("material mass")
                .weight(Asset::Wood, 1)
                .weight(Asset::Frame, 1)
                .weight(Asset::Panel, 1)
                .weight(Asset::Chair, 3)
                .weight(Asset::Table, 5)
                .weight(Asset::Offcut, 1)
                .weight(Asset::Waste, 1),
        )
        .invariant(
            LinearInvariant::new("tool capacity")
                .weight(Asset::Durability, 1)
                .weight(Asset::Wear, 1)
                .weight(Asset::ToolPotential, 1),
        )
        .invariant(
            LinearInvariant::new("repair supplies")
                .weight(Asset::RepairKit, 1)
                .weight(Asset::UsedRepairKit, 1),
        )
        .invariant(
            LinearInvariant::new("upgrade opportunity")
                .weight(Asset::UpgradeKit, 1)
                .weight(Asset::Upgraded, 1),
        )
        .invariant(LinearInvariant::new("closed customer funds").weight(Asset::Credit, 1))
        .invariant(
            LinearInvariant::new("labor accounting")
                .weight(Asset::Labor, 1)
                .weight(Asset::SpentLabor, 1),
        )
        .invariant(
            LinearInvariant::new("time accounting")
                .weight(Asset::Time, 1)
                .weight(Asset::SpentTime, 1),
        )
        .invariant(
            LinearInvariant::new("workshop lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("production workshop preserves resources and contracts")
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(candidates(&world).is_empty());
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

    #[test]
    fn exact_plans_trade_contract_revenue_against_time_and_waste() {
        let world = initial_showcase();
        let front = pareto_front(&world).unwrap();
        let mut revenues = std::collections::BTreeSet::new();
        let mut upgraded = false;
        for entry in front.front().entries() {
            let replay = world.replayed(entry.payload()).unwrap();
            assert!(replay.matches(&goal()));
            assert_eq!(&objectives(&replay), entry.objectives());
            revenues.insert(
                replay
                    .balance(&AccountId::Workshop, &Asset::Credit)
                    .to_u64(),
            );
            upgraded |= !replay
                .balance(&AccountId::Workshop, &Asset::Upgraded)
                .is_zero();
        }
        assert!(
            revenues.len() >= 2,
            "contracts must create a real outcome tradeoff"
        );
        assert!(upgraded, "the upgrade must be economically useful");
        let shortest = solve_bfs(&world).unwrap();
        let leanest = minimize_waste(&world).unwrap();
        for trace in [shortest.trace(), leanest.trace()] {
            assert!(world.replayed(trace).unwrap().matches(&goal()));
        }
        let stress = initial_stress();
        assert!(
            stress
                .replayed(solve_bfs(&stress).unwrap().trace())
                .unwrap()
                .matches(&goal())
        );
    }
}
