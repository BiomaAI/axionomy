//! A cold chain with physical storage, thermal history and competing fuel uses.
use axionomy::path;
use axionomy::path::slug;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Trace, basket,
};

pub const BATCHES: [u8; 6] = [0, 1, 2, 3, 4, 5];
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Place {
    Warehouse,
    Fridge,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum AccountId {
    World,
    Batch(u8),
    Buyer,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Asset {
    WorldIdentity,
    BatchIdentity(u8),
    BuyerIdentity,
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Role {
    World,
    Batch,
    Source,
    Destination,
    Buyer,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    ProtectCold,
    ProcessFirst,
    ConserveFuel,
    RebalanceCold,
}

fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, q(role, asset))
}
fn law() -> Rate<Ontology> {
    Rate::new().preserve(Role::World, basket([(Asset::WorldIdentity, 1)]))
}
fn decisions() -> Rate<Ontology> {
    law().preserve(Role::World, basket([(Asset::Decisions, 1)]))
}
fn once(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.condition(path!("one-event"), Q::units(), C::Equal, Q::constant(1))
}
fn batch(rate: Rate<Ontology>, id: u8) -> Rate<Ontology> {
    rate.preserve(
        Role::Batch,
        basket([(Asset::BatchIdentity(id), 1), (Asset::Active, 1)]),
    )
    .distinct(Role::Batch, Role::World)
}
fn aging(id: u8) -> Rate<Ontology> {
    once(law())
        .preserve(Role::World, basket([(Asset::Aging, 1)]))
        .preserve(Role::Batch, basket([(Asset::BatchIdentity(id), 1)]))
        .consume(Role::World, basket([(Asset::PendingAge(id), 1)]))
        .distinct(Role::Batch, Role::World)
}
fn ages_resolved(mut rate: Rate<Ontology>) -> Rate<Ontology> {
    for id in BATCHES {
        rate = rate.condition(
            slug(format!("batch {id} aging settled")),
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
        (Asset::WorldIdentity, 1),
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
            Account::from(basket([
                (Asset::BuyerIdentity, 1),
                (Asset::Money, days * 6),
            ])),
        );
    for id in BATCHES {
        let assets = if id < 2 {
            basket([
                (Asset::BatchIdentity(id), 1),
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
            basket([(Asset::BatchIdentity(id), 1), (Asset::Dormant, 1)])
        };
        builder = builder.account(AccountId::Batch(id), Account::from(assets));
        for at in [Place::Warehouse, Place::Fridge] {
            for destination in BATCHES.into_iter().filter(|other| *other != id) {
                let mut split = decisions()
                    .preserve(
                        Role::Source,
                        basket([
                            (Asset::BatchIdentity(id), 1),
                            (Asset::Active, 1),
                            (Asset::At(at), 1),
                        ]),
                    )
                    .preserve(
                        Role::Destination,
                        basket([(Asset::BatchIdentity(destination), 1)]),
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
                    .produce_computed(Role::Destination, Asset::Heat, q(Role::Source, Asset::Heat))
                    .distinct(Role::Source, Role::Destination)
                    .distinct(Role::Source, Role::World)
                    .distinct(Role::Destination, Role::World);
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
                builder = builder.rate(
                    RateId::Split {
                        source: id,
                        destination,
                        at,
                    },
                    split,
                );
            }
            let to = if at == Place::Warehouse {
                Place::Fridge
            } else {
                Place::Warehouse
            };
            let amount = q(Role::Batch, Asset::Fresh)
                + q(Role::Batch, Asset::Preserved)
                + q(Role::Batch, Asset::Spoiled);
            let mut movement = once(batch(decisions(), id))
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
            builder = builder.rate(
                RateId::Move {
                    batch: id,
                    from: at,
                    to,
                },
                movement,
            );
            for preserved in [false, true] {
                let price = if preserved { 2 } else { 3 };
                let mut deliver = batch(decisions(), id)
                    .preserve(Role::Batch, basket([(Asset::At(at), 1)]))
                    .preserve(Role::Buyer, basket([(Asset::BuyerIdentity, 1)]))
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
                    )
                    .distinct(Role::Buyer, Role::Batch)
                    .distinct(Role::Buyer, Role::World);
                if at == Place::Fridge {
                    deliver = deliver
                        .consume(Role::Batch, basket([(Asset::ColdOccupancy, 1)]))
                        .produce(Role::World, basket([(Asset::ColdSpace, 1)]));
                }
                builder = builder.rate(
                    RateId::Deliver {
                        batch: id,
                        preserved,
                        at,
                    },
                    deliver,
                );
            }
            let mut discard = batch(decisions(), id)
                .preserve(Role::Batch, basket([(Asset::At(at), 1)]))
                .consume(Role::Batch, basket([(Asset::Spoiled, 1)]))
                .produce(Role::World, basket([(Asset::Disposed, 1)]));
            if at == Place::Fridge {
                discard = discard
                    .consume(Role::Batch, basket([(Asset::ColdOccupancy, 1)]))
                    .produce(Role::World, basket([(Asset::ColdSpace, 1)]));
            }
            builder = builder.rate(RateId::Discard { batch: id, at }, discard);
            for spoil in [false, true] {
                let mut warm = aging(id)
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
                builder = builder.rate(
                    RateId::AgeWarm {
                        batch: id,
                        at,
                        spoil,
                    },
                    warm,
                );
            }
        }
        builder = builder.rate(
            RateId::Process(id),
            batch(decisions(), id)
                .preserve(Role::Batch, basket([(Asset::At(Place::Warehouse), 1)]))
                .consume(Role::Batch, basket([(Asset::Fresh, 1)]))
                .produce(Role::Batch, basket([(Asset::Preserved, 1)]))
                .consume(Role::World, basket([(Asset::Energy, 1), (Asset::Labor, 1)]))
                .produce(
                    Role::World,
                    basket([(Asset::SpentEnergy, 1), (Asset::SpentLabor, 1)]),
                ),
        );
        builder = builder.rate(
            RateId::AgeEmpty(id),
            aging(id).condition(
                path!("no-fresh-stock-to-age"),
                q(Role::Batch, Asset::Fresh),
                C::Equal,
                Q::constant(0),
            ),
        );
        builder = builder.rate(
            RateId::AgeCold(id),
            aging(id)
                .preserve(
                    Role::Batch,
                    basket([(Asset::At(Place::Fridge), 1), (Asset::Fresh, 1)]),
                )
                .preserve(Role::World, basket([(Asset::CoolingOn, 1)])),
        );
        builder = builder.invariant(
            LinearInvariant::new(slug(format!("batch {id} identity")))
                .weight(Asset::BatchIdentity(id), 1),
        );
    }
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
    for day in 1..days as u8 {
        let supply = if (outage_start..outage_start + outage_days as u8).contains(&day) {
            0
        } else {
            3
        };
        let accepted = Q::minimum(
            Q::constant(supply),
            Q::constant(6) - q(Role::World, Asset::Energy),
        );
        let mut open = ages_resolved(once(law()))
            .condition(
                path!("next-day"),
                q(Role::World, Asset::Day) + Q::constant(1),
                C::Equal,
                Q::constant(u64::from(day)),
            )
            .consume(
                Role::World,
                basket([
                    (Asset::Aging, 1),
                    (Asset::FutureDemand, 2),
                    (Asset::FutureLabor, 2),
                    (Asset::FutureFreight, 2),
                    (Asset::GridPotential, supply),
                ]),
            )
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
            .produce_computed(
                Role::World,
                Asset::SpilledEnergy,
                Q::constant(supply) - accepted,
            );
        for (from, to) in [
            (Asset::Demand, Asset::MissedDemand),
            (Asset::Labor, Asset::UnusedLabor),
            (Asset::Freight, Asset::UnusedFreight),
            (Asset::CoolingOn, Asset::CoolingOff),
        ] {
            open = clear(open, Role::World, from).produce_computed(
                Role::World,
                to,
                q(Role::World, from),
            );
        }
        builder = builder.rate(RateId::OpenDay(day), open);
    }
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

pub fn action(rate: RateId, units: u64) -> Action {
    let base = Exchange::new(rate, Quantity::new(units)).bind(Role::World, AccountId::World);
    match rate {
        RateId::Split {
            source,
            destination,
            ..
        } => base
            .bind(Role::Source, AccountId::Batch(source))
            .bind(Role::Destination, AccountId::Batch(destination)),
        RateId::Deliver { batch, .. } => base
            .bind(Role::Batch, AccountId::Batch(batch))
            .bind(Role::Buyer, AccountId::Buyer),
        RateId::Move { batch, .. }
        | RateId::Process(batch)
        | RateId::Discard { batch, .. }
        | RateId::AgeEmpty(batch)
        | RateId::AgeCold(batch)
        | RateId::AgeWarm { batch, .. } => base.bind(Role::Batch, AccountId::Batch(batch)),
        _ => base,
    }
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
pub fn place(world: &World, id: u8) -> Option<Place> {
    [Place::Warehouse, Place::Fridge]
        .into_iter()
        .find(|at| value(world, AccountId::Batch(id), Asset::At(*at)) > 0)
}
pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().flat_map(|id| {
        let units: Vec<_> = if matches!(
            id,
            RateId::Split { .. }
                | RateId::Deliver { .. }
                | RateId::Process(_)
                | RateId::Discard { .. }
        ) {
            (1..=12).collect()
        } else {
            vec![1]
        };
        units.into_iter().map(move |units| action(id, units))
    }))
}

fn record(world: &mut World, trace: &mut Trace<Ontology>, rate: RateId, units: u64) -> bool {
    let exchange = action(rate, units);
    if units == 0 || !world.is_applicable(&exchange) {
        return false;
    }
    world.apply(exchange.clone()).expect("assessed action");
    trace.push(exchange);
    true
}
fn cool(world: &mut World, trace: &mut Trace<Ontology>, backup: bool) {
    let cold_fresh: u64 = BATCHES
        .into_iter()
        .filter(|id| place(world, *id) == Some(Place::Fridge))
        .map(|id| value(world, AccountId::Batch(id), Asset::Fresh))
        .sum();
    if cold_fresh == 0 {
        return;
    }
    if backup
        && value(world, AccountId::World, Asset::Energy) < 2
        && value(world, AccountId::World, Asset::Fuel) >= 2
    {
        record(world, trace, RateId::Generate, 1);
    }
    record(world, trace, RateId::Cool, 1);
}
fn process(world: &mut World, trace: &mut Trace<Ontology>) {
    for id in BATCHES {
        if place(world, id) == Some(Place::Warehouse) {
            let amount = value(world, AccountId::Batch(id), Asset::Fresh)
                .min(value(world, AccountId::World, Asset::Labor))
                .min(value(world, AccountId::World, Asset::Energy));
            record(world, trace, RateId::Process(id), amount);
        }
    }
}
pub fn run(initial: &World, policy: Policy) -> Trace<Ontology> {
    let mut world = initial.clone();
    let mut trace = Trace::new();
    let days = value(&world, AccountId::World, Asset::Days);
    for day in 0..days {
        if matches!(policy, Policy::ProtectCold | Policy::RebalanceCold) {
            cool(&mut world, &mut trace, true);
        }
        if policy == Policy::ProcessFirst {
            process(&mut world, &mut trace);
        }
        if policy != Policy::ConserveFuel || day % 2 == 0 {
            let mut shipments: Vec<_> = BATCHES
                .into_iter()
                .flat_map(|id| {
                    let at = place(&world, id);
                    [false, true]
                        .into_iter()
                        .filter_map(move |preserved| at.map(|at| (id, preserved, at)))
                })
                .collect();
            shipments.sort_by_key(|(id, preserved, at)| {
                (
                    *preserved,
                    std::cmp::Reverse(
                        value(&world, AccountId::Batch(*id), Asset::Heat) * 10
                            + if policy == Policy::RebalanceCold {
                                u64::from(*at == Place::Fridge) * 100
                            } else {
                                u64::from(*at == Place::Warehouse)
                            },
                    ),
                )
            });
            for (id, preserved, at) in shipments {
                let amount = value(
                    &world,
                    AccountId::Batch(id),
                    if preserved {
                        Asset::Preserved
                    } else {
                        Asset::Fresh
                    },
                )
                .min(value(&world, AccountId::World, Asset::Freight))
                .min(value(&world, AccountId::World, Asset::Demand));
                record(
                    &mut world,
                    &mut trace,
                    RateId::Deliver {
                        batch: id,
                        preserved,
                        at,
                    },
                    amount,
                );
            }
        }
        if !matches!(policy, Policy::ProcessFirst | Policy::RebalanceCold) {
            process(&mut world, &mut trace);
        }
        if !matches!(policy, Policy::ProtectCold | Policy::RebalanceCold) {
            cool(&mut world, &mut trace, policy == Policy::ProcessFirst);
        }
        for id in BATCHES {
            if let Some(at) = place(&world, id) {
                let amount = value(&world, AccountId::Batch(id), Asset::Spoiled);
                record(
                    &mut world,
                    &mut trace,
                    RateId::Discard { batch: id, at },
                    amount,
                );
            }
        }
        // Split only when a child can actually fit. It inherits the parent's
        // accumulated heat; moving it to cold storage never resets that history.
        for id in BATCHES {
            if place(&world, id) != Some(Place::Warehouse)
                || value(&world, AccountId::World, Asset::Labor) == 0
            {
                continue;
            }
            let fresh = value(&world, AccountId::Batch(id), Asset::Fresh);
            let space = value(&world, AccountId::World, Asset::ColdSpace);
            if fresh == 0 || space == 0 {
                continue;
            }
            let all = fresh + value(&world, AccountId::Batch(id), Asset::Preserved);
            if all <= space {
                record(
                    &mut world,
                    &mut trace,
                    RateId::Move {
                        batch: id,
                        from: Place::Warehouse,
                        to: Place::Fridge,
                    },
                    1,
                );
            } else if fresh > space
                && let Some(child) = BATCHES
                    .into_iter()
                    .find(|other| value(&world, AccountId::Batch(*other), Asset::Dormant) > 0)
                && record(
                    &mut world,
                    &mut trace,
                    RateId::Split {
                        source: id,
                        destination: child,
                        at: Place::Warehouse,
                    },
                    space,
                )
            {
                record(
                    &mut world,
                    &mut trace,
                    RateId::Move {
                        batch: child,
                        from: Place::Warehouse,
                        to: Place::Fridge,
                    },
                    1,
                );
            }
        }
        if policy == Policy::RebalanceCold {
            process(&mut world, &mut trace);
        }
        assert!(record(&mut world, &mut trace, RateId::EndDay, 1));
        for id in BATCHES {
            if value(&world, AccountId::Batch(id), Asset::Fresh) == 0 {
                assert!(record(&mut world, &mut trace, RateId::AgeEmpty(id), 1));
            } else if place(&world, id) == Some(Place::Fridge)
                && value(&world, AccountId::World, Asset::CoolingOn) > 0
            {
                assert!(record(&mut world, &mut trace, RateId::AgeCold(id), 1));
            } else {
                let at = place(&world, id).unwrap();
                let spoil = value(&world, AccountId::Batch(id), Asset::Heat) >= 2;
                assert!(record(
                    &mut world,
                    &mut trace,
                    RateId::AgeWarm {
                        batch: id,
                        at,
                        spoil
                    },
                    1
                ));
            }
        }
        assert!(record(
            &mut world,
            &mut trace,
            if day + 1 == days {
                RateId::Close
            } else {
                RateId::OpenDay(day as u8 + 1)
            },
            1
        ));
    }
    assert!(world.matches(&goal()));
    trace
}

#[cfg(test)]
mod tests {
    use super::*;
    fn end_day(world: &mut World, next: u8) {
        world.apply(action(RateId::EndDay, 1)).unwrap();
        assert!(
            !world.is_applicable(&action(RateId::OpenDay(next), 1)),
            "all cohort exposure must settle before the next day"
        );
        for id in BATCHES {
            let legal = candidates(world).into_iter().find(|a| matches!(a.rate(), RateId::AgeEmpty(batch) | RateId::AgeCold(batch) | RateId::AgeWarm { batch, .. } if *batch == id)).unwrap();
            world.apply(legal).unwrap();
        }
        world.apply(action(RateId::OpenDay(next), 1)).unwrap();
    }
    #[test]
    fn split_cohorts_inherit_heat_and_diverge_only_after_different_storage_exposure() {
        let mut world = initial(false, 17);
        world.apply(action(RateId::Cool, 1)).unwrap();
        end_day(&mut world, 1);
        assert_eq!(value(&world, AccountId::Batch(0), Asset::Heat), 2);
        let before = world.state_key();
        assert!(
            world
                .apply(action(
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
                RateId::Move {
                    batch: 2,
                    from: Place::Warehouse,
                    to: Place::Fridge,
                },
                1,
            ))
            .unwrap();
        world.apply(action(RateId::Cool, 1)).unwrap();
        end_day(&mut world, 2);
        assert_eq!(value(&world, AccountId::Batch(0), Asset::Spoiled), 4);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Fresh), 2);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Heat), 2);
        assert_eq!(total(&world, Asset::ColdOccupancy), 4);
    }
    #[test]
    fn backup_and_transport_draw_from_the_same_finite_fuel() {
        let mut world = initial(false, 17);
        world.apply(action(RateId::Generate, 1)).unwrap();
        assert_eq!(value(&world, AccountId::World, Asset::Energy), 6);
        assert!(!world.is_applicable(&action(RateId::Generate, 1)));
        world
            .apply(action(
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
            RateId::Deliver {
                batch: 0,
                preserved: false,
                at: Place::Warehouse
            },
            1
        )));
        let bad = action(RateId::Process(0), 1).bind(Role::Batch, AccountId::Batch(1));
        assert!(!world.is_applicable(&bad));
    }
    #[test]
    fn common_outage_scenarios_have_replayable_resource_tradeoffs() {
        let mut outcomes = std::collections::BTreeSet::new();
        for stress in [false, true] {
            for seed in 0..4 {
                let initial = initial(stress, seed);
                for policy in [
                    Policy::ProtectCold,
                    Policy::ProcessFirst,
                    Policy::ConserveFuel,
                    Policy::RebalanceCold,
                ] {
                    let trace = run(&initial, policy);
                    assert_eq!(
                        trace,
                        run(&initial, policy),
                        "same policy and outage schedule must replay identically"
                    );
                    let replay = initial.replayed(&trace).unwrap();
                    assert!(replay.matches(&goal()));
                    assert!(candidates(&replay).is_empty());
                    assert_eq!(
                        total(&initial, Asset::Fresh),
                        total(&replay, Asset::Fresh)
                            + total(&replay, Asset::Preserved)
                            + total(&replay, Asset::Spoiled)
                            + total(&replay, Asset::Disposed)
                            + total(&replay, Asset::Delivered)
                    );
                    let outcome = (
                        total(&replay, Asset::Delivered),
                        total(&replay, Asset::SpentEnergy),
                        total(&replay, Asset::MissedDemand),
                        total(&replay, Asset::Money),
                    );
                    outcomes.insert(outcome);
                }
            }
        }
        assert!(
            outcomes.len() >= 4,
            "policies and capacity must change physical outcomes: {outcomes:?}"
        );
    }
}
