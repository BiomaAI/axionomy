//! Repeated search, clinical decline and capacity-one evacuation.
use axionomy::{
    Account, EconomicView, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Trace, basket,
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Location {
    Base,
    North,
    South,
    East,
    Harbor,
    West,
}
pub const LOCATIONS: [Location; 6] = [
    Location::Base,
    Location::North,
    Location::South,
    Location::East,
    Location::Harbor,
    Location::West,
];
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum AccountId {
    World,
    Responder,
    Nature,
    Patient(u8),
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Asset {
    WorldIdentity,
    ResponderIdentity,
    NatureIdentity,
    PatientIdentity(u8),
    Active,
    Closed,
    Time,
    Horizon,
    TimeRemaining,
    PatientCount,
    Resolved,
    SavedCount,
    DeathCount,
    LeftCount,
    SavedVitality,
    At(Location),
    Site(Location),
    Surveyed(Location),
    Unsearched(Location),
    Known(u8, Location),
    Truth(u8, Location),
    Ready,
    AwaitingScan,
    Sensor,
    UsedSensor,
    ScanRound(u8),
    Signal(u8, Location),
    Hint(Location),
    Fuel,
    SpentFuel,
    Energy,
    SpentEnergy,
    MedicalKit,
    UsedMedicalKit,
    SupplyAvailable,
    SupplyUsed,
    Seat,
    Carrying(u8),
    Hidden,
    Found,
    Onboard,
    Alive,
    Saved,
    Dead,
    LeftBehind,
    Vitality,
    Harm,
    NextDecline,
    Deadline,
    Treatable,
    Stabilized,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Role {
    World,
    Responder,
    Nature,
    Patient,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Phase {
    Hidden,
    Found,
    Onboard,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Loss {
    Deterioration,
    Deadline,
    Horizon,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum RateId {
    Drive {
        from: Location,
        to: Location,
        fast: bool,
    },
    Search(Location),
    Scan,
    Report {
        round: u8,
        location: Location,
    },
    Discover {
        patient: u8,
        location: Location,
    },
    Deteriorate(u8),
    Stabilize {
        patient: u8,
        location: Location,
        restore: bool,
    },
    Load {
        patient: u8,
        location: Location,
    },
    Unload(u8),
    Lose {
        patient: u8,
        phase: Phase,
        cause: Loss,
    },
    Resupply,
    Wait,
    Close,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Triage,
    RapidNorth,
    StabilizeFirst,
}
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `rescue/incident` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "rescue/incident";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub type Proposal = Trace<Ontology>;
pub type ResponderView<'a> = EconomicView<'a, Ontology>;

pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}
pub fn now(world: &World) -> u64 {
    value(world, AccountId::World, Asset::Time)
}
fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
fn clock() -> Q<Ontology> {
    q(Role::World, Asset::Time)
}
fn identity() -> Rate<Ontology> {
    Rate::new()
        .preserve(Role::World, basket([(Asset::WorldIdentity, 1)]))
        .preserve(Role::Responder, basket([(Asset::ResponderIdentity, 1)]))
        .distinct(Role::World, Role::Responder)
        .condition("one incident event", Q::units(), C::Equal, Q::constant(1))
}
fn law() -> Rate<Ontology> {
    identity().preserve(Role::World, basket([(Asset::Active, 1)]))
}
fn work(time: u64, energy: u64) -> Rate<Ontology> {
    law()
        .preserve(Role::Responder, basket([(Asset::Ready, 1)]))
        .consume(Role::World, basket([(Asset::TimeRemaining, time)]))
        .produce(Role::World, basket([(Asset::Time, time)]))
        .consume(Role::Responder, basket([(Asset::Energy, energy)]))
        .produce(Role::Responder, basket([(Asset::SpentEnergy, energy)]))
}
fn patient(rate: Rate<Ontology>, id: u8) -> Rate<Ontology> {
    rate.preserve(Role::Patient, basket([(Asset::PatientIdentity(id), 1)]))
        .distinct(Role::Patient, Role::Responder)
        .distinct(Role::Patient, Role::World)
}
fn viable(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.preserve(Role::Patient, basket([(Asset::Alive, 1)]))
        .condition(
            "patient still has vitality",
            q(Role::Patient, Asset::Vitality),
            C::GreaterThanOrEqual,
            Q::constant(1),
        )
        .condition(
            "settle overdue deterioration first",
            clock() + Q::constant(1),
            C::LessThanOrEqual,
            q(Role::Patient, Asset::NextDecline),
        )
        .condition(
            "intervention before deadline",
            clock() + Q::constant(1),
            C::LessThanOrEqual,
            q(Role::Patient, Asset::Deadline),
        )
}
fn phase_asset(phase: Phase) -> Asset {
    match phase {
        Phase::Hidden => Asset::Hidden,
        Phase::Found => Asset::Found,
        Phase::Onboard => Asset::Onboard,
    }
}
pub fn distance(from: Location, to: Location) -> u64 {
    if from == to {
        return 0;
    }
    let roads = [
        (Location::Base, Location::North),
        (Location::Base, Location::South),
        (Location::Base, Location::Harbor),
        (Location::North, Location::East),
        (Location::East, Location::Harbor),
        (Location::South, Location::West),
        (Location::West, Location::Harbor),
    ];
    let mut pending = std::collections::VecDeque::from([(from, 0)]);
    let mut seen = std::collections::BTreeSet::from([from]);
    while let Some((at, distance)) = pending.pop_front() {
        for (a, b) in roads {
            let next = if a == at {
                b
            } else if b == at {
                a
            } else {
                continue;
            };
            if next == to {
                return distance + 1;
            }
            if seen.insert(next) {
                pending.push_back((next, distance + 1));
            }
        }
    }
    unreachable!("connected emergency road network")
}
pub fn initial(stress: bool, seed: u64) -> World {
    let count = if stress { 4 } else { 3 };
    let sites = if stress {
        &LOCATIONS[1..]
    } else {
        &LOCATIONS[1..5]
    };
    let horizon = if stress { 30 } else { 24 };
    let offset = (seed % sites.len() as u64) as usize;
    let stride = if sites.len() == 4 {
        if seed & 4 == 0 { 1 } else { 3 }
    } else {
        1 + ((seed / 5) % 4) as usize
    };
    let locations: Vec<_> = (0..count)
        .map(|id| sites[(offset + id * stride) % sites.len()])
        .collect();
    let mut world = basket([
        (Asset::WorldIdentity, 1),
        (Asset::Active, 1),
        (Asset::Horizon, horizon),
        (Asset::TimeRemaining, horizon),
        (Asset::PatientCount, count as u64),
        (Asset::Fuel, 4),
        (Asset::Energy, 6),
        (Asset::MedicalKit, 1),
        (Asset::SupplyAvailable, 1),
    ]);
    let mut responder = basket([
        (Asset::ResponderIdentity, 1),
        (Asset::At(Location::Base), 1),
        (Asset::Ready, 1),
        (Asset::Fuel, if stress { 7 } else { 9 }),
        (Asset::Energy, if stress { 10 } else { 12 }),
        (Asset::MedicalKit, 2),
        (Asset::Sensor, 2),
        (Asset::ScanRound(0), 1),
        (Asset::Seat, 1),
    ]);
    for site in sites {
        world.insert(Asset::Site(*site), Quantity::new(1));
        responder.insert(Asset::Unsearched(*site), Quantity::new(1));
    }
    let mut nature = basket([(Asset::NatureIdentity, 1)]);
    for (id, location) in locations.iter().enumerate() {
        nature.insert(Asset::Truth(id as u8, *location), Quantity::new(1));
    }
    for (round, location) in locations.iter().enumerate().take(2) {
        let report = if (seed % 4 + round as u64).is_multiple_of(4) {
            sites[(offset + count) % sites.len()]
        } else {
            *location
        };
        nature.insert(Asset::Signal(round as u8, report), Quantity::new(1));
    }
    let mut builder = EconomyBuilder::new()
        .account(AccountId::World, Account::from(world))
        .account(AccountId::Responder, Account::from(responder))
        .account(AccountId::Nature, Account::from(nature));
    for id in 0..count as u8 {
        let first_decline = [4, 6, 8, 5][id as usize] + (seed.rotate_right(id as u32 * 3) % 3);
        builder = builder.account(
            AccountId::Patient(id),
            Account::from(basket([
                (Asset::PatientIdentity(id), 1),
                (Asset::Hidden, 1),
                (Asset::Alive, 1),
                (Asset::Vitality, 3),
                (Asset::NextDecline, first_decline),
                (Asset::Deadline, first_decline + 10),
                (Asset::Treatable, 1),
            ])),
        );
        builder = builder.rate(
            RateId::Deteriorate(id),
            patient(law(), id)
                .preserve(Role::Patient, basket([(Asset::Alive, 1)]))
                .condition(
                    "clinical decline is due",
                    clock(),
                    C::GreaterThanOrEqual,
                    q(Role::Patient, Asset::NextDecline),
                )
                .consume(Role::Patient, basket([(Asset::Vitality, 1)]))
                .produce(
                    Role::Patient,
                    basket([(Asset::Harm, 1), (Asset::NextDecline, 3)]),
                ),
        );
        for location in sites.iter().copied() {
            builder = builder.rate(
                RateId::Discover {
                    patient: id,
                    location,
                },
                patient(law(), id)
                    .preserve(Role::Responder, basket([(Asset::Surveyed(location), 1)]))
                    .preserve(Role::Patient, basket([(Asset::Alive, 1)]))
                    .preserve(
                        Role::Nature,
                        basket([(Asset::NatureIdentity, 1), (Asset::Truth(id, location), 1)]),
                    )
                    .consume(Role::Patient, basket([(Asset::Hidden, 1)]))
                    .produce(
                        Role::Patient,
                        basket([(Asset::Found, 1), (Asset::At(location), 1)]),
                    )
                    .produce(Role::World, basket([(Asset::Known(id, location), 1)]))
                    .distinct(Role::Nature, Role::World)
                    .distinct(Role::Nature, Role::Responder)
                    .distinct(Role::Nature, Role::Patient),
            );
            for restore in [false, true] {
                let mut stabilize = viable(patient(work(1, 1), id))
                    .preserve(Role::Responder, basket([(Asset::At(location), 1)]))
                    .preserve(
                        Role::Patient,
                        basket([(Asset::At(location), 1), (Asset::Found, 1)]),
                    )
                    .consume(Role::Responder, basket([(Asset::MedicalKit, 1)]))
                    .produce(Role::Responder, basket([(Asset::UsedMedicalKit, 1)]))
                    .consume(Role::Patient, basket([(Asset::Treatable, 1)]))
                    .produce(
                        Role::Patient,
                        basket([
                            (Asset::Stabilized, 1),
                            (Asset::NextDecline, 4),
                            (Asset::Deadline, 4),
                        ]),
                    );
                if restore {
                    stabilize = stabilize
                        .consume(Role::Patient, basket([(Asset::Harm, 1)]))
                        .produce(Role::Patient, basket([(Asset::Vitality, 1)]));
                } else {
                    stabilize = stabilize.condition(
                        "no harm to restore",
                        q(Role::Patient, Asset::Harm),
                        C::Equal,
                        Q::constant(0),
                    );
                }
                builder = builder.rate(
                    RateId::Stabilize {
                        patient: id,
                        location,
                        restore,
                    },
                    stabilize,
                );
            }
            builder = builder.rate(
                RateId::Load {
                    patient: id,
                    location,
                },
                viable(patient(work(1, 1), id))
                    .preserve(Role::Responder, basket([(Asset::At(location), 1)]))
                    .consume(Role::Responder, basket([(Asset::Seat, 1)]))
                    .produce(Role::Responder, basket([(Asset::Carrying(id), 1)]))
                    .consume(
                        Role::Patient,
                        basket([(Asset::Found, 1), (Asset::At(location), 1)]),
                    )
                    .produce(Role::Patient, basket([(Asset::Onboard, 1)])),
            );
        }
        // Alive is consumed on evacuation, so add its requirements directly
        // instead of preserving and consuming the same token.
        builder = builder.rate(
            RateId::Unload(id),
            patient(work(1, 1), id)
                .preserve(Role::Responder, basket([(Asset::At(Location::Base), 1)]))
                .condition(
                    "patient still has vitality",
                    q(Role::Patient, Asset::Vitality),
                    C::GreaterThanOrEqual,
                    Q::constant(1),
                )
                .condition(
                    "clinical observations current",
                    clock() + Q::constant(1),
                    C::LessThanOrEqual,
                    q(Role::Patient, Asset::NextDecline),
                )
                .condition(
                    "arrival before deadline",
                    clock() + Q::constant(1),
                    C::LessThanOrEqual,
                    q(Role::Patient, Asset::Deadline),
                )
                .consume(Role::Responder, basket([(Asset::Carrying(id), 1)]))
                .produce(Role::Responder, basket([(Asset::Seat, 1)]))
                .consume(
                    Role::Patient,
                    basket([(Asset::Alive, 1), (Asset::Onboard, 1)]),
                )
                .produce(
                    Role::Patient,
                    basket([(Asset::Saved, 1), (Asset::Resolved, 1)]),
                )
                .produce(
                    Role::World,
                    basket([(Asset::SavedCount, 1), (Asset::Resolved, 1)]),
                )
                .produce_computed(
                    Role::World,
                    Asset::SavedVitality,
                    q(Role::Patient, Asset::Vitality),
                ),
        );
        for phase in [Phase::Hidden, Phase::Found, Phase::Onboard] {
            for cause in [Loss::Deterioration, Loss::Deadline, Loss::Horizon] {
                let mut lost = patient(law(), id)
                    .consume(
                        Role::Patient,
                        basket([(Asset::Alive, 1), (phase_asset(phase), 1)]),
                    )
                    .produce(
                        Role::Patient,
                        basket([
                            (
                                if cause == Loss::Horizon {
                                    Asset::LeftBehind
                                } else {
                                    Asset::Dead
                                },
                                1,
                            ),
                            (Asset::Resolved, 1),
                        ]),
                    )
                    .produce(
                        Role::World,
                        basket([
                            (Asset::Resolved, 1),
                            (
                                if cause == Loss::Horizon {
                                    Asset::LeftCount
                                } else {
                                    Asset::DeathCount
                                },
                                1,
                            ),
                        ]),
                    );
                lost = match cause {
                    Loss::Deterioration => lost.condition(
                        "no vitality remains",
                        q(Role::Patient, Asset::Vitality),
                        C::Equal,
                        Q::constant(0),
                    ),
                    Loss::Deadline => lost.condition(
                        "deadline elapsed",
                        clock(),
                        C::GreaterThanOrEqual,
                        q(Role::Patient, Asset::Deadline) + Q::constant(1),
                    ),
                    Loss::Horizon => lost
                        .condition(
                            "incident horizon reached",
                            clock(),
                            C::GreaterThanOrEqual,
                            q(Role::World, Asset::Horizon),
                        )
                        .condition(
                            "patient still alive",
                            q(Role::Patient, Asset::Vitality),
                            C::GreaterThanOrEqual,
                            Q::constant(1),
                        )
                        .condition(
                            "not past deadline",
                            clock(),
                            C::LessThanOrEqual,
                            q(Role::Patient, Asset::Deadline),
                        ),
                };
                if phase == Phase::Onboard {
                    lost = lost
                        .consume(Role::Responder, basket([(Asset::Carrying(id), 1)]))
                        .produce(Role::Responder, basket([(Asset::Seat, 1)]));
                }
                builder = builder.rate(
                    RateId::Lose {
                        patient: id,
                        phase,
                        cause,
                    },
                    lost,
                );
            }
        }
    }
    for from in LOCATIONS {
        for to in LOCATIONS.into_iter().filter(|to| *to != from) {
            if (from != Location::Base && !sites.contains(&from))
                || (to != Location::Base && !sites.contains(&to))
            {
                continue;
            }
            let distance = distance(from, to);
            for fast in [false, true] {
                let fuel = distance * if fast { 2 } else { 1 };
                builder = builder.rate(
                    RateId::Drive { from, to, fast },
                    work(distance * if fast { 1 } else { 2 }, 0)
                        .consume(
                            Role::Responder,
                            basket([(Asset::At(from), 1), (Asset::Fuel, fuel)]),
                        )
                        .produce(
                            Role::Responder,
                            basket([(Asset::At(to), 1), (Asset::SpentFuel, fuel)]),
                        ),
                );
            }
        }
    }
    for location in sites.iter().copied() {
        builder = builder.rate(
            RateId::Search(location),
            work(1, 1)
                .preserve(Role::Responder, basket([(Asset::At(location), 1)]))
                .consume(Role::Responder, basket([(Asset::Unsearched(location), 1)]))
                .produce(Role::Responder, basket([(Asset::Surveyed(location), 1)])),
        );
        for round in 0..2 {
            builder = builder.rate(
                RateId::Report { round, location },
                law()
                    .preserve(
                        Role::Nature,
                        basket([
                            (Asset::NatureIdentity, 1),
                            (Asset::Signal(round, location), 1),
                        ]),
                    )
                    .consume(
                        Role::Responder,
                        basket([(Asset::AwaitingScan, 1), (Asset::ScanRound(round), 1)]),
                    )
                    .produce(
                        Role::Responder,
                        basket([
                            (Asset::Ready, 1),
                            (Asset::ScanRound(round + 1), 1),
                            (Asset::Hint(location), 1),
                        ]),
                    )
                    .distinct(Role::Nature, Role::World)
                    .distinct(Role::Nature, Role::Responder),
            );
        }
    }
    builder = builder.rate(
        RateId::Scan,
        law()
            .consume(
                Role::Responder,
                basket([(Asset::Ready, 1), (Asset::Sensor, 1), (Asset::Energy, 1)]),
            )
            .produce(
                Role::Responder,
                basket([
                    (Asset::AwaitingScan, 1),
                    (Asset::UsedSensor, 1),
                    (Asset::SpentEnergy, 1),
                ]),
            )
            .consume(Role::World, basket([(Asset::TimeRemaining, 1)]))
            .produce(Role::World, basket([(Asset::Time, 1)])),
    );
    builder = builder.rate(
        RateId::Resupply,
        work(2, 0)
            .preserve(Role::Responder, basket([(Asset::At(Location::Base), 1)]))
            .consume(
                Role::World,
                basket([
                    (Asset::SupplyAvailable, 1),
                    (Asset::Fuel, 4),
                    (Asset::Energy, 6),
                    (Asset::MedicalKit, 1),
                ]),
            )
            .produce(Role::World, basket([(Asset::SupplyUsed, 1)]))
            .produce(
                Role::Responder,
                basket([(Asset::Fuel, 4), (Asset::Energy, 6), (Asset::MedicalKit, 1)]),
            ),
    );
    builder = builder.rate(RateId::Wait, work(1, 0));
    builder = builder.rate(
        RateId::Close,
        identity()
            .condition(
                "all patients accounted for",
                q(Role::World, Asset::Resolved),
                C::Equal,
                q(Role::World, Asset::PatientCount),
            )
            .consume(Role::World, basket([(Asset::Active, 1)]))
            .produce(Role::World, basket([(Asset::Closed, 1)])),
    );
    let mut seats = LinearInvariant::new("one evacuation seat").weight(Asset::Seat, 1);
    for id in 0..count as u8 {
        seats = seats.weight(Asset::Carrying(id), 1);
    }
    let mut positions = LinearInvariant::new("responder activity");
    // Patient location facts are introduced on discovery, so encode only the
    // responder's conserved Ready/AwaitingScan state, not all At facts globally.
    positions = positions
        .weight(Asset::Ready, 1)
        .weight(Asset::AwaitingScan, 1);
    let mut sectors = LinearInvariant::new("one survey per sector");
    for location in sites {
        sectors = sectors
            .weight(Asset::Unsearched(*location), 1)
            .weight(Asset::Surveyed(*location), 1);
    }
    builder
        .invariant(seats)
        .invariant(positions)
        .invariant(sectors)
        .invariant(
            LinearInvariant::new("patient outcomes")
                .weight(Asset::Alive, 1)
                .weight(Asset::Saved, 1)
                .weight(Asset::Dead, 1)
                .weight(Asset::LeftBehind, 1),
        )
        .invariant(
            LinearInvariant::new("clinical vitality")
                .weight(Asset::Vitality, 1)
                .weight(Asset::Harm, 1),
        )
        .invariant(
            LinearInvariant::new("finite medicine")
                .weight(Asset::MedicalKit, 1)
                .weight(Asset::UsedMedicalKit, 1),
        )
        .invariant(
            LinearInvariant::new("finite fuel")
                .weight(Asset::Fuel, 1)
                .weight(Asset::SpentFuel, 1),
        )
        .invariant(
            LinearInvariant::new("finite effort")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("sensor charges")
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant(
            LinearInvariant::new("resupply opportunity")
                .weight(Asset::SupplyAvailable, 1)
                .weight(Asset::SupplyUsed, 1),
        )
        .invariant(
            LinearInvariant::new("bounded incident time")
                .weight(Asset::Time, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new("incident lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Closed, 1),
        )
        .build()
        .expect("rescue incident conserves capacities and resources")
}

pub fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1))
        .bind(Role::World, AccountId::World)
        .bind(Role::Responder, AccountId::Responder);
    match rate {
        RateId::Discover { patient, .. } => exchange
            .bind(Role::Patient, AccountId::Patient(patient))
            .bind(Role::Nature, AccountId::Nature),
        RateId::Report { .. } => exchange.bind(Role::Nature, AccountId::Nature),
        RateId::Deteriorate(patient)
        | RateId::Stabilize { patient, .. }
        | RateId::Load { patient, .. }
        | RateId::Unload(patient)
        | RateId::Lose { patient, .. } => exchange.bind(Role::Patient, AccountId::Patient(patient)),
        _ => exchange,
    }
}
pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::World, basket([(Asset::Closed, 1)]))
}
pub fn responder_view(world: &World) -> ResponderView<'_> {
    let mut visible = vec![AccountId::Responder, AccountId::World];
    for id in 0..value(world, AccountId::World, Asset::PatientCount) as u8 {
        if LOCATIONS
            .into_iter()
            .any(|location| value(world, AccountId::World, Asset::Known(id, location)) > 0)
        {
            visible.push(AccountId::Patient(id));
        }
    }
    world.view(visible)
}
pub fn candidates(world: &World) -> Vec<Action> {
    let mut rates: Vec<_> = world.rate_ids().copied().collect();
    rates.sort();
    world.applicable(rates.into_iter().map(action))
}
/// Environmental responses may inspect hidden accounts. The decision policy
/// below receives only the responder's explicit economic view.
pub fn nature_response(world: &World) -> Option<Action> {
    let count = value(world, AccountId::World, Asset::PatientCount) as u8;
    for id in 0..count {
        for cause in [Loss::Deterioration, Loss::Deadline, Loss::Horizon] {
            for phase in [Phase::Hidden, Phase::Found, Phase::Onboard] {
                let exchange = action(RateId::Lose {
                    patient: id,
                    phase,
                    cause,
                });
                if world.is_applicable(&exchange) {
                    return Some(exchange);
                }
            }
        }
        let exchange = action(RateId::Deteriorate(id));
        if world.is_applicable(&exchange) {
            return Some(exchange);
        }
    }
    for id in 0..count {
        for location in LOCATIONS {
            let exchange = action(RateId::Discover {
                patient: id,
                location,
            });
            if world.is_applicable(&exchange) {
                return Some(exchange);
            }
        }
    }
    for round in 0..2 {
        for location in LOCATIONS {
            let exchange = action(RateId::Report { round, location });
            if world.is_applicable(&exchange) {
                return Some(exchange);
            }
        }
    }
    None
}
pub fn decision(view: &ResponderView<'_>, policy: Policy) -> Option<Action> {
    let val = |account, asset| view.balance(&account, &asset).map_or(0, Quantity::to_u64);
    let count = val(AccountId::World, Asset::PatientCount) as u8;
    if val(AccountId::World, Asset::Resolved) == u64::from(count) {
        return Some(action(RateId::Close));
    }
    let from = LOCATIONS
        .into_iter()
        .find(|location| val(AccountId::Responder, Asset::At(*location)) > 0)?;
    let now = val(AccountId::World, Asset::Time);
    let drive = |to, urgent| {
        let distance = distance(from, to);
        let fast = (urgent || policy == Policy::RapidNorth)
            && val(AccountId::Responder, Asset::Fuel) >= distance * 2;
        action(RateId::Drive { from, to, fast })
    };
    if let Some(id) = (0..count).find(|id| val(AccountId::Responder, Asset::Carrying(*id)) > 0) {
        if from == Location::Base {
            return Some(action(RateId::Unload(id)));
        }
        let urgent = val(AccountId::Patient(id), Asset::Vitality) <= 1
            || val(AccountId::Patient(id), Asset::NextDecline)
                <= now + distance(from, Location::Base) * 2 + 1;
        return Some(drive(Location::Base, urgent));
    }
    let mut found: Vec<_> = (0..count)
        .filter(|id| {
            val(AccountId::Patient(*id), Asset::Found) > 0
                && val(AccountId::Patient(*id), Asset::Alive) > 0
        })
        .collect();
    found.sort_by_key(|id| {
        (
            val(AccountId::Patient(*id), Asset::Vitality),
            val(AccountId::Patient(*id), Asset::NextDecline),
        )
    });
    for id in found.iter().copied() {
        if val(AccountId::Patient(id), Asset::At(from)) == 0 {
            continue;
        }
        let vitality = val(AccountId::Patient(id), Asset::Vitality);
        let arrival = now
            + 2
            + distance(from, Location::Base) * if policy == Policy::RapidNorth { 1 } else { 2 };
        let decline = val(AccountId::Patient(id), Asset::NextDecline);
        let projected_loss = if arrival >= decline {
            (arrival - decline) / 3 + 1
        } else {
            0
        };
        if policy != Policy::RapidNorth
            && val(AccountId::Responder, Asset::MedicalKit) > 0
            && val(AccountId::Patient(id), Asset::Treatable) > 0
            && (policy == Policy::StabilizeFirst || vitality <= projected_loss)
        {
            return Some(action(RateId::Stabilize {
                patient: id,
                location: from,
                restore: val(AccountId::Patient(id), Asset::Harm) > 0,
            }));
        }
        return Some(action(RateId::Load {
            patient: id,
            location: from,
        }));
    }
    if val(AccountId::Responder, Asset::Unsearched(from)) > 0 {
        return Some(action(RateId::Search(from)));
    }
    if from == Location::Base {
        if val(AccountId::World, Asset::SupplyAvailable) > 0
            && (val(AccountId::Responder, Asset::Fuel) < 4
                || val(AccountId::Responder, Asset::Energy) < 4
                || val(AccountId::Responder, Asset::MedicalKit) == 0)
        {
            return Some(action(RateId::Resupply));
        }
        if policy != Policy::RapidNorth
            && val(AccountId::Responder, Asset::Sensor) > 0
            && val(AccountId::Responder, Asset::UsedSensor)
                <= val(AccountId::World, Asset::SavedCount)
        {
            return Some(action(RateId::Scan));
        }
    }
    if let Some(id) = found.first()
        && let Some(to) = LOCATIONS
            .into_iter()
            .find(|location| val(AccountId::Patient(*id), Asset::At(*location)) > 0)
    {
        return Some(drive(to, true));
    }
    let mut destinations: Vec<_> = LOCATIONS
        .into_iter()
        .filter(|location| val(AccountId::Responder, Asset::Unsearched(*location)) > 0)
        .collect();
    destinations.sort_by_key(|location| {
        (
            if policy == Policy::RapidNorth {
                u64::from(*location != Location::North)
            } else {
                u64::from(val(AccountId::Responder, Asset::Hint(*location)) == 0)
            },
            distance(from, *location),
            *location,
        )
    });
    if let Some(to) = destinations.first().copied() {
        let needed = distance(from, to) + distance(to, Location::Base);
        if from != Location::Base
            && (val(AccountId::Responder, Asset::Fuel) < needed
                || val(AccountId::Responder, Asset::Energy) < 3)
        {
            return Some(drive(Location::Base, false));
        }
        return Some(drive(to, val(AccountId::Responder, Asset::Hint(to)) > 0));
    }
    Some(action(RateId::Wait))
}
pub fn run(initial: &World, policy: Policy) -> Proposal {
    let mut world = initial.clone();
    let mut trace = Trace::new();
    for _ in 0..250 {
        if world.matches(&goal()) {
            return trace;
        }
        let exchange = nature_response(&world)
            .or_else(|| {
                decision(&responder_view(&world), policy)
                    .filter(|proposal| world.is_applicable(proposal))
            })
            .unwrap_or_else(|| action(RateId::Wait));
        world
            .apply(exchange.clone())
            .expect("policy/environment proposal remains feasible");
        trace.push(exchange);
    }
    panic!("incident did not reach its finite terminal state")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle(world: &mut World) {
        while let Some(exchange) = nature_response(world) {
            world.apply(exchange).unwrap();
        }
    }
    #[test]
    fn policies_repeatedly_search_and_replay_across_common_scenarios() {
        let mut results = std::collections::BTreeSet::new();
        let mut saved_multiple = false;
        let mut treated = false;
        let mut resupplied = false;
        for stress in [false, true] {
            for seed in [0, 3, 17, u64::MAX] {
                for policy in [Policy::Triage, Policy::RapidNorth, Policy::StabilizeFirst] {
                    let source = initial(stress, seed);
                    let before = source.state_key();
                    let trace = run(&source, policy);
                    assert_eq!(trace.exchanges(), run(&source, policy).exchanges());
                    let mut replay = source.clone();
                    for exchange in trace.exchanges() {
                        replay.apply(exchange.clone()).unwrap();
                        assert_eq!(
                            LOCATIONS
                                .into_iter()
                                .map(|location| value(
                                    &replay,
                                    AccountId::Responder,
                                    Asset::At(location)
                                ))
                                .sum::<u64>(),
                            1
                        );
                        let occupied: u64 =
                            (0..value(&replay, AccountId::World, Asset::PatientCount) as u8)
                                .map(|id| value(&replay, AccountId::Responder, Asset::Carrying(id)))
                                .sum();
                        assert_eq!(
                            occupied + value(&replay, AccountId::Responder, Asset::Seat),
                            1
                        );
                    }
                    assert!(replay.matches(&goal()));
                    assert_eq!(source.state_key(), before);
                    assert_eq!(
                        value(&replay, AccountId::World, Asset::Resolved),
                        value(&source, AccountId::World, Asset::PatientCount)
                    );
                    saved_multiple |= value(&replay, AccountId::World, Asset::SavedCount) >= 2;
                    treated |= trace
                        .exchanges()
                        .iter()
                        .any(|action| matches!(action.rate(), RateId::Stabilize { .. }));
                    resupplied |= trace
                        .exchanges()
                        .iter()
                        .any(|action| action.rate() == &RateId::Resupply);
                    results.insert((
                        value(&replay, AccountId::World, Asset::SavedCount),
                        value(&replay, AccountId::World, Asset::DeathCount),
                        value(&replay, AccountId::Responder, Asset::SpentFuel),
                    ));
                    if seed == 17 {
                        eprintln!(
                            "{stress:?} {policy:?}: saved {} vitality {} dead {} fuel {} time {}",
                            value(&replay, AccountId::World, Asset::SavedCount),
                            value(&replay, AccountId::World, Asset::SavedVitality),
                            value(&replay, AccountId::World, Asset::DeathCount),
                            value(&replay, AccountId::Responder, Asset::SpentFuel),
                            now(&replay)
                        );
                    }
                }
            }
        }
        assert!(saved_multiple && treated && resupplied);
        assert!(results.len() >= 4);
    }
    #[test]
    fn decisions_cannot_read_undiscovered_patients_or_nature() {
        let left = initial(false, 0);
        let right = initial(false, 17);
        assert_ne!(left.state_key(), right.state_key());
        assert_eq!(
            responder_view(&left).observation_key(),
            responder_view(&right).observation_key()
        );
        for policy in [Policy::Triage, Policy::RapidNorth, Policy::StabilizeFirst] {
            assert_eq!(
                decision(&responder_view(&left), policy),
                decision(&responder_view(&right), policy)
            );
        }
        assert!(responder_view(&left).account(&AccountId::Nature).is_none());
        assert!(
            responder_view(&left)
                .account(&AccountId::Patient(0))
                .is_none()
        );
    }
    #[test]
    fn clinical_updates_and_treatment_are_authoritative_and_finite() {
        let mut world = initial(false, 0);
        world
            .apply(action(RateId::Drive {
                from: Location::Base,
                to: Location::North,
                fast: true,
            }))
            .unwrap();
        world
            .apply(action(RateId::Search(Location::North)))
            .unwrap();
        settle(&mut world);
        assert!(
            responder_view(&world)
                .account(&AccountId::Patient(0))
                .is_some()
        );
        while now(&world) < 4 {
            world.apply(action(RateId::Wait)).unwrap();
        }
        let load = action(RateId::Load {
            patient: 0,
            location: Location::North,
        });
        assert!(!world.is_applicable(&load));
        settle(&mut world);
        assert_eq!(value(&world, AccountId::Patient(0), Asset::Vitality), 2);
        let treatment = action(RateId::Stabilize {
            patient: 0,
            location: Location::North,
            restore: true,
        });
        world.apply(treatment.clone()).unwrap();
        assert_eq!(value(&world, AccountId::Patient(0), Asset::Vitality), 3);
        assert_eq!(value(&world, AccountId::Patient(0), Asset::NextDecline), 11);
        assert!(!world.is_applicable(&treatment));
        world.apply(load.clone()).unwrap();
        assert!(!world.is_applicable(&load));
        assert_eq!(value(&world, AccountId::Responder, Asset::Seat), 0);
        while now(&world) < 21 {
            world.apply(action(RateId::Wait)).unwrap();
            settle(&mut world);
        }
        assert_eq!(value(&world, AccountId::Patient(0), Asset::Dead), 1);
        assert_eq!(value(&world, AccountId::Responder, Asset::Seat), 1);
        assert!(!world.is_applicable(&action(RateId::Unload(0))));
    }
}
