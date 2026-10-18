//! Independent teammates, private observations and timed cooperative extraction.
use axionomy::{
    Account, EconomicView, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Trace, basket,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Agent {
    Scout,
    Medic,
}
pub const AGENTS: [Agent; 2] = [Agent::Scout, Agent::Medic];
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Location {
    Base,
    North,
    East,
    South,
    West,
}
pub const LOCATIONS: [Location; 5] = [
    Location::Base,
    Location::North,
    Location::East,
    Location::South,
    Location::West,
];
pub const ROADS: [(Location, Location); 6] = [
    (Location::Base, Location::North),
    (Location::Base, Location::South),
    (Location::North, Location::East),
    (Location::East, Location::South),
    (Location::South, Location::West),
    (Location::West, Location::North),
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccountId {
    World,
    Nature,
    Agent(Agent),
    Objective(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Asset {
    WorldIdentity,
    NatureIdentity,
    AgentIdentity(Agent),
    ObjectiveIdentity(u8),
    Active,
    Closed,
    Time,
    TimeRemaining,
    Horizon,
    Count,
    Resolved,
    Completed,
    FailedCount,
    SavedVitality,
    Permit(Agent),
    UsedPermit(Agent),
    EnvironmentReady,
    WeatherDue(bool),
    HazardSite(Location),
    WeatherOpen(Location),
    WeatherBlocked(Location),
    At(Location),
    Travelling(Location),
    TeamAt(Agent, Location),
    TeamTravelling(Agent),
    Ready,
    ReadyAt,
    Collecting(u8),
    Extracting(u8),
    AwaitingReport,
    Energy,
    SpentEnergy,
    MedicalKit,
    UsedMedicalKit,
    Health,
    Harm,
    Radio,
    UsedRadio,
    Sensor,
    UsedSensor,
    ScanRound(u8),
    Signal(u8, Location),
    Hint(u8, Location),
    Unsearched(Location),
    Surveyed(Location),
    Known(u8, Location),
    Shared(u8, Location),
    Truth(u8, Location),
    Deadline,
    Hidden,
    Located,
    Working,
    Carried,
    Complete,
    Failed,
    Stable,
    Unstable,
    ClinicalDue(u8),
    ClinicalSettled(u8),
    NextClinical,
    ClinicalCount,
    Cargo(u8),
    Seat,
    ReservedSeat(u8),
    Supply,
    UsedSupply,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    World,
    Scout,
    Medic,
    Nature,
    Objective,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    Hidden,
    Located,
    Carried,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RateId {
    Depart {
        agent: Agent,
        from: Location,
        to: Location,
    },
    Arrive {
        agent: Agent,
        to: Location,
        hazardous: bool,
    },
    Survey(Location),
    Discover {
        objective: u8,
        location: Location,
    },
    Scan,
    Report {
        round: u8,
        location: Location,
    },
    Share {
        objective: u8,
        location: Location,
        remote: bool,
    },
    Collect {
        objective: u8,
        location: Location,
    },
    FinishCollect(u8),
    TreatPatient {
        objective: u8,
        location: Location,
        restore: bool,
    },
    Extract {
        objective: u8,
        location: Location,
    },
    FinishExtract(u8),
    Deliver(u8),
    TreatAgent {
        target: Agent,
        location: Location,
    },
    Resupply(Agent),
    Weather {
        location: Location,
        opening: bool,
    },
    Clinical {
        objective: u8,
        day: u8,
        active: bool,
    },
    FinishClinical(u8),
    Fail {
        objective: u8,
        phase: Phase,
        dead: bool,
    },
    Tick(u8),
    Close,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    RadioCoordination,
    Rendezvous,
    NorthFirst,
}
pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Proposal = Trace<RateId, Role, AccountId>;
pub type AgentView<'a> = EconomicView<'a, AccountId, Asset, RateId, Role>;
pub fn patient(id: u8) -> bool {
    id.is_multiple_of(2)
}
pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).get()
}
pub fn now(world: &World) -> u64 {
    value(world, AccountId::World, Asset::Time)
}
fn role(agent: Agent) -> Role {
    if agent == Agent::Scout {
        Role::Scout
    } else {
        Role::Medic
    }
}
fn q(role: Role, asset: Asset) -> Q<Role, Asset> {
    Q::balance(role, asset)
}
fn clock() -> Q<Role, Asset> {
    q(Role::World, Asset::Time)
}
fn identity() -> Rate<Role, Asset> {
    Rate::new()
        .preserve(Role::World, basket([(Asset::WorldIdentity, 1)]))
        .preserve(
            Role::Scout,
            basket([(Asset::AgentIdentity(Agent::Scout), 1)]),
        )
        .preserve(
            Role::Medic,
            basket([(Asset::AgentIdentity(Agent::Medic), 1)]),
        )
        .distinct(Role::World, Role::Scout)
        .distinct(Role::World, Role::Medic)
        .distinct(Role::Scout, Role::Medic)
        .condition("one mission event", Q::units(), C::Equal, Q::constant(1))
}
fn event() -> Rate<Role, Asset> {
    identity().preserve(Role::World, basket([(Asset::Active, 1)]))
}
fn law() -> Rate<Role, Asset> {
    event()
        .preserve(Role::World, basket([(Asset::EnvironmentReady, 1)]))
        .condition(
            "settle scheduled clinical events first",
            clock() + Q::constant(1),
            C::LessThanOrEqual,
            q(Role::World, Asset::NextClinical),
        )
}
fn permit(rate: Rate<Role, Asset>, agent: Agent, energy: u64) -> Rate<Role, Asset> {
    rate.consume(Role::World, basket([(Asset::Permit(agent), 1)]))
        .produce(Role::World, basket([(Asset::UsedPermit(agent), 1)]))
        .consume(role(agent), basket([(Asset::Energy, energy)]))
        .produce(role(agent), basket([(Asset::SpentEnergy, energy)]))
}
fn work(agent: Agent, energy: u64) -> Rate<Role, Asset> {
    permit(law(), agent, energy).preserve(role(agent), basket([(Asset::Ready, 1)]))
}
fn objective(rate: Rate<Role, Asset>, id: u8) -> Rate<Role, Asset> {
    rate.preserve(Role::Objective, basket([(Asset::ObjectiveIdentity(id), 1)]))
        .distinct(Role::Objective, Role::World)
        .distinct(Role::Objective, Role::Scout)
        .distinct(Role::Objective, Role::Medic)
}
fn nature(rate: Rate<Role, Asset>) -> Rate<Role, Asset> {
    rate.preserve(Role::Nature, basket([(Asset::NatureIdentity, 1)]))
        .distinct(Role::Nature, Role::World)
        .distinct(Role::Nature, Role::Scout)
        .distinct(Role::Nature, Role::Medic)
}
fn before_horizon(rate: Rate<Role, Asset>, duration: Q<Role, Asset>) -> Rate<Role, Asset> {
    rate.condition(
        "activity ends within mission horizon",
        clock() + duration,
        C::LessThanOrEqual,
        q(Role::World, Asset::Horizon),
    )
}
fn phase_asset(phase: Phase) -> Asset {
    match phase {
        Phase::Hidden => Asset::Hidden,
        Phase::Located => Asset::Located,
        Phase::Carried => Asset::Carried,
    }
}
pub fn distance(from: Location, to: Location) -> u64 {
    if from == to {
        return 0;
    }
    let mut queue = std::collections::VecDeque::from([(from, 0)]);
    let mut seen = std::collections::BTreeSet::from([from]);
    while let Some((at, cost)) = queue.pop_front() {
        for (a, b) in ROADS {
            let next = if a == at {
                b
            } else if b == at {
                a
            } else {
                continue;
            };
            if next == to {
                return cost + 1;
            }
            if seen.insert(next) {
                queue.push_back((next, cost + 1));
            }
        }
    }
    unreachable!("connected mission roads")
}
pub fn initial(stress: bool, seed: u64) -> World {
    let count: u8 = if stress { 3 } else { 2 };
    let horizon: u64 = if stress { 32 } else { 28 };
    let sites = &LOCATIONS[1..];
    let offset = (seed % 4) as usize;
    let stride = if seed & 4 == 0 { 1 } else { 3 };
    let locations: Vec<_> = (0..count as usize)
        .map(|id| sites[(offset + id * stride) % 4])
        .collect();
    let hazard = locations[((seed / 4) % u64::from(count)) as usize];
    let mut world = basket([
        (Asset::WorldIdentity, 1),
        (Asset::Active, 1),
        (Asset::EnvironmentReady, 1),
        (Asset::NextClinical, 8),
        (Asset::Horizon, horizon),
        (Asset::TimeRemaining, horizon),
        (Asset::Count, count.into()),
        (Asset::Energy, 8),
        (Asset::MedicalKit, 2),
        (Asset::Supply, 2),
    ]);
    for location in LOCATIONS {
        world.insert(Asset::WeatherOpen(location), Quantity::new(1));
    }
    for agent in AGENTS {
        world.insert(Asset::Permit(agent), Quantity::new(1));
        world.insert(Asset::TeamAt(agent, Location::Base), Quantity::new(1));
    }
    let mut hidden = basket([(Asset::NatureIdentity, 1), (Asset::HazardSite(hazard), 1)]);
    for (id, location) in locations.iter().copied().enumerate() {
        hidden.insert(Asset::Truth(id as u8, location), Quantity::new(1));
    }
    for round in 0..2 {
        hidden.insert(
            Asset::Signal(
                round,
                if (seed % 4 + u64::from(round)).is_multiple_of(4) {
                    sites[(offset + count as usize) % 4]
                } else {
                    locations[usize::from(round)]
                },
            ),
            Quantity::new(1),
        );
    }
    let mut builder = EconomyBuilder::new()
        .account(AccountId::World, Account::from(world))
        .account(AccountId::Nature, Account::from(hidden));
    for agent in AGENTS {
        let mut stock = basket([
            (Asset::AgentIdentity(agent), 1),
            (Asset::At(Location::Base), 1),
            (Asset::Ready, 1),
            (Asset::Health, 2),
            (Asset::Energy, if stress { 12 } else { 16 }),
        ]);
        if agent == Agent::Scout {
            stock.insert(Asset::Radio, Quantity::new(if stress { 1 } else { 2 }));
            stock.insert(Asset::Sensor, Quantity::new(2));
            stock.insert(Asset::ScanRound(0), Quantity::new(1));
            for location in sites {
                stock.insert(Asset::Unsearched(*location), Quantity::new(1));
            }
        } else {
            stock.insert(Asset::MedicalKit, Quantity::new(2));
            stock.insert(Asset::Seat, Quantity::new(1));
        }
        builder = builder.account(AccountId::Agent(agent), Account::from(stock));
        for from in LOCATIONS {
            for to in LOCATIONS.into_iter().filter(|to| *to != from) {
                let duration = Q::constant(distance(from, to)) + q(role(agent), Asset::Harm);
                builder = builder.rate(
                    RateId::Depart { agent, from, to },
                    before_horizon(permit(law(), agent, 0), duration.clone())
                        .preserve(Role::World, basket([(Asset::WeatherOpen(to), 1)]))
                        .consume(
                            role(agent),
                            basket([(Asset::Ready, 1), (Asset::At(from), 1)]),
                        )
                        .consume(Role::World, basket([(Asset::TeamAt(agent, from), 1)]))
                        .produce(Role::World, basket([(Asset::TeamTravelling(agent), 1)]))
                        .produce(role(agent), basket([(Asset::Travelling(to), 1)]))
                        .consume_computed(role(agent), Asset::Energy, duration.clone())
                        .produce_computed(role(agent), Asset::SpentEnergy, duration.clone())
                        .produce_computed(role(agent), Asset::ReadyAt, clock() + duration),
                );
            }
        }
        for to in LOCATIONS {
            for hazardous in [false, true] {
                let mut arrive = law()
                    .preserve(
                        Role::World,
                        basket([(
                            if hazardous {
                                Asset::WeatherBlocked(to)
                            } else {
                                Asset::WeatherOpen(to)
                            },
                            1,
                        )]),
                    )
                    .condition(
                        "travel time elapsed",
                        clock(),
                        C::GreaterThanOrEqual,
                        q(role(agent), Asset::ReadyAt),
                    )
                    .consume(role(agent), basket([(Asset::Travelling(to), 1)]))
                    .consume(Role::World, basket([(Asset::TeamTravelling(agent), 1)]))
                    .produce(Role::World, basket([(Asset::TeamAt(agent, to), 1)]))
                    .produce(role(agent), basket([(Asset::Ready, 1), (Asset::At(to), 1)]))
                    .consume_computed(role(agent), Asset::ReadyAt, q(role(agent), Asset::ReadyAt));
                if hazardous {
                    arrive = arrive
                        .consume(role(agent), basket([(Asset::Health, 1)]))
                        .produce(role(agent), basket([(Asset::Harm, 1)]));
                }
                builder = builder.rate(
                    RateId::Arrive {
                        agent,
                        to,
                        hazardous,
                    },
                    arrive,
                );
            }
        }
        builder = builder.rate(
            RateId::Resupply(agent),
            work(agent, 0)
                .preserve(role(agent), basket([(Asset::At(Location::Base), 1)]))
                .consume(
                    Role::World,
                    basket([
                        (Asset::Energy, 4),
                        (Asset::MedicalKit, u64::from(agent == Agent::Medic)),
                        (Asset::Supply, 1),
                    ]),
                )
                .produce(Role::World, basket([(Asset::UsedSupply, 1)]))
                .produce(role(agent), basket([(Asset::Energy, 4)]))
                .produce(
                    Role::Medic,
                    basket([(Asset::MedicalKit, u64::from(agent == Agent::Medic))]),
                ),
        );
        for location in LOCATIONS {
            let mut treatment = work(Agent::Medic, 1)
                .preserve(Role::Medic, basket([(Asset::At(location), 1)]))
                .consume(Role::Medic, basket([(Asset::MedicalKit, 1)]))
                .produce(Role::Medic, basket([(Asset::UsedMedicalKit, 1)]))
                .consume(role(agent), basket([(Asset::Harm, 1)]))
                .produce(role(agent), basket([(Asset::Health, 1)]));
            if agent != Agent::Medic {
                treatment = treatment.preserve(
                    Role::Scout,
                    basket([(Asset::At(location), 1), (Asset::Ready, 1)]),
                );
            }
            builder = builder.rate(
                RateId::TreatAgent {
                    target: agent,
                    location,
                },
                treatment,
            );
        }
    }
    for id in 0..count {
        let mut stock = basket([
            (Asset::ObjectiveIdentity(id), 1),
            (Asset::Hidden, 1),
            (Asset::Deadline, horizon - 2),
        ]);
        if patient(id) {
            stock.insert(Asset::Health, Quantity::new(3));
            stock.insert(Asset::Unstable, Quantity::new(1));
            for day in [8, 16, 24] {
                stock.insert(Asset::ClinicalDue(day), Quantity::new(1));
            }
        }
        builder = builder.account(AccountId::Objective(id), Account::from(stock));
        for location in sites.iter().copied() {
            builder = builder.rate(
                RateId::Discover {
                    objective: id,
                    location,
                },
                nature(objective(law(), id))
                    .preserve(Role::Scout, basket([(Asset::Surveyed(location), 1)]))
                    .preserve(Role::Nature, basket([(Asset::Truth(id, location), 1)]))
                    .consume(Role::Objective, basket([(Asset::Hidden, 1)]))
                    .produce(Role::Objective, basket([(Asset::Located, 1)]))
                    .produce(Role::Scout, basket([(Asset::Known(id, location), 1)])),
            );
            for remote in [false, true] {
                let mut share = work(Agent::Scout, 0)
                    .preserve(Role::Scout, basket([(Asset::Known(id, location), 1)]))
                    .condition(
                        "recipient does not already know",
                        q(Role::Medic, Asset::Known(id, location)),
                        C::Equal,
                        Q::constant(0),
                    )
                    .produce(Role::Scout, basket([(Asset::Shared(id, location), 1)]))
                    .produce(Role::Medic, basket([(Asset::Known(id, location), 1)]));
                if remote {
                    share = share
                        .consume(Role::Scout, basket([(Asset::Radio, 1)]))
                        .produce(Role::Scout, basket([(Asset::UsedRadio, 1)]));
                } else {
                    // A local briefing can take place at any rendezvous, but its
                    // two locations must actually coincide in the ledger.
                    let colocated = LOCATIONS
                        .into_iter()
                        .map(|site| {
                            Q::multiply(
                                q(Role::Scout, Asset::At(site)),
                                q(Role::Medic, Asset::At(site)),
                            )
                        })
                        .reduce(|a, b| a + b)
                        .unwrap();
                    share = share.condition(
                        "local briefing requires rendezvous",
                        colocated,
                        C::Equal,
                        Q::constant(1),
                    );
                }
                builder = builder.rate(
                    RateId::Share {
                        objective: id,
                        location,
                        remote,
                    },
                    share,
                );
            }
            if patient(id) {
                for restore in [false, true] {
                    let mut treat = objective(work(Agent::Medic, 1), id)
                        .preserve(
                            Role::Medic,
                            basket([(Asset::At(location), 1), (Asset::Known(id, location), 1)]),
                        )
                        .preserve(Role::Objective, basket([(Asset::Located, 1)]))
                        .condition(
                            "patient alive",
                            q(Role::Objective, Asset::Health),
                            C::GreaterThanOrEqual,
                            Q::constant(1),
                        )
                        .consume(Role::Medic, basket([(Asset::MedicalKit, 1)]))
                        .produce(Role::Medic, basket([(Asset::UsedMedicalKit, 1)]))
                        .consume(Role::Objective, basket([(Asset::Unstable, 1)]))
                        .produce(Role::Objective, basket([(Asset::Stable, 1)]));
                    if restore {
                        treat = treat
                            .consume(Role::Objective, basket([(Asset::Harm, 1)]))
                            .produce(Role::Objective, basket([(Asset::Health, 1)]));
                    } else {
                        treat = treat.condition(
                            "no damage to restore",
                            q(Role::Objective, Asset::Harm),
                            C::Equal,
                            Q::constant(0),
                        );
                    }
                    builder = builder.rate(
                        RateId::TreatPatient {
                            objective: id,
                            location,
                            restore,
                        },
                        treat,
                    );
                }
                let mut extract = before_horizon(objective(law(), id), Q::constant(2))
                    .preserve(Role::Objective, basket([(Asset::Stable, 1)]))
                    .condition(
                        "patient alive",
                        q(Role::Objective, Asset::Health),
                        C::GreaterThanOrEqual,
                        Q::constant(1),
                    )
                    .consume(Role::Objective, basket([(Asset::Located, 1)]))
                    .produce(Role::Objective, basket([(Asset::Working, 1)]))
                    .consume(Role::Medic, basket([(Asset::Seat, 1)]))
                    .produce(Role::Medic, basket([(Asset::ReservedSeat(id), 1)]));
                for agent in AGENTS {
                    extract = permit(extract, agent, 1)
                        .preserve(
                            role(agent),
                            basket([(Asset::At(location), 1), (Asset::Known(id, location), 1)]),
                        )
                        .consume(role(agent), basket([(Asset::Ready, 1)]))
                        .produce(role(agent), basket([(Asset::Extracting(id), 1)]))
                        .produce_computed(role(agent), Asset::ReadyAt, clock() + Q::constant(2));
                }
                builder = builder.rate(
                    RateId::Extract {
                        objective: id,
                        location,
                    },
                    extract,
                );
            } else {
                builder = builder.rate(
                    RateId::Collect {
                        objective: id,
                        location,
                    },
                    before_horizon(
                        objective(permit(law(), Agent::Scout, 2), id),
                        Q::constant(3),
                    )
                    .preserve(
                        Role::Scout,
                        basket([(Asset::At(location), 1), (Asset::Known(id, location), 1)]),
                    )
                    .consume(Role::Scout, basket([(Asset::Ready, 1)]))
                    .produce(Role::Scout, basket([(Asset::Collecting(id), 1)]))
                    .produce_computed(Role::Scout, Asset::ReadyAt, clock() + Q::constant(3))
                    .consume(Role::Objective, basket([(Asset::Located, 1)]))
                    .produce(Role::Objective, basket([(Asset::Working, 1)])),
                );
            }
        }
        let carrier = if patient(id) {
            Role::Medic
        } else {
            Role::Scout
        };
        let mut finish = objective(law(), id)
            .consume(Role::Objective, basket([(Asset::Working, 1)]))
            .produce(Role::Objective, basket([(Asset::Carried, 1)]))
            .produce(carrier, basket([(Asset::Cargo(id), 1)]));
        for agent in if patient(id) {
            &AGENTS[..]
        } else {
            &AGENTS[..1]
        } {
            finish = finish
                .condition(
                    "work duration elapsed",
                    clock(),
                    C::GreaterThanOrEqual,
                    q(role(*agent), Asset::ReadyAt),
                )
                .consume(
                    role(*agent),
                    basket([(
                        if patient(id) {
                            Asset::Extracting(id)
                        } else {
                            Asset::Collecting(id)
                        },
                        1,
                    )]),
                )
                .produce(role(*agent), basket([(Asset::Ready, 1)]))
                .consume_computed(
                    role(*agent),
                    Asset::ReadyAt,
                    q(role(*agent), Asset::ReadyAt),
                );
        }
        if patient(id) {
            finish = finish.consume(Role::Medic, basket([(Asset::ReservedSeat(id), 1)]));
        }
        builder = builder.rate(
            if patient(id) {
                RateId::FinishExtract(id)
            } else {
                RateId::FinishCollect(id)
            },
            finish,
        );
        let mut deliver = objective(
            work(
                if patient(id) {
                    Agent::Medic
                } else {
                    Agent::Scout
                },
                0,
            ),
            id,
        )
        .preserve(carrier, basket([(Asset::At(Location::Base), 1)]))
        .condition(
            "delivery before deadline",
            clock(),
            C::LessThanOrEqual,
            q(Role::Objective, Asset::Deadline),
        )
        .consume(carrier, basket([(Asset::Cargo(id), 1)]))
        .consume(Role::Objective, basket([(Asset::Carried, 1)]))
        .produce(Role::Objective, basket([(Asset::Complete, 1)]))
        .produce(
            Role::World,
            basket([(Asset::Resolved, 1), (Asset::Completed, 1)]),
        );
        if patient(id) {
            deliver = deliver
                .condition(
                    "evacuated alive",
                    q(Role::Objective, Asset::Health),
                    C::GreaterThanOrEqual,
                    Q::constant(1),
                )
                .produce(Role::Medic, basket([(Asset::Seat, 1)]))
                .produce_computed(
                    Role::World,
                    Asset::SavedVitality,
                    q(Role::Objective, Asset::Health),
                );
        }
        builder = builder.rate(RateId::Deliver(id), deliver);
        for phase in [Phase::Hidden, Phase::Located, Phase::Carried] {
            for dead in [false, true] {
                if dead && !patient(id) {
                    continue;
                }
                let mut fail = objective(law(), id)
                    .consume(Role::Objective, basket([(phase_asset(phase), 1)]))
                    .produce(Role::Objective, basket([(Asset::Failed, 1)]))
                    .produce(
                        Role::World,
                        basket([(Asset::Resolved, 1), (Asset::FailedCount, 1)]),
                    );
                fail = if dead {
                    fail.condition(
                        "patient died",
                        q(Role::Objective, Asset::Health),
                        C::Equal,
                        Q::constant(0),
                    )
                } else {
                    fail.condition(
                        "deadline passed",
                        clock(),
                        C::GreaterThanOrEqual,
                        q(Role::Objective, Asset::Deadline) + Q::constant(1),
                    )
                };
                if phase == Phase::Carried {
                    fail = fail.consume(carrier, basket([(Asset::Cargo(id), 1)]));
                    if patient(id) {
                        fail = fail.produce(Role::Medic, basket([(Asset::Seat, 1)]));
                    }
                }
                builder = builder.rate(
                    RateId::Fail {
                        objective: id,
                        phase,
                        dead,
                    },
                    fail,
                );
            }
        }
        if patient(id) {
            for day in [8, 16, 24] {
                for active in [false, true] {
                    let unresolved = q(Role::Objective, Asset::Hidden)
                        + q(Role::Objective, Asset::Located)
                        + q(Role::Objective, Asset::Working)
                        + q(Role::Objective, Asset::Carried);
                    let mut clinical = objective(
                        event().preserve(Role::World, basket([(Asset::EnvironmentReady, 1)])),
                        id,
                    )
                    .condition(
                        "clinical pulse time reached",
                        clock(),
                        C::GreaterThanOrEqual,
                        Q::constant(day.into()),
                    )
                    .condition(
                        "scheduled pulse",
                        q(Role::World, Asset::NextClinical),
                        C::Equal,
                        Q::constant(day.into()),
                    )
                    .consume(Role::Objective, basket([(Asset::ClinicalDue(day), 1)]))
                    .produce(Role::Objective, basket([(Asset::ClinicalSettled(day), 1)]))
                    .produce(Role::World, basket([(Asset::ClinicalCount, 1)]))
                    .condition(
                        "match living unresolved patient",
                        Q::multiply(
                            unresolved,
                            Q::minimum(q(Role::Objective, Asset::Health), Q::constant(1)),
                        ),
                        C::Equal,
                        Q::constant(u64::from(active)),
                    );
                    if active {
                        clinical = clinical
                            .consume(Role::Objective, basket([(Asset::Health, 1)]))
                            .produce(Role::Objective, basket([(Asset::Harm, 1)]))
                            .consume_computed(
                                Role::Objective,
                                Asset::Stable,
                                q(Role::Objective, Asset::Stable),
                            )
                            .produce_computed(
                                Role::Objective,
                                Asset::Unstable,
                                q(Role::Objective, Asset::Stable),
                            );
                    }
                    builder = builder.rate(
                        RateId::Clinical {
                            objective: id,
                            day,
                            active,
                        },
                        clinical,
                    );
                }
            }
        }
    }
    for location in sites.iter().copied() {
        builder = builder.rate(
            RateId::Survey(location),
            work(Agent::Scout, 1)
                .preserve(Role::Scout, basket([(Asset::At(location), 1)]))
                .consume(Role::Scout, basket([(Asset::Unsearched(location), 1)]))
                .produce(Role::Scout, basket([(Asset::Surveyed(location), 1)])),
        );
        for round in 0..2 {
            builder = builder.rate(
                RateId::Report { round, location },
                nature(law())
                    .preserve(Role::Nature, basket([(Asset::Signal(round, location), 1)]))
                    .consume(
                        Role::Scout,
                        basket([(Asset::AwaitingReport, 1), (Asset::ScanRound(round), 1)]),
                    )
                    .produce(
                        Role::Scout,
                        basket([
                            (Asset::Ready, 1),
                            (Asset::ScanRound(round + 1), 1),
                            (Asset::Hint(round, location), 1),
                        ]),
                    ),
            );
        }
        for opening in [false, true] {
            builder = builder.rate(
                RateId::Weather { location, opening },
                nature(event())
                    .preserve(Role::Nature, basket([(Asset::HazardSite(location), 1)]))
                    .consume(
                        Role::World,
                        basket([
                            (Asset::WeatherDue(opening), 1),
                            (
                                if opening {
                                    Asset::WeatherBlocked(location)
                                } else {
                                    Asset::WeatherOpen(location)
                                },
                                1,
                            ),
                        ]),
                    )
                    .produce(
                        Role::World,
                        basket([
                            (Asset::EnvironmentReady, 1),
                            (
                                if opening {
                                    Asset::WeatherOpen(location)
                                } else {
                                    Asset::WeatherBlocked(location)
                                },
                                1,
                            ),
                        ]),
                    ),
            );
        }
    }
    builder = builder.rate(
        RateId::Scan,
        permit(law(), Agent::Scout, 1)
            .consume(Role::Scout, basket([(Asset::Ready, 1), (Asset::Sensor, 1)]))
            .produce(
                Role::Scout,
                basket([(Asset::AwaitingReport, 1), (Asset::UsedSensor, 1)]),
            ),
    );
    for day in 0..horizon as u8 {
        let mut tick = law()
            .condition("clock step", clock(), C::Equal, Q::constant(day.into()))
            .consume(Role::World, basket([(Asset::TimeRemaining, 1)]))
            .produce(Role::World, basket([(Asset::Time, 1)]));
        for agent in AGENTS {
            tick = tick
                .consume_computed(
                    Role::World,
                    Asset::UsedPermit(agent),
                    q(Role::World, Asset::UsedPermit(agent)),
                )
                .produce_computed(
                    Role::World,
                    Asset::Permit(agent),
                    q(Role::World, Asset::UsedPermit(agent)),
                );
        }
        // Tick and Nature settlement are distinct authoritative exchanges.
        if [5, 10].contains(&(day + 1)) {
            tick = event()
                .condition("clock step", clock(), C::Equal, Q::constant(day.into()))
                .consume(
                    Role::World,
                    basket([(Asset::EnvironmentReady, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::World,
                    basket([(Asset::Time, 1), (Asset::WeatherDue(day + 1 == 10), 1)]),
                );
            for agent in AGENTS {
                tick = tick
                    .consume_computed(
                        Role::World,
                        Asset::UsedPermit(agent),
                        q(Role::World, Asset::UsedPermit(agent)),
                    )
                    .produce_computed(
                        Role::World,
                        Asset::Permit(agent),
                        q(Role::World, Asset::UsedPermit(agent)),
                    );
            }
        }
        builder = builder.rate(RateId::Tick(day), tick);
    }
    for day in [8, 16, 24] {
        builder = builder.rate(
            RateId::FinishClinical(day),
            event()
                .condition(
                    "scheduled pulse",
                    q(Role::World, Asset::NextClinical),
                    C::Equal,
                    Q::constant(day.into()),
                )
                .condition(
                    "all casualties assessed",
                    q(Role::World, Asset::ClinicalCount),
                    C::Equal,
                    Q::constant(u64::from(count.div_ceil(2))),
                )
                .consume_computed(
                    Role::World,
                    Asset::ClinicalCount,
                    q(Role::World, Asset::ClinicalCount),
                )
                .produce(
                    Role::World,
                    basket([(Asset::NextClinical, if day == 24 { 100 } else { 8 })]),
                ),
        );
        builder = builder.invariant(
            LinearInvariant::new(format!("clinical pulse {day}"))
                .weight(Asset::ClinicalDue(day), 1)
                .weight(Asset::ClinicalSettled(day), 1),
        );
    }
    let mut activity = LinearInvariant::new("two independent agent activities")
        .weight(Asset::Ready, 1)
        .weight(Asset::AwaitingReport, 1);
    let mut positions = LinearInvariant::new("two physical agent positions");
    let mut surveys = LinearInvariant::new("finite sector surveys");
    let mut seat = LinearInvariant::new("one medic evacuation seat").weight(Asset::Seat, 1);
    for location in LOCATIONS {
        activity = activity.weight(Asset::Travelling(location), 1);
        positions = positions
            .weight(Asset::At(location), 1)
            .weight(Asset::Travelling(location), 1);
        surveys = surveys
            .weight(Asset::Unsearched(location), 1)
            .weight(Asset::Surveyed(location), 1);
    }
    for id in 0..count {
        activity = activity
            .weight(Asset::Collecting(id), 1)
            .weight(Asset::Extracting(id), 1);
        if patient(id) {
            seat = seat
                .weight(Asset::ReservedSeat(id), 1)
                .weight(Asset::Cargo(id), 1);
        }
    }
    for agent in AGENTS {
        builder = builder.invariant(
            LinearInvariant::new(format!("{agent:?} one decision per tick"))
                .weight(Asset::Permit(agent), 1)
                .weight(Asset::UsedPermit(agent), 1),
        );
    }
    builder
        .rate(
            RateId::Close,
            identity()
                .condition(
                    "all objectives resolved",
                    q(Role::World, Asset::Resolved),
                    C::Equal,
                    q(Role::World, Asset::Count),
                )
                .preserve(Role::Scout, basket([(Asset::Ready, 1)]))
                .preserve(Role::Medic, basket([(Asset::Ready, 1)]))
                .consume(Role::World, basket([(Asset::Active, 1)]))
                .produce(Role::World, basket([(Asset::Closed, 1)])),
        )
        .invariant(activity)
        .invariant(positions)
        .invariant(surveys)
        .invariant(seat)
        .invariant(
            LinearInvariant::new("objective lifecycle")
                .weight(Asset::Hidden, 1)
                .weight(Asset::Located, 1)
                .weight(Asset::Working, 1)
                .weight(Asset::Carried, 1)
                .weight(Asset::Complete, 1)
                .weight(Asset::Failed, 1),
        )
        .invariant(
            LinearInvariant::new("health and harm")
                .weight(Asset::Health, 1)
                .weight(Asset::Harm, 1),
        )
        .invariant(
            LinearInvariant::new("stabilization state")
                .weight(Asset::Stable, 1)
                .weight(Asset::Unstable, 1),
        )
        .invariant(
            LinearInvariant::new("energy")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("medical kits")
                .weight(Asset::MedicalKit, 1)
                .weight(Asset::UsedMedicalKit, 1),
        )
        .invariant(
            LinearInvariant::new("radio charges")
                .weight(Asset::Radio, 1)
                .weight(Asset::UsedRadio, 1),
        )
        .invariant(
            LinearInvariant::new("sensor charges")
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant(
            LinearInvariant::new("finite resupply")
                .weight(Asset::Supply, 1)
                .weight(Asset::UsedSupply, 1),
        )
        .invariant(
            LinearInvariant::new("time")
                .weight(Asset::Time, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new("mission lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Closed, 1),
        )
        .invariant(
            LinearInvariant::new("weather settlement")
                .weight(Asset::EnvironmentReady, 1)
                .weight(Asset::WeatherDue(false), 1)
                .weight(Asset::WeatherDue(true), 1),
        )
        .build()
        .expect("expedition resources and stages conserved")
}

pub fn action(rate: RateId) -> Action {
    let mut exchange = Exchange::new(rate, Quantity::new(1))
        .bind(Role::World, AccountId::World)
        .bind(Role::Scout, AccountId::Agent(Agent::Scout))
        .bind(Role::Medic, AccountId::Agent(Agent::Medic));
    match rate {
        RateId::Discover { objective, .. } => {
            exchange = exchange
                .bind(Role::Objective, AccountId::Objective(objective))
                .bind(Role::Nature, AccountId::Nature);
        }
        RateId::Collect { objective, .. }
        | RateId::TreatPatient { objective, .. }
        | RateId::Extract { objective, .. }
        | RateId::FinishCollect(objective)
        | RateId::FinishExtract(objective)
        | RateId::Deliver(objective)
        | RateId::Clinical { objective, .. }
        | RateId::Fail { objective, .. } => {
            exchange = exchange.bind(Role::Objective, AccountId::Objective(objective));
        }
        RateId::Report { .. } | RateId::Weather { .. } => {
            exchange = exchange.bind(Role::Nature, AccountId::Nature);
        }
        _ => {}
    }
    exchange
}
pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::World, basket([(Asset::Closed, 1)]))
}
pub fn agent_view(world: &World, agent: Agent) -> AgentView<'_> {
    let mut accounts = vec![AccountId::World, AccountId::Agent(agent)];
    for id in 0..value(world, AccountId::World, Asset::Count) as u8 {
        if LOCATIONS
            .into_iter()
            .any(|location| value(world, AccountId::Agent(agent), Asset::Known(id, location)) > 0)
        {
            accounts.push(AccountId::Objective(id));
        }
    }
    world.view(accounts)
}
pub fn candidates(world: &World) -> Vec<Action> {
    let mut rates: Vec<_> = world.rate_ids().copied().collect();
    rates.sort();
    world.applicable(rates.into_iter().map(action))
}
pub fn response(world: &World) -> Option<Action> {
    let count = value(world, AccountId::World, Asset::Count) as u8;
    let mut rates = Vec::new();
    for location in LOCATIONS {
        for opening in [false, true] {
            rates.push(RateId::Weather { location, opening });
        }
    }
    for day in [8, 16, 24] {
        rates.push(RateId::FinishClinical(day));
    }
    for id in 0..count {
        if patient(id) {
            for day in [8, 16, 24] {
                for active in [true, false] {
                    rates.push(RateId::Clinical {
                        objective: id,
                        day,
                        active,
                    });
                }
            }
        }
        for phase in [Phase::Hidden, Phase::Located, Phase::Carried] {
            for dead in [true, false] {
                rates.push(RateId::Fail {
                    objective: id,
                    phase,
                    dead,
                });
            }
        }
        rates.push(if patient(id) {
            RateId::FinishExtract(id)
        } else {
            RateId::FinishCollect(id)
        });
        for location in LOCATIONS {
            rates.push(RateId::Discover {
                objective: id,
                location,
            });
        }
    }
    for agent in AGENTS {
        for to in LOCATIONS {
            for hazardous in [false, true] {
                rates.push(RateId::Arrive {
                    agent,
                    to,
                    hazardous,
                });
            }
        }
    }
    for round in 0..2 {
        for location in LOCATIONS {
            rates.push(RateId::Report { round, location });
        }
    }
    rates
        .into_iter()
        .map(action)
        .find(|exchange| world.is_applicable(exchange))
}
/// Policies read only their own account, public team positions and objectives
/// explicitly learned by that agent. They cannot inspect Nature or teammate intel.
pub fn decision(view: &AgentView<'_>, agent: Agent, policy: Policy) -> Option<Action> {
    let val = |account, asset| view.balance(&account, &asset).map_or(0, Quantity::get);
    let own = AccountId::Agent(agent);
    let world = AccountId::World;
    if val(own, Asset::Ready) == 0 || val(world, Asset::Permit(agent)) == 0 {
        return None;
    }
    let from = LOCATIONS
        .into_iter()
        .find(|location| val(own, Asset::At(*location)) > 0)?;
    let count = val(world, Asset::Count) as u8;
    let known = |id| {
        LOCATIONS
            .into_iter()
            .find(|location| val(own, Asset::Known(id, *location)) > 0)
    };
    let move_to = |to| {
        if to != from
            && val(world, Asset::WeatherOpen(to)) > 0
            && val(own, Asset::Energy) >= distance(from, to) + val(own, Asset::Harm)
        {
            Some(action(RateId::Depart { agent, from, to }))
        } else {
            None
        }
    };
    if let Some(id) = (0..count).find(|id| val(own, Asset::Cargo(*id)) > 0) {
        return if from == Location::Base {
            Some(action(RateId::Deliver(id)))
        } else {
            move_to(Location::Base)
        };
    }
    if from == Location::Base
        && val(world, Asset::Supply) > 0
        && (val(own, Asset::Energy) < 7
            || (agent == Agent::Medic && val(own, Asset::MedicalKit) == 0))
    {
        return Some(action(RateId::Resupply(agent)));
    }
    if agent == Agent::Medic {
        if val(own, Asset::MedicalKit) > 1 && val(own, Asset::Harm) >= 2 {
            return Some(action(RateId::TreatAgent {
                target: Agent::Medic,
                location: from,
            }));
        }
        for id in (0..count).filter(|id| patient(*id)) {
            let Some(site) = known(id) else {
                continue;
            };
            if val(AccountId::Objective(id), Asset::Located) == 0 {
                continue;
            }
            if val(own, Asset::MedicalKit) == 0
                && val(AccountId::Objective(id), Asset::Unstable) > 0
                && val(world, Asset::Supply) > 0
            {
                return move_to(Location::Base);
            }
            if from != site {
                return move_to(site);
            }
            if val(AccountId::Objective(id), Asset::Unstable) > 0 && val(own, Asset::MedicalKit) > 0
            {
                if val(world, Asset::NextClinical) <= val(world, Asset::Time) + 1 {
                    return None;
                }
                return Some(action(RateId::TreatPatient {
                    objective: id,
                    location: site,
                    restore: val(AccountId::Objective(id), Asset::Harm) > 0,
                }));
            }
            return None;
        }
        return if from != Location::Base {
            move_to(Location::Base)
        } else {
            None
        };
    }
    // Learn the sector before acting on a fallible sensor hint.
    if val(own, Asset::Unsearched(from)) > 0 {
        return Some(action(RateId::Survey(from)));
    }
    for id in (0..count).filter(|id| patient(*id)) {
        let Some(site) = known(id) else {
            continue;
        };
        if val(AccountId::Objective(id), Asset::Located) == 0 {
            continue;
        }
        if val(own, Asset::Shared(id, site)) == 0 {
            let together = val(world, Asset::TeamAt(Agent::Medic, from)) > 0;
            if together {
                return Some(action(RateId::Share {
                    objective: id,
                    location: site,
                    remote: false,
                }));
            }
            if policy == Policy::RadioCoordination && val(own, Asset::Radio) > 0 {
                return Some(action(RateId::Share {
                    objective: id,
                    location: site,
                    remote: true,
                }));
            }
            return move_to(Location::Base);
        }
        if from == site
            && val(world, Asset::TeamAt(Agent::Medic, site)) > 0
            && val(world, Asset::Permit(Agent::Medic)) > 0
            && val(AccountId::Objective(id), Asset::Stable) > 0
        {
            return Some(action(RateId::Extract {
                objective: id,
                location: site,
            }));
        }
        if from != site && val(world, Asset::WeatherOpen(site)) > 0 {
            return move_to(site);
        }
        // While the medic is on the way, collect data already at this sector;
        // otherwise preserve the rendezvous rather than repeatedly wandering.
        if from == site {
            return None;
        }
    }
    for id in (0..count).filter(|id| !patient(*id)) {
        if val(AccountId::Objective(id), Asset::Located) == 0 {
            continue;
        }
        if let Some(site) = known(id) {
            if from == site {
                return Some(action(RateId::Collect {
                    objective: id,
                    location: site,
                }));
            }
            if let Some(exchange) = move_to(site) {
                return Some(exchange);
            }
        }
    }
    if policy != Policy::NorthFirst
        && from == Location::Base
        && val(own, Asset::Sensor) > 0
        && (val(own, Asset::UsedSensor) == 0 || val(world, Asset::Completed) > 0)
    {
        return Some(action(RateId::Scan));
    }
    let mut sites: Vec<_> = LOCATIONS
        .into_iter()
        .filter(|site| {
            val(own, Asset::Unsearched(*site)) > 0 && val(world, Asset::WeatherOpen(*site)) > 0
        })
        .collect();
    sites.sort_by_key(|site| {
        (
            if policy == Policy::NorthFirst {
                u64::from(*site != Location::North)
            } else {
                u64::from(!(0..count).any(|id| val(own, Asset::Hint(id, *site)) > 0))
            },
            distance(from, *site),
            *site,
        )
    });
    sites.into_iter().find_map(move_to)
}
pub fn run(source: &World, policy: Policy) -> Proposal {
    let mut world = source.clone();
    let mut trace = Trace::new();
    for _ in 0..500 {
        if world.matches(&goal()) {
            return trace;
        }
        let exchange = response(&world)
            .or_else(|| {
                let close = action(RateId::Close);
                world.is_applicable(&close).then_some(close)
            })
            .or_else(|| {
                AGENTS
                    .into_iter()
                    .filter_map(|agent| decision(&agent_view(&world, agent), agent, policy))
                    .find(|exchange| world.is_applicable(exchange))
            })
            .unwrap_or_else(|| action(RateId::Tick(now(&world) as u8)));
        world.apply(exchange.clone()).unwrap_or_else(|error| {
            panic!(
                "{policy:?} at {}: {:?}: {error:?}",
                now(&world),
                exchange.rate()
            )
        });
        trace.push(exchange);
    }
    panic!("finite expedition failed to terminate")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expeditions_replay_independent_work_and_common_scenario_tradeoffs() {
        let mut outcomes = std::collections::BTreeSet::new();
        let mut overlap = false;
        let mut extraction = false;
        let mut local = false;
        for stress in [false, true] {
            for seed in [0, 3, 17, u64::MAX] {
                for policy in [
                    Policy::RadioCoordination,
                    Policy::Rendezvous,
                    Policy::NorthFirst,
                ] {
                    let source = initial(stress, seed);
                    let before = source.state_key();
                    let trace = run(&source, policy);
                    assert_eq!(trace, run(&source, policy));
                    let mut replay = source.clone();
                    for exchange in trace.exchanges() {
                        replay.apply(exchange.clone()).unwrap();
                        overlap |= AGENTS.into_iter().all(|agent| {
                            value(&replay, AccountId::Agent(agent), Asset::Ready) == 0
                        });
                        for agent in AGENTS {
                            let own = AccountId::Agent(agent);
                            assert_eq!(
                                LOCATIONS
                                    .into_iter()
                                    .map(|site| value(&replay, own, Asset::At(site))
                                        + value(&replay, own, Asset::Travelling(site)))
                                    .sum::<u64>(),
                                1
                            );
                            for site in LOCATIONS {
                                assert_eq!(
                                    value(&replay, own, Asset::At(site)),
                                    value(&replay, AccountId::World, Asset::TeamAt(agent, site))
                                );
                            }
                        }
                    }
                    extraction |= trace
                        .exchanges()
                        .iter()
                        .any(|exchange| matches!(exchange.rate(), RateId::Extract { .. }));
                    local |= trace.exchanges().iter().any(|exchange| {
                        matches!(exchange.rate(), RateId::Share { remote: false, .. })
                    });
                    assert_eq!(source.state_key(), before);
                    assert!(replay.matches(&goal()));
                    let completed = value(&replay, AccountId::World, Asset::Completed);
                    let failed = value(&replay, AccountId::World, Asset::FailedCount);
                    assert_eq!(
                        completed + failed,
                        value(&source, AccountId::World, Asset::Count)
                    );
                    outcomes.insert((completed, failed, now(&replay)));
                    if seed == 17 {
                        eprintln!(
                            "{stress:?} {policy:?}: completed {completed}, failed {failed}, time {}, vitality {}",
                            now(&replay),
                            value(&replay, AccountId::World, Asset::SavedVitality)
                        );
                    }
                }
            }
        }
        assert!(overlap && extraction && local);
        assert!(outcomes.len() >= 4, "{outcomes:?}");
    }
    #[test]
    fn equal_observations_give_equal_decisions_without_hidden_intelligence() {
        let a = initial(false, 0);
        let b = initial(false, 17);
        assert_ne!(a.state_key(), b.state_key());
        for agent in AGENTS {
            let left = agent_view(&a, agent);
            let right = agent_view(&b, agent);
            assert_eq!(left.observation_key(), right.observation_key());
            assert!(left.account(&AccountId::Nature).is_none());
            assert!(
                left.account(&AccountId::Agent(if agent == Agent::Scout {
                    Agent::Medic
                } else {
                    Agent::Scout
                }))
                .is_none()
            );
            assert!(left.account(&AccountId::Objective(0)).is_none());
            for policy in [
                Policy::RadioCoordination,
                Policy::Rendezvous,
                Policy::NorthFirst,
            ] {
                assert_eq!(
                    decision(&left, agent, policy),
                    decision(&right, agent, policy)
                );
            }
        }
    }
    #[test]
    fn base_resupply_does_not_teleport_medicine_to_an_absent_medic() {
        let mut world = initial(false, 17);
        world
            .apply(action(RateId::Depart {
                agent: Agent::Medic,
                from: Location::Base,
                to: Location::North,
            }))
            .unwrap();
        world.apply(action(RateId::Resupply(Agent::Scout))).unwrap();
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Medic), Asset::MedicalKit),
            2
        );
        assert_eq!(value(&world, AccountId::World, Asset::MedicalKit), 2);
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Scout), Asset::Energy),
            20
        );
        assert!(!world.is_applicable(&action(RateId::Resupply(Agent::Medic))));
    }
    #[test]
    fn private_discovery_requires_a_real_briefing_before_joint_extraction() {
        let source = initial(false, 17);
        let trace = run(&source, Policy::RadioCoordination);
        let mut replay = source.clone();
        let mut discovered = false;
        let mut briefed = false;
        let mut extracted = false;
        for exchange in trace.exchanges() {
            if let RateId::Extract {
                objective,
                location,
            } = exchange.rate()
            {
                assert!(briefed);
                assert_eq!(
                    value(&replay, AccountId::Objective(*objective), Asset::Stable),
                    1
                );
                for agent in AGENTS {
                    assert_eq!(
                        value(&replay, AccountId::Agent(agent), Asset::At(*location)),
                        1
                    );
                }
                let rebound = exchange
                    .clone()
                    .bind(Role::Medic, AccountId::Agent(Agent::Scout));
                assert!(!replay.is_applicable(&rebound));
                extracted = true;
            }
            replay.apply(exchange.clone()).unwrap();
            if let RateId::Discover {
                objective: 0,
                location,
            } = exchange.rate()
            {
                assert!(
                    agent_view(&replay, Agent::Scout)
                        .account(&AccountId::Objective(0))
                        .is_some()
                );
                assert!(
                    agent_view(&replay, Agent::Medic)
                        .account(&AccountId::Objective(0))
                        .is_none()
                );
                assert!(!replay.is_applicable(&action(RateId::Extract {
                    objective: 0,
                    location: *location
                })));
                discovered = true;
            }
            if let RateId::Share {
                objective: 0,
                remote: true,
                ..
            } = exchange.rate()
            {
                assert!(discovered);
                assert!(
                    agent_view(&replay, Agent::Medic)
                        .account(&AccountId::Objective(0))
                        .is_some()
                );
                assert_eq!(
                    value(&replay, AccountId::Agent(Agent::Scout), Asset::UsedRadio),
                    1
                );
                briefed = true;
            }
        }
        assert!(discovered && briefed && extracted);
    }
    #[test]
    fn movement_occupies_agents_and_scheduled_updates_cannot_be_skipped() {
        let mut world = initial(false, 0);
        world
            .apply(action(RateId::Depart {
                agent: Agent::Scout,
                from: Location::Base,
                to: Location::North,
            }))
            .unwrap();
        world
            .apply(action(RateId::Depart {
                agent: Agent::Medic,
                from: Location::Base,
                to: Location::South,
            }))
            .unwrap();
        assert!(!world.is_applicable(&action(RateId::Arrive {
            agent: Agent::Scout,
            to: Location::North,
            hazardous: false
        })));
        assert!(!world.is_applicable(&action(RateId::Survey(Location::North))));
        world.apply(action(RateId::Tick(0))).unwrap();
        while let Some(exchange) = response(&world) {
            world.apply(exchange).unwrap();
        }
        assert_eq!(
            value(
                &world,
                AccountId::Agent(Agent::Scout),
                Asset::At(Location::North)
            ),
            1
        );
        assert_eq!(
            value(
                &world,
                AccountId::Agent(Agent::Medic),
                Asset::At(Location::South)
            ),
            1
        );
        for tick in 1..5 {
            world.apply(action(RateId::Tick(tick))).unwrap();
        }
        assert!(!world.is_applicable(&action(RateId::Tick(5))));
        assert!(!world.is_applicable(&action(RateId::Survey(Location::North))));
        while let Some(exchange) = response(&world) {
            world.apply(exchange).unwrap();
        }
        for tick in 5..8 {
            world.apply(action(RateId::Tick(tick))).unwrap();
        }
        assert!(!world.is_applicable(&action(RateId::Tick(8))));
        assert!(!world.is_applicable(&action(RateId::Survey(Location::North))));
        while let Some(exchange) = response(&world) {
            world.apply(exchange).unwrap();
        }
        assert_eq!(value(&world, AccountId::Objective(0), Asset::Health), 2);
        assert!(world.is_applicable(&action(RateId::Tick(8))));
    }
}
