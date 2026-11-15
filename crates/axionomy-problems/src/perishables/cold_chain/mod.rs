//! A cold chain with physical storage, thermal history and competing fuel uses.
use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Slot, basket,
};

pub const BATCHES: [u8; 6] = [0, 1, 2, 3, 4, 5];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Place {
    Warehouse,
    Fridge,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Batch(u8),
    Buyer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Active,
    Dormant,
    At(Place),
    Fresh,
    Preserved,
    Spoiled,
    Disposed,
    Delivered,
    Heat,
    Origin(u8),
    ColdSpace,
    ColdOccupancy,
    Energy,
    SpentEnergy,
    GridPotential,
    SpilledEnergy,
    Fuel,
    GeneratorFuel,
    TransportFuel,
    CoolingOn,
    CoolingOff,
    Labor,
    SpentLabor,
    FutureLabor,
    UnusedLabor,
    Freight,
    UsedFreight,
    FutureFreight,
    UnusedFreight,
    Money,
    Demand,
    FutureDemand,
    SatisfiedDemand,
    MissedDemand,
    Day,
    Days,
    Outage(u8),
    Decisions,
    Aging,
    PendingAge(u8),
    Solved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Batch,
    Source,
    Destination,
    Buyer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Split {
        source: u8,
        destination: u8,
        at: Place,
    },
    Move {
        batch: u8,
        from: Place,
        to: Place,
    },
    Process(u8),
    Deliver {
        batch: u8,
        preserved: bool,
        at: Place,
    },
    Discard {
        batch: u8,
        at: Place,
    },
    Cool,
    Generate,
    EndDay,
    AgeEmpty(u8),
    AgeCold(u8),
    AgeWarm {
        batch: u8,
        at: Place,
        spoil: bool,
    },
    OpenDay(u8),
    Close,
}
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `perishables/cold-chain` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "perishables/cold-chain";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
/// One unit of each asset `assets` name.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, q(role, asset))
}
/// A law of the cold chain, its world role fixed to the world's account.
fn law() -> Rate<Ontology> {
    Rate::new().account(Role::World, Pattern::literal(AccountId::World))
}
fn decisions() -> Rate<Ontology> {
    law().preserve(Role::World, basket([(Asset::Decisions, 1)]))
}
fn once(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.condition(path!("one-event"), Q::units(), C::Equal, Q::constant(1))
}
/// `rate` for each batch, `{batch}`, its role fixed to that batch's account.
fn each_batch(rate: Rate<Ontology>, batch: &Param<u8>) -> Rate<Ontology> {
    rate.variable(batch.range(BATCHES[0], BATCHES[5]))
        .account(Role::Batch, AccountIdPattern::Batch(batch.var()).into())
}
/// `rate` on an active batch `{batch}`.
fn active(rate: Rate<Ontology>, batch: &Param<u8>) -> Rate<Ontology> {
    each_batch(rate, batch).preserve(Role::Batch, basket([(Asset::Active, 1)]))
}
/// The batch `{batch}` aging at the day's end.
fn aging(batch: &Param<u8>) -> Rate<Ontology> {
    each_batch(once(law()), batch)
        .preserve(Role::World, basket([(Asset::Aging, 1)]))
        .consume_at(
            Role::World,
            one([AssetPattern::PendingAge(batch.var()).into()]),
        )
}
fn ages_resolved(mut rate: Rate<Ontology>) -> Rate<Ontology> {
    for id in BATCHES {
        rate = rate.condition(
            path!("batch-aging-settled").join(&path::write(&id)),
            q(Role::World, Asset::PendingAge(id)),
            C::Equal,
            Q::constant(0),
        );
    }
    rate
}
pub fn initial(stress: bool, seed: u64) -> World {
    let days = if stress { 8 } else { 6 };
    let cold = if stress { 3 } else { 4 };
    let outage_start = 2 + (seed % 2) as u8;
    let outage_days = if stress { 3 } else { 2 };
    let mut environment = basket([
        (Asset::Decisions, 1),
        (Asset::CoolingOff, 1),
        (Asset::Days, days),
        (Asset::Energy, 3),
        (Asset::GridPotential, 3 * (days - 1 - outage_days)),
        (Asset::Fuel, if stress { 5 } else { 6 }),
        (Asset::Labor, 2),
        (Asset::FutureLabor, (days - 1) * 2),
        (Asset::Freight, 2),
        (Asset::FutureFreight, (days - 1) * 2),
        (Asset::Demand, 2),
        (Asset::FutureDemand, (days - 1) * 2),
    ]);
    for day in outage_start..outage_start + outage_days as u8 {
        environment.insert(Asset::Outage(day), Quantity::new(1));
    }
    let mut builder = EconomyBuilder::new()
        .account(AccountId::World, Account::from(environment))
        .account(
            AccountId::Buyer,
            Account::from(basket([(Asset::Money, days * 6)])),
        );
    for id in BATCHES {
        let assets = if id < 2 {
            basket([
                (Asset::Active, 1),
                (
                    Asset::At(if id == 0 {
                        Place::Warehouse
                    } else {
                        Place::Fridge
                    }),
                    1,
                ),
                (Asset::Origin(id), 1),
                (
                    Asset::Fresh,
                    if id == 0 {
                        if stress { 9 } else { 6 }
                    } else {
                        cold
                    },
                ),
                (Asset::ColdOccupancy, if id == 1 { cold } else { 0 }),
            ])
        } else {
            basket([(Asset::Dormant, 1)])
        };
        builder = builder.account(AccountId::Batch(id), Account::from(assets));
    }

    // The rates' variables: a batch, and the batches a split takes from and
    // starts.
    let batch = Param::<u8>::new(path!("batch"));
    let (source, destination) = (
        Param::<u8>::new(path!("source")),
        Param::<u8>::new(path!("destination")),
    );
    for at in [Place::Warehouse, Place::Fridge] {
        let mut split = decisions()
            .variable(source.range(BATCHES[0], BATCHES[5]))
            .variable(destination.range(BATCHES[0], BATCHES[5]))
            .distinct_variables(source.name().clone(), destination.name().clone())
            .account(Role::Source, AccountIdPattern::Batch(source.var()).into())
            .account(
                Role::Destination,
                AccountIdPattern::Batch(destination.var()).into(),
            )
            .preserve(
                Role::Source,
                basket([(Asset::Active, 1), (Asset::At(at), 1)]),
            )
            .condition(
                path!("leave-a-nonempty-parent-cohort"),
                q(Role::Source, Asset::Fresh),
                C::GreaterThanOrEqual,
                Q::units() + Q::constant(1),
            )
            .consume(Role::Source, basket([(Asset::Fresh, 1)]))
            .consume_computed(Role::Destination, Asset::Dormant, Q::constant(1))
            .produce(Role::Destination, basket([(Asset::Fresh, 1)]))
            .produce_computed(Role::Destination, Asset::Active, Q::constant(1))
            .produce_computed(Role::Destination, Asset::At(at), Q::constant(1))
            .produce_computed(Role::Destination, Asset::Heat, q(Role::Source, Asset::Heat));
        for origin in [0, 1] {
            split = split.produce_computed(
                Role::Destination,
                Asset::Origin(origin),
                q(Role::Source, Asset::Origin(origin)),
            );
        }
        if at == Place::Fridge {
            split = split
                .consume(Role::Source, basket([(Asset::ColdOccupancy, 1)]))
                .produce(Role::Destination, basket([(Asset::ColdOccupancy, 1)]));
        }
        builder = builder.rate_at(
            RateIdPattern::Split {
                source: source.var(),
                destination: destination.var(),
                at: Slot::Is(at),
            }
            .into(),
            split,
        );
        let to = if at == Place::Warehouse {
            Place::Fridge
        } else {
            Place::Warehouse
        };
        let amount = q(Role::Batch, Asset::Fresh)
            + q(Role::Batch, Asset::Preserved)
            + q(Role::Batch, Asset::Spoiled);
        let mut movement = once(active(decisions(), &batch))
            .consume(Role::Batch, basket([(Asset::At(at), 1)]))
            .produce(Role::Batch, basket([(Asset::At(to), 1)]))
            .condition(
                path!("move-a-nonempty-cohort"),
                amount.clone(),
                C::GreaterThanOrEqual,
                Q::constant(1),
            )
            .consume(Role::World, basket([(Asset::Labor, 1)]))
            .produce(Role::World, basket([(Asset::SpentLabor, 1)]));
        if to == Place::Fridge {
            movement = movement
                .consume_computed(Role::World, Asset::ColdSpace, amount.clone())
                .produce_computed(Role::Batch, Asset::ColdOccupancy, amount);
        } else {
            movement = movement
                .consume_computed(Role::Batch, Asset::ColdOccupancy, amount.clone())
                .produce_computed(Role::World, Asset::ColdSpace, amount);
        }
        builder = builder.rate_at(
            RateIdPattern::Move {
                batch: batch.var(),
                from: Slot::Is(at),
                to: Slot::Is(to),
            }
            .into(),
            movement,
        );
        for preserved in [false, true] {
            let price = if preserved { 2 } else { 3 };
            let mut deliver = active(decisions(), &batch)
                .account(Role::Buyer, Pattern::literal(AccountId::Buyer))
                .preserve(Role::Batch, basket([(Asset::At(at), 1)]))
                .consume(
                    Role::Batch,
                    basket([(
                        if preserved {
                            Asset::Preserved
                        } else {
                            Asset::Fresh
                        },
                        1,
                    )]),
                )
                .consume(Role::Buyer, basket([(Asset::Money, price)]))
                .produce(Role::Buyer, basket([(Asset::Delivered, 1)]))
                .consume(
                    Role::World,
                    basket([(Asset::Freight, 1), (Asset::Demand, 1)]),
                )
                .consume_computed(Role::World, Asset::Fuel, Q::constant(1))
                .produce_computed(Role::World, Asset::TransportFuel, Q::constant(1))
                .produce(
                    Role::World,
                    basket([
                        (Asset::Money, price),
                        (Asset::SatisfiedDemand, 1),
                        (Asset::UsedFreight, 1),
                    ]),
                );
            if at == Place::Fridge {
                deliver = deliver
                    .consume(Role::Batch, basket([(Asset::ColdOccupancy, 1)]))
                    .produce(Role::World, basket([(Asset::ColdSpace, 1)]));
            }
            builder = builder.rate_at(
                RateIdPattern::Deliver {
                    batch: batch.var(),
                    preserved: Slot::Is(preserved),
                    at: Slot::Is(at),
                }
                .into(),
                deliver,
            );
        }
        let mut discard = active(decisions(), &batch)
            .preserve(Role::Batch, basket([(Asset::At(at), 1)]))
            .consume(Role::Batch, basket([(Asset::Spoiled, 1)]))
            .produce(Role::World, basket([(Asset::Disposed, 1)]));
        if at == Place::Fridge {
            discard = discard
                .consume(Role::Batch, basket([(Asset::ColdOccupancy, 1)]))
                .produce(Role::World, basket([(Asset::ColdSpace, 1)]));
        }
        builder = builder.rate_at(
            RateIdPattern::Discard {
                batch: batch.var(),
                at: Slot::Is(at),
            }
            .into(),
            discard,
        );
        for spoil in [false, true] {
            let mut warm = aging(&batch)
                .preserve(Role::Batch, basket([(Asset::At(at), 1)]))
                .condition(
                    path!("fresh-stock-is-present"),
                    q(Role::Batch, Asset::Fresh),
                    C::GreaterThanOrEqual,
                    Q::constant(1),
                )
                .condition(
                    path!("thermal-threshold"),
                    q(Role::Batch, Asset::Heat),
                    if spoil {
                        C::GreaterThanOrEqual
                    } else {
                        C::LessThanOrEqual
                    },
                    Q::constant(if spoil { 2 } else { 1 }),
                )
                .produce(Role::Batch, basket([(Asset::Heat, 2)]));
            if at == Place::Fridge {
                warm = warm.preserve(Role::World, basket([(Asset::CoolingOff, 1)]));
            }
            if spoil {
                warm = clear(warm, Role::Batch, Asset::Fresh).produce_computed(
                    Role::Batch,
                    Asset::Spoiled,
                    q(Role::Batch, Asset::Fresh),
                );
            }
            builder = builder.rate_at(
                RateIdPattern::AgeWarm {
                    batch: batch.var(),
                    at: Slot::Is(at),
                    spoil: Slot::Is(spoil),
                }
                .into(),
                warm,
            );
        }
    }
    builder = builder.rate_at(
        RateIdPattern::Process(batch.var()).into(),
        active(decisions(), &batch)
            .preserve(Role::Batch, basket([(Asset::At(Place::Warehouse), 1)]))
            .consume(Role::Batch, basket([(Asset::Fresh, 1)]))
            .produce(Role::Batch, basket([(Asset::Preserved, 1)]))
            .consume(Role::World, basket([(Asset::Energy, 1), (Asset::Labor, 1)]))
            .produce(
                Role::World,
                basket([(Asset::SpentEnergy, 1), (Asset::SpentLabor, 1)]),
            ),
    );
    builder = builder.rate_at(
        RateIdPattern::AgeEmpty(batch.var()).into(),
        aging(&batch).condition(
            path!("no-fresh-stock-to-age"),
            q(Role::Batch, Asset::Fresh),
            C::Equal,
            Q::constant(0),
        ),
    );
    builder = builder.rate_at(
        RateIdPattern::AgeCold(batch.var()).into(),
        aging(&batch)
            .preserve(
                Role::Batch,
                basket([(Asset::At(Place::Fridge), 1), (Asset::Fresh, 1)]),
            )
            .preserve(Role::World, basket([(Asset::CoolingOn, 1)])),
    );
    builder = builder.rate(
        RateId::Cool,
        once(decisions())
            .consume(
                Role::World,
                basket([(Asset::CoolingOff, 1), (Asset::Energy, 2)]),
            )
            .produce(
                Role::World,
                basket([(Asset::CoolingOn, 1), (Asset::SpentEnergy, 2)]),
            ),
    );
    builder = builder.rate(
        RateId::Generate,
        once(decisions())
            .condition(
                path!("battery-capacity"),
                q(Role::World, Asset::Energy),
                C::LessThanOrEqual,
                Q::constant(3),
            )
            .consume(Role::World, basket([(Asset::Fuel, 1)]))
            .produce(
                Role::World,
                basket([(Asset::GeneratorFuel, 1), (Asset::Energy, 3)]),
            ),
    );
    let mut end = once(law())
        .consume(Role::World, basket([(Asset::Decisions, 1)]))
        .produce(Role::World, basket([(Asset::Aging, 1)]));
    for id in BATCHES {
        end = end.produce(Role::World, basket([(Asset::PendingAge(id), 1)]));
    }
    builder = builder.rate(RateId::EndDay, end);
    // Each day opens with the grid's supply, three units of electricity
    // but none on a day of the outage the world records, as much of it as
    // the battery takes.
    let day = Param::<u8>::new(path!("day"));
    let supply = Q::constant(3)
        * (Q::constant(1) - Q::balance_at(Role::World, AssetPattern::Outage(day.var()).into()));
    let accepted = Q::minimum(
        supply.clone(),
        Q::constant(6) - q(Role::World, Asset::Energy),
    );
    let mut open = ages_resolved(once(law()))
        .variable(day.range(1, days as u8 - 1))
        .condition(
            path!("next-day"),
            q(Role::World, Asset::Day) + Q::constant(1),
            C::Equal,
            day.quantity(),
        )
        .consume(
            Role::World,
            basket([
                (Asset::Aging, 1),
                (Asset::FutureDemand, 2),
                (Asset::FutureLabor, 2),
                (Asset::FutureFreight, 2),
            ]),
        )
        .consume_computed(Role::World, Asset::GridPotential, supply.clone())
        .produce(
            Role::World,
            basket([
                (Asset::Decisions, 1),
                (Asset::Day, 1),
                (Asset::Demand, 2),
                (Asset::Labor, 2),
                (Asset::Freight, 2),
            ]),
        )
        .produce_computed(Role::World, Asset::Energy, accepted.clone())
        .produce_computed(Role::World, Asset::SpilledEnergy, supply - accepted);
    for (from, to) in [
        (Asset::Demand, Asset::MissedDemand),
        (Asset::Labor, Asset::UnusedLabor),
        (Asset::Freight, Asset::UnusedFreight),
        (Asset::CoolingOn, Asset::CoolingOff),
    ] {
        open =
            clear(open, Role::World, from).produce_computed(Role::World, to, q(Role::World, from));
    }
    builder = builder.rate_at(RateIdPattern::OpenDay(day.var()).into(), open);
    builder = builder.rate(
        RateId::Close,
        ages_resolved(once(law()))
            .condition(
                path!("last-day"),
                q(Role::World, Asset::Day) + Q::constant(1),
                C::Equal,
                q(Role::World, Asset::Days),
            )
            .consume(Role::World, basket([(Asset::Aging, 1)]))
            .produce(Role::World, basket([(Asset::Solved, 1)]))
            .consume_computed(Role::World, Asset::Demand, q(Role::World, Asset::Demand))
            .produce_computed(
                Role::World,
                Asset::MissedDemand,
                q(Role::World, Asset::Demand),
            ),
    );
    builder
        .invariant(
            LinearInvariant::new(path!("food-mass"))
                .weight(Asset::Fresh, 1)
                .weight(Asset::Preserved, 1)
                .weight(Asset::Spoiled, 1)
                .weight(Asset::Disposed, 1)
                .weight(Asset::Delivered, 1),
        )
        .invariant(
            LinearInvariant::new(path!("batch-activation"))
                .weight(Asset::Active, 1)
                .weight(Asset::Dormant, 1),
        )
        .invariant(
            LinearInvariant::new(path!("physical-cold-storage"))
                .weight(Asset::ColdSpace, 1)
                .weight(Asset::ColdOccupancy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("closed-electricity-and-fuel-potential"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1)
                .weight(Asset::GridPotential, 1)
                .weight(Asset::SpilledEnergy, 1)
                .weight(Asset::Fuel, 3)
                .weight(Asset::TransportFuel, 3),
        )
        .invariant(
            LinearInvariant::new(path!("finite-fuel"))
                .weight(Asset::Fuel, 1)
                .weight(Asset::GeneratorFuel, 1)
                .weight(Asset::TransportFuel, 1),
        )
        .invariant(
            LinearInvariant::new(path!("cooling-switch"))
                .weight(Asset::CoolingOn, 1)
                .weight(Asset::CoolingOff, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-labor"))
                .weight(Asset::Labor, 1)
                .weight(Asset::SpentLabor, 1)
                .weight(Asset::FutureLabor, 1)
                .weight(Asset::UnusedLabor, 1),
        )
        .invariant(
            LinearInvariant::new(path!("daily-transport-capacity"))
                .weight(Asset::Freight, 1)
                .weight(Asset::UsedFreight, 1)
                .weight(Asset::FutureFreight, 1)
                .weight(Asset::UnusedFreight, 1),
        )
        .invariant(
            LinearInvariant::new(path!("daily-food-obligations"))
                .weight(Asset::Demand, 1)
                .weight(Asset::FutureDemand, 1)
                .weight(Asset::SatisfiedDemand, 1)
                .weight(Asset::MissedDemand, 1),
        )
        .invariant(LinearInvariant::new(path!("funded-food-purchases")).weight(Asset::Money, 1))
        .invariant(
            LinearInvariant::new(path!("cold-chain-lifecycle"))
                .weight(Asset::Decisions, 1)
                .weight(Asset::Aging, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("closed cold chain")
}

/// The exchange of `units` of `rate`, each of its roles bound to the
/// account `world`'s rate fixes it to.
///
/// # Panics
///
/// If `rate` names no instance of `world`'s rates.
pub fn action(world: &World, rate: RateId, units: u64) -> Action {
    let instance = world
        .instance(&rate)
        .unwrap_or_else(|_| panic!("the cold chain has no rate {rate:?}"));
    instance.fixed_accounts().fold(
        Exchange::new(rate, Quantity::new(units)),
        |exchange, (role, account)| {
            exchange.bind(
                *role,
                *account
                    .as_literal()
                    .expect("an instance's accounts are written out"),
            )
        },
    )
}
pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::World, basket([(Asset::Solved, 1)]))
}
pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}
pub fn total(world: &World, asset: Asset) -> u64 {
    world
        .accounts()
        .map(|(_, account)| account.balance(&asset).to_u64())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Ends the day, each batch aging by `ages`, and opens day `next`.
    fn end_day(world: &mut World, next: u8, ages: [RateId; 6]) {
        world.apply(action(world, RateId::EndDay, 1)).unwrap();
        assert!(
            !world.is_applicable(&action(world, RateId::OpenDay(next), 1)),
            "all cohort exposure must settle before the next day"
        );
        for age in ages {
            world
                .apply(action(world, age, 1))
                .unwrap_or_else(|error| panic!("{age:?}: {error}"));
        }
        world
            .apply(action(world, RateId::OpenDay(next), 1))
            .unwrap();
    }
    #[test]
    fn split_cohorts_inherit_heat_and_diverge_only_after_different_storage_exposure() {
        let mut world = initial(false, 17);
        world.apply(action(&world, RateId::Cool, 1)).unwrap();
        end_day(
            &mut world,
            1,
            [
                RateId::AgeWarm {
                    batch: 0,
                    at: Place::Warehouse,
                    spoil: false,
                },
                RateId::AgeCold(1),
                RateId::AgeEmpty(2),
                RateId::AgeEmpty(3),
                RateId::AgeEmpty(4),
                RateId::AgeEmpty(5),
            ],
        );
        assert_eq!(value(&world, AccountId::Batch(0), Asset::Heat), 2);
        let before = world.state_key();
        assert!(
            world
                .apply(action(
                    &world,
                    RateId::Move {
                        batch: 0,
                        from: Place::Warehouse,
                        to: Place::Fridge
                    },
                    1
                ))
                .is_err()
        );
        assert_eq!(before, world.state_key());
        world
            .apply(action(
                &world,
                RateId::Deliver {
                    batch: 1,
                    preserved: false,
                    at: Place::Fridge,
                },
                2,
            ))
            .unwrap();
        world
            .apply(action(
                &world,
                RateId::Split {
                    source: 0,
                    destination: 2,
                    at: Place::Warehouse,
                },
                2,
            ))
            .unwrap();
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Heat), 2);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Origin(0)), 1);
        world
            .apply(action(
                &world,
                RateId::Move {
                    batch: 2,
                    from: Place::Warehouse,
                    to: Place::Fridge,
                },
                1,
            ))
            .unwrap();
        world.apply(action(&world, RateId::Cool, 1)).unwrap();
        // The split-off child ages cold; its warm parent spoils.
        end_day(
            &mut world,
            2,
            [
                RateId::AgeWarm {
                    batch: 0,
                    at: Place::Warehouse,
                    spoil: true,
                },
                RateId::AgeCold(1),
                RateId::AgeCold(2),
                RateId::AgeEmpty(3),
                RateId::AgeEmpty(4),
                RateId::AgeEmpty(5),
            ],
        );
        assert_eq!(value(&world, AccountId::Batch(0), Asset::Spoiled), 4);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Fresh), 2);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Heat), 2);
        assert_eq!(total(&world, Asset::ColdOccupancy), 4);
    }
    #[test]
    fn backup_and_transport_draw_from_the_same_finite_fuel() {
        let mut world = initial(false, 17);
        world.apply(action(&world, RateId::Generate, 1)).unwrap();
        assert_eq!(value(&world, AccountId::World, Asset::Energy), 6);
        assert!(!world.is_applicable(&action(&world, RateId::Generate, 1)));
        world
            .apply(action(
                &world,
                RateId::Deliver {
                    batch: 0,
                    preserved: false,
                    at: Place::Warehouse,
                },
                2,
            ))
            .unwrap();
        assert_eq!(value(&world, AccountId::World, Asset::Fuel), 4);
        assert_eq!(value(&world, AccountId::World, Asset::Money), 6);
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Deliver {
                batch: 0,
                preserved: false,
                at: Place::Warehouse
            },
            1
        )));
        let bad = action(&world, RateId::Process(0), 1).bind(Role::Batch, AccountId::Batch(1));
        assert!(!world.is_applicable(&bad));
    }
}
