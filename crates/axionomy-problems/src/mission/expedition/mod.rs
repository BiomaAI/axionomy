//! Independent teammates, private observations and timed cooperative extraction.
use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Relation, Segment, Slot, Variable,
    basket,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Agent {
    Scout,
    Medic,
}
pub const AGENTS: [Agent; 2] = [Agent::Scout, Agent::Medic];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Nature,
    Agent(Agent),
    Objective(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Scout,
    Medic,
    Nature,
    Objective,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Phase {
    Hidden,
    Located,
    Carried,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `mission/expedition` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "mission/expedition";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub fn patient(id: u8) -> bool {
    id.is_multiple_of(2)
}
/// Whether objective `id` is an archive: the odd ones are.
fn archive(id: u8) -> bool {
    !patient(id)
}
pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
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
fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
fn clock() -> Q<Ontology> {
    q(Role::World, Asset::Time)
}
/// One unit of each asset `assets` name.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}
/// One event of the mission, between the world and both teammates, each
/// role fixed to its account.
fn identity() -> Rate<Ontology> {
    Rate::new()
        .account(Role::World, Pattern::literal(AccountId::World))
        .account(
            Role::Scout,
            Pattern::literal(AccountId::Agent(Agent::Scout)),
        )
        .account(
            Role::Medic,
            Pattern::literal(AccountId::Agent(Agent::Medic)),
        )
        .condition(
            path!("one-mission-event"),
            Q::units(),
            C::Equal,
            Q::constant(1),
        )
}
fn event() -> Rate<Ontology> {
    identity().preserve(Role::World, basket([(Asset::Active, 1)]))
}
/// `rate` once the clinical pulse due before the next tick has settled.
fn pulse_settled(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.condition(
        path!("settle-scheduled-clinical-events-first"),
        clock() + Q::constant(1),
        C::LessThanOrEqual,
        q(Role::World, Asset::NextClinical),
    )
}
fn law() -> Rate<Ontology> {
    pulse_settled(event().preserve(Role::World, basket([(Asset::EnvironmentReady, 1)])))
}
fn permit(rate: Rate<Ontology>, agent: Agent, energy: u64) -> Rate<Ontology> {
    rate.consume(Role::World, basket([(Asset::Permit(agent), 1)]))
        .produce(Role::World, basket([(Asset::UsedPermit(agent), 1)]))
        .consume(role(agent), basket([(Asset::Energy, energy)]))
        .produce(role(agent), basket([(Asset::SpentEnergy, energy)]))
}
fn work(agent: Agent, energy: u64) -> Rate<Ontology> {
    permit(law(), agent, energy).preserve(role(agent), basket([(Asset::Ready, 1)]))
}
/// `rate` for each objective `objectives` takes, `{objective}`, its role
/// fixed to that objective's account.
fn each_objective(
    rate: Rate<Ontology>,
    objective: &Param<u8>,
    objectives: Variable,
) -> Rate<Ontology> {
    rate.variable(objectives).account(
        Role::Objective,
        AccountIdPattern::Objective(objective.var()).into(),
    )
}
/// `rate` for the one objective `id`, its role fixed to its account.
fn objective(rate: Rate<Ontology>, id: u8) -> Rate<Ontology> {
    rate.account(Role::Objective, Pattern::literal(AccountId::Objective(id)))
}
/// `rate` with Nature, its role fixed to Nature's account.
fn nature(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Nature, Pattern::literal(AccountId::Nature))
}
fn before_horizon(rate: Rate<Ontology>, duration: Q<Ontology>) -> Rate<Ontology> {
    rate.condition(
        path!("activity-ends-within-mission-horizon"),
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
/// The days a clinical pulse falls on.
const PULSES: [u8; 3] = [8, 16, 24];
/// The days after the last pulse its end schedules the next for: past any
/// expedition's horizon, so none.
const LAST_PULSE_GAP: u64 = 100;
/// The ticks after which the hazard site's weather closes in, and clears.
const WEATHER: [u8; 2] = [5, 10];
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
    let mut hidden = basket([(Asset::HazardSite(hazard), 1)]);
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

    // The rates' variables: an objective, a site, two places, a scan round
    // and a day; and the numbers relations give: how many roads apart two
    // places are, and what a day brings.
    let objective_id = Param::<u8>::new(path!("objective"));
    let location = Param::<Location>::new(path!("location"));
    let (from, to) = (
        Param::<Location>::new(path!("from")),
        Param::new(path!("to")),
    );
    let apart = Param::<u64>::new(path!("distance"));
    let (round, next) = (Param::<u8>::new(path!("round")), Param::new(path!("next")));
    let day = Param::<u8>::new(path!("day"));
    let site = || location.values(sites.iter().copied());
    let at = |place: &Param<Location>| AssetPattern::At(place.var()).into();
    let known = || AssetPattern::Known(objective_id.var(), location.var()).into();
    let every_objective = || objective_id.range(0, count - 1);
    let objectives =
        |which: fn(u8) -> bool| objective_id.values((0..count).filter(|id| which(*id)));
    // The world's map: every two places, and how many roads apart they are.
    let map = || {
        Relation::pairs_giving(
            &from,
            &to,
            [apart.name().clone()],
            LOCATIONS
                .into_iter()
                .flat_map(|start| LOCATIONS.map(|end| (start, end)))
                .filter(|(start, end)| start != end)
                .map(|(start, end)| (start, end, [distance(start, end)])),
        )
    };

    for agent in AGENTS {
        let mut stock = basket([
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
        // A journey takes the road distance between two places, as the
        // world's map gives it, slowed by the traveller's harm.
        let duration = apart.quantity() + q(role(agent), Asset::Harm);
        builder = builder.rate_at(
            RateIdPattern::Depart {
                agent: Slot::Is(agent),
                from: from.var(),
                to: to.var(),
            }
            .into(),
            before_horizon(permit(law(), agent, 0), duration.clone())
                .variable(from.every())
                .variable(to.every())
                .relation(map())
                .preserve_at(
                    Role::World,
                    one([AssetPattern::WeatherOpen(to.var()).into()]),
                )
                .consume_at(
                    role(agent),
                    one([Pattern::literal(Asset::Ready), at(&from)]),
                )
                .consume_at(
                    Role::World,
                    one([AssetPattern::TeamAt(Slot::Is(agent), from.var()).into()]),
                )
                .produce(Role::World, basket([(Asset::TeamTravelling(agent), 1)]))
                .produce_at(
                    role(agent),
                    one([AssetPattern::Travelling(to.var()).into()]),
                )
                .consume_computed(role(agent), Asset::Energy, duration.clone())
                .produce_computed(role(agent), Asset::SpentEnergy, duration.clone())
                .produce_computed(role(agent), Asset::ReadyAt, clock() + duration),
        );
        // Arriving where the weather has closed in harms the traveller.
        for hazardous in [false, true] {
            let weather = if hazardous {
                AssetPattern::WeatherBlocked(to.var())
            } else {
                AssetPattern::WeatherOpen(to.var())
            };
            let mut arrive = law()
                .variable(to.every())
                .preserve_at(Role::World, one([weather.into()]))
                .condition(
                    path!("travel-time-elapsed"),
                    clock(),
                    C::GreaterThanOrEqual,
                    q(role(agent), Asset::ReadyAt),
                )
                .consume_at(
                    role(agent),
                    one([AssetPattern::Travelling(to.var()).into()]),
                )
                .consume(Role::World, basket([(Asset::TeamTravelling(agent), 1)]))
                .produce_at(
                    Role::World,
                    one([AssetPattern::TeamAt(Slot::Is(agent), to.var()).into()]),
                )
                .produce_at(role(agent), one([Pattern::literal(Asset::Ready), at(&to)]))
                .consume_computed(role(agent), Asset::ReadyAt, q(role(agent), Asset::ReadyAt));
            if hazardous {
                arrive = arrive
                    .consume(role(agent), basket([(Asset::Health, 1)]))
                    .produce(role(agent), basket([(Asset::Harm, 1)]));
            }
            builder = builder.rate_at(
                RateIdPattern::Arrive {
                    agent: Slot::Is(agent),
                    to: to.var(),
                    hazardous: Slot::Is(hazardous),
                }
                .into(),
                arrive,
            );
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
        // The Medic treats itself where it stands, or the Scout where both
        // stand.
        let mut treatment = work(Agent::Medic, 1)
            .variable(location.every())
            .preserve_at(Role::Medic, one([at(&location)]))
            .consume(Role::Medic, basket([(Asset::MedicalKit, 1)]))
            .produce(Role::Medic, basket([(Asset::UsedMedicalKit, 1)]))
            .consume(role(agent), basket([(Asset::Harm, 1)]))
            .produce(role(agent), basket([(Asset::Health, 1)]));
        if agent != Agent::Medic {
            treatment = treatment.preserve_at(
                Role::Scout,
                one([at(&location), Pattern::literal(Asset::Ready)]),
            );
        }
        builder = builder.rate_at(
            RateIdPattern::TreatAgent {
                target: Slot::Is(agent),
                location: location.var(),
            }
            .into(),
            treatment,
        );
    }
    for id in 0..count {
        let mut stock = basket([(Asset::Hidden, 1), (Asset::Deadline, horizon - 2)]);
        if patient(id) {
            stock.insert(Asset::Health, Quantity::new(3));
            stock.insert(Asset::Unstable, Quantity::new(1));
            for day in PULSES {
                stock.insert(Asset::ClinicalDue(day), Quantity::new(1));
            }
        }
        builder = builder.account(AccountId::Objective(id), Account::from(stock));
    }

    builder = builder.rate_at(
        RateIdPattern::Discover {
            objective: objective_id.var(),
            location: location.var(),
        }
        .into(),
        nature(each_objective(law(), &objective_id, every_objective()))
            .variable(site())
            .preserve_at(
                Role::Scout,
                one([AssetPattern::Surveyed(location.var()).into()]),
            )
            .preserve_at(
                Role::Nature,
                one([AssetPattern::Truth(objective_id.var(), location.var()).into()]),
            )
            .consume(Role::Objective, basket([(Asset::Hidden, 1)]))
            .produce(Role::Objective, basket([(Asset::Located, 1)]))
            .produce_at(Role::Scout, one([known()])),
    );
    // Sharing what the Scout knows of an objective with the Medic, by radio
    // or at a rendezvous.
    for remote in [false, true] {
        let mut share = work(Agent::Scout, 0)
            .variable(every_objective())
            .variable(site())
            .preserve_at(Role::Scout, one([known()]))
            .condition(
                path!("recipient-does-not-already-know"),
                Q::balance_at(Role::Medic, known()),
                C::Equal,
                Q::constant(0),
            )
            .produce_at(
                Role::Scout,
                one([AssetPattern::Shared(objective_id.var(), location.var()).into()]),
            )
            .produce_at(Role::Medic, one([known()]));
        if remote {
            share = share
                .consume(Role::Scout, basket([(Asset::Radio, 1)]))
                .produce(Role::Scout, basket([(Asset::UsedRadio, 1)]));
        } else {
            // A local briefing can take place at any rendezvous, but its
            // two locations must actually coincide in the ledger.
            let colocated = LOCATIONS
                .into_iter()
                .map(|site| q(Role::Scout, Asset::At(site)) * q(Role::Medic, Asset::At(site)))
                .reduce(|a, b| a + b)
                .unwrap();
            share = share.condition(
                path!("local-briefing-requires-rendezvous"),
                colocated,
                C::Equal,
                Q::constant(1),
            );
        }
        builder = builder.rate_at(
            RateIdPattern::Share {
                objective: objective_id.var(),
                location: location.var(),
                remote: Slot::Is(remote),
            }
            .into(),
            share,
        );
    }
    // Stabilizing a casualty who has lost health restores one; one who has
    // lost none is only stabilized.
    for restore in [false, true] {
        let mut treat = each_objective(work(Agent::Medic, 1), &objective_id, objectives(patient))
            .variable(site())
            .preserve_at(Role::Medic, one([at(&location), known()]))
            .preserve(Role::Objective, basket([(Asset::Located, 1)]))
            .condition(
                path!("patient-alive"),
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
                path!("no-damage-to-restore"),
                q(Role::Objective, Asset::Harm),
                C::Equal,
                Q::constant(0),
            );
        }
        builder = builder.rate_at(
            RateIdPattern::TreatPatient {
                objective: objective_id.var(),
                location: location.var(),
                restore: Slot::Is(restore),
            }
            .into(),
            treat,
        );
    }
    // A casualty is extracted by both teammates together.
    let mut extract = before_horizon(
        each_objective(law(), &objective_id, objectives(patient)),
        Q::constant(2),
    )
    .variable(site())
    .preserve(Role::Objective, basket([(Asset::Stable, 1)]))
    .condition(
        path!("patient-alive"),
        q(Role::Objective, Asset::Health),
        C::GreaterThanOrEqual,
        Q::constant(1),
    )
    .consume(Role::Objective, basket([(Asset::Located, 1)]))
    .produce(Role::Objective, basket([(Asset::Working, 1)]))
    .consume(Role::Medic, basket([(Asset::Seat, 1)]))
    .produce_at(
        Role::Medic,
        one([AssetPattern::ReservedSeat(objective_id.var()).into()]),
    );
    for agent in AGENTS {
        extract = permit(extract, agent, 1)
            .preserve_at(role(agent), one([at(&location), known()]))
            .consume(role(agent), basket([(Asset::Ready, 1)]))
            .produce_at(
                role(agent),
                one([AssetPattern::Extracting(objective_id.var()).into()]),
            )
            .produce_computed(role(agent), Asset::ReadyAt, clock() + Q::constant(2));
    }
    builder = builder.rate_at(
        RateIdPattern::Extract {
            objective: objective_id.var(),
            location: location.var(),
        }
        .into(),
        extract,
    );
    builder = builder.rate_at(
        RateIdPattern::Collect {
            objective: objective_id.var(),
            location: location.var(),
        }
        .into(),
        before_horizon(
            each_objective(
                permit(law(), Agent::Scout, 2),
                &objective_id,
                objectives(archive),
            ),
            Q::constant(3),
        )
        .variable(site())
        .preserve_at(Role::Scout, one([at(&location), known()]))
        .consume(Role::Scout, basket([(Asset::Ready, 1)]))
        .produce_at(
            Role::Scout,
            one([AssetPattern::Collecting(objective_id.var()).into()]),
        )
        .produce_computed(Role::Scout, Asset::ReadyAt, clock() + Q::constant(3))
        .consume(Role::Objective, basket([(Asset::Located, 1)]))
        .produce(Role::Objective, basket([(Asset::Working, 1)])),
    );
    // Finishing the work: an archive's collection by the Scout, a
    // casualty's extraction by both teammates, the casualty taking the
    // Medic's reserved seat.
    for casualty in [false, true] {
        let (carrier, workers, working): (Role, &[Agent], _) = if casualty {
            (
                Role::Medic,
                &AGENTS[..],
                AssetPattern::Extracting(objective_id.var()),
            )
        } else {
            (
                Role::Scout,
                &AGENTS[..1],
                AssetPattern::Collecting(objective_id.var()),
            )
        };
        let mut finish = each_objective(
            law(),
            &objective_id,
            if casualty {
                objectives(patient)
            } else {
                objectives(archive)
            },
        )
        .consume(Role::Objective, basket([(Asset::Working, 1)]))
        .produce(Role::Objective, basket([(Asset::Carried, 1)]))
        .produce_at(
            carrier,
            one([AssetPattern::Cargo(objective_id.var()).into()]),
        );
        for agent in workers {
            finish = finish
                .condition(
                    path!("work-duration-elapsed"),
                    clock(),
                    C::GreaterThanOrEqual,
                    q(role(*agent), Asset::ReadyAt),
                )
                .consume_at(role(*agent), one([working.clone().into()]))
                .produce(role(*agent), basket([(Asset::Ready, 1)]))
                .consume_computed(
                    role(*agent),
                    Asset::ReadyAt,
                    q(role(*agent), Asset::ReadyAt),
                );
        }
        if casualty {
            finish = finish.consume_at(
                Role::Medic,
                one([AssetPattern::ReservedSeat(objective_id.var()).into()]),
            );
        }
        builder = builder.rate_at(
            if casualty {
                RateIdPattern::FinishExtract(objective_id.var())
            } else {
                RateIdPattern::FinishCollect(objective_id.var())
            }
            .into(),
            finish,
        );
    }
    // An objective is lost once its deadline passes, or a casualty once it
    // dies, from each phase: a law of its own for each, what it gives back
    // differing by phase. One carried is lost by whoever carries it, so an
    // archive's and a casualty's loss in transit, like their deliveries,
    // differ by carrier, and are each objective's own.
    for phase in [Phase::Hidden, Phase::Located, Phase::Carried] {
        for dead in [false, true] {
            let lost = |rate: Rate<Ontology>| {
                let rate = rate
                    .consume(Role::Objective, basket([(phase_asset(phase), 1)]))
                    .produce(Role::Objective, basket([(Asset::Failed, 1)]))
                    .produce(
                        Role::World,
                        basket([(Asset::Resolved, 1), (Asset::FailedCount, 1)]),
                    );
                if dead {
                    rate.condition(
                        path!("patient-died"),
                        q(Role::Objective, Asset::Health),
                        C::Equal,
                        Q::constant(0),
                    )
                } else {
                    rate.condition(
                        path!("deadline-passed"),
                        clock(),
                        C::GreaterThanOrEqual,
                        q(Role::Objective, Asset::Deadline) + Q::constant(1),
                    )
                }
            };
            if phase == Phase::Carried && !dead {
                for id in 0..count {
                    let carrier = if patient(id) {
                        Role::Medic
                    } else {
                        Role::Scout
                    };
                    let mut fail = lost(objective(law(), id))
                        .consume(carrier, basket([(Asset::Cargo(id), 1)]));
                    if patient(id) {
                        fail = fail.produce(Role::Medic, basket([(Asset::Seat, 1)]));
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
                continue;
            }
            let mut fail = lost(each_objective(
                law(),
                &objective_id,
                if dead {
                    objectives(patient)
                } else {
                    every_objective()
                },
            ));
            if phase == Phase::Carried {
                fail = fail
                    .consume_at(
                        Role::Medic,
                        one([AssetPattern::Cargo(objective_id.var()).into()]),
                    )
                    .produce(Role::Medic, basket([(Asset::Seat, 1)]));
            }
            builder = builder.rate_at(
                RateIdPattern::Fail {
                    objective: objective_id.var(),
                    phase: Slot::Is(phase),
                    dead: Slot::Is(dead),
                }
                .into(),
                fail,
            );
        }
    }
    for id in 0..count {
        let carrier = if patient(id) {
            Role::Medic
        } else {
            Role::Scout
        };
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
            path!("delivery-before-deadline"),
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
                    path!("evacuated-alive"),
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
    }
    // A clinical pulse, on each of its days, harms a living casualty still
    // in the field and undoes its stabilization, and passes over any other.
    let pulse_days = || {
        Relation::new(
            [day.name().clone()],
            PULSES.map(|pulse| [Segment::from(u64::from(pulse))]),
        )
    };
    for active in [false, true] {
        let unresolved = q(Role::Objective, Asset::Hidden)
            + q(Role::Objective, Asset::Located)
            + q(Role::Objective, Asset::Working)
            + q(Role::Objective, Asset::Carried);
        let mut clinical = each_objective(
            event().preserve(Role::World, basket([(Asset::EnvironmentReady, 1)])),
            &objective_id,
            objectives(patient),
        )
        .variable(day.range(PULSES[0], PULSES[2]))
        .relation(pulse_days())
        .condition(
            path!("clinical-pulse-time-reached"),
            clock(),
            C::GreaterThanOrEqual,
            day.quantity(),
        )
        .condition(
            path!("scheduled-pulse"),
            q(Role::World, Asset::NextClinical),
            C::Equal,
            day.quantity(),
        )
        .consume_at(
            Role::Objective,
            one([AssetPattern::ClinicalDue(day.var()).into()]),
        )
        .produce_at(
            Role::Objective,
            one([AssetPattern::ClinicalSettled(day.var()).into()]),
        )
        .produce(Role::World, basket([(Asset::ClinicalCount, 1)]))
        .condition(
            path!("match-living-unresolved-patient"),
            unresolved * Q::minimum(q(Role::Objective, Asset::Health), Q::constant(1)),
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
        builder = builder.rate_at(
            RateIdPattern::Clinical {
                objective: objective_id.var(),
                day: day.var(),
                active: Slot::Is(active),
            }
            .into(),
            clinical,
        );
    }
    builder = builder.rate_at(
        RateIdPattern::Survey(location.var()).into(),
        work(Agent::Scout, 1)
            .variable(site())
            .preserve_at(Role::Scout, one([at(&location)]))
            .consume_at(
                Role::Scout,
                one([AssetPattern::Unsearched(location.var()).into()]),
            )
            .produce_at(
                Role::Scout,
                one([AssetPattern::Surveyed(location.var()).into()]),
            ),
    );
    builder = builder.rate_at(
        RateIdPattern::Report {
            round: round.var(),
            location: location.var(),
        }
        .into(),
        nature(law())
            .variable(round.range(0, 1))
            .variable(site())
            .derive(next.name().clone(), round.quantity() + Q::constant(1))
            .preserve_at(
                Role::Nature,
                one([AssetPattern::Signal(round.var(), location.var()).into()]),
            )
            .consume_at(
                Role::Scout,
                one([
                    Pattern::literal(Asset::AwaitingReport),
                    AssetPattern::ScanRound(round.var()).into(),
                ]),
            )
            .produce_at(
                Role::Scout,
                one([
                    Pattern::literal(Asset::Ready),
                    AssetPattern::ScanRound(next.var()).into(),
                    AssetPattern::Hint(round.var(), location.var()).into(),
                ]),
            ),
    );
    // The hazard site's weather closes, and later opens again.
    for opening in [false, true] {
        let (before, after) = if opening {
            (
                AssetPattern::WeatherBlocked(location.var()),
                AssetPattern::WeatherOpen(location.var()),
            )
        } else {
            (
                AssetPattern::WeatherOpen(location.var()),
                AssetPattern::WeatherBlocked(location.var()),
            )
        };
        builder = builder.rate_at(
            RateIdPattern::Weather {
                location: location.var(),
                opening: Slot::Is(opening),
            }
            .into(),
            nature(event())
                .variable(site())
                .preserve_at(
                    Role::Nature,
                    one([AssetPattern::HazardSite(location.var()).into()]),
                )
                .consume_at(
                    Role::World,
                    one([Pattern::literal(Asset::WeatherDue(opening)), before.into()]),
                )
                .produce_at(
                    Role::World,
                    one([Pattern::literal(Asset::EnvironmentReady), after.into()]),
                ),
        );
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
    // The clock steps a day while the environment is settled and the
    // clinical pulse due has settled. The day the weather closes in and
    // the day it clears, the step leaves the environment unsettled, the
    // weather falling due: a relation over the days gives which.
    let (closes, clears) = (
        Param::<u64>::new(path!("weather-closes")),
        Param::<u64>::new(path!("weather-clears")),
    );
    let weather = closes.quantity() + clears.quantity();
    let mut tick = pulse_settled(event())
        .variable(day.range(0, horizon as u8 - 1))
        .relation(Relation::each_giving(
            &day,
            [closes.name().clone(), clears.name().clone()],
            (0..horizon as u8).map(|day| {
                (
                    day,
                    [
                        u64::from(day + 1 == WEATHER[0]),
                        u64::from(day + 1 == WEATHER[1]),
                    ],
                )
            }),
        ))
        .condition(path!("clock-step"), clock(), C::Equal, day.quantity())
        .preserve_computed(
            Role::World,
            Asset::EnvironmentReady,
            Q::constant(1) - weather.clone(),
        )
        .consume_computed(Role::World, Asset::EnvironmentReady, weather)
        .consume(Role::World, basket([(Asset::TimeRemaining, 1)]))
        .produce(Role::World, basket([(Asset::Time, 1)]))
        .produce_computed(Role::World, Asset::WeatherDue(false), closes.quantity())
        .produce_computed(Role::World, Asset::WeatherDue(true), clears.quantity());
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
    builder = builder.rate_at(RateIdPattern::Tick(day.var()).into(), tick);
    // A pulse ends once every casualty is assessed, scheduling the next
    // pulse as many days on as a relation over the pulses gives, the last
    // scheduling none.
    let gap = Param::<u64>::new(path!("days-to-next-pulse"));
    builder = builder.rate_at(
        RateIdPattern::FinishClinical(day.var()).into(),
        event()
            .variable(day.range(PULSES[0], PULSES[2]))
            .relation(Relation::each_giving(
                &day,
                [gap.name().clone()],
                PULSES.iter().enumerate().map(|(index, &pulse)| {
                    let next = PULSES
                        .get(index + 1)
                        .map_or(LAST_PULSE_GAP, |&next| u64::from(next - pulse));
                    (pulse, [next])
                }),
            ))
            .condition(
                path!("scheduled-pulse"),
                q(Role::World, Asset::NextClinical),
                C::Equal,
                day.quantity(),
            )
            .condition(
                path!("all-casualties-assessed"),
                q(Role::World, Asset::ClinicalCount),
                C::Equal,
                Q::constant(u64::from(count.div_ceil(2))),
            )
            .consume_computed(
                Role::World,
                Asset::ClinicalCount,
                q(Role::World, Asset::ClinicalCount),
            )
            .produce_computed(Role::World, Asset::NextClinical, gap.quantity()),
    );
    for day in PULSES {
        builder = builder.invariant(
            LinearInvariant::new(path!("clinical-pulse").join(&path::write(&day)))
                .weight(Asset::ClinicalDue(day), 1)
                .weight(Asset::ClinicalSettled(day), 1),
        );
    }
    let mut activity = LinearInvariant::new(path!("two-independent-agent-activities"))
        .weight(Asset::Ready, 1)
        .weight(Asset::AwaitingReport, 1);
    let mut positions = LinearInvariant::new(path!("two-physical-agent-positions"));
    let mut surveys = LinearInvariant::new(path!("finite-sector-surveys"));
    let mut seat = LinearInvariant::new(path!("one-medic-evacuation-seat")).weight(Asset::Seat, 1);
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
            LinearInvariant::new(path!("one-decision-per-tick").join(&path::write(&agent)))
                .weight(Asset::Permit(agent), 1)
                .weight(Asset::UsedPermit(agent), 1),
        );
    }
    builder
        .rate(
            RateId::Close,
            identity()
                .condition(
                    path!("all-objectives-resolved"),
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
            LinearInvariant::new(path!("objective-lifecycle"))
                .weight(Asset::Hidden, 1)
                .weight(Asset::Located, 1)
                .weight(Asset::Working, 1)
                .weight(Asset::Carried, 1)
                .weight(Asset::Complete, 1)
                .weight(Asset::Failed, 1),
        )
        .invariant(
            LinearInvariant::new(path!("health-and-harm"))
                .weight(Asset::Health, 1)
                .weight(Asset::Harm, 1),
        )
        .invariant(
            LinearInvariant::new(path!("stabilization-state"))
                .weight(Asset::Stable, 1)
                .weight(Asset::Unstable, 1),
        )
        .invariant(
            LinearInvariant::new(path!("energy"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("medical-kits"))
                .weight(Asset::MedicalKit, 1)
                .weight(Asset::UsedMedicalKit, 1),
        )
        .invariant(
            LinearInvariant::new(path!("radio-charges"))
                .weight(Asset::Radio, 1)
                .weight(Asset::UsedRadio, 1),
        )
        .invariant(
            LinearInvariant::new(path!("sensor-charges"))
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-resupply"))
                .weight(Asset::Supply, 1)
                .weight(Asset::UsedSupply, 1),
        )
        .invariant(
            LinearInvariant::new(path!("time"))
                .weight(Asset::Time, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new(path!("mission-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Closed, 1),
        )
        .invariant(
            LinearInvariant::new(path!("weather-settlement"))
                .weight(Asset::EnvironmentReady, 1)
                .weight(Asset::WeatherDue(false), 1)
                .weight(Asset::WeatherDue(true), 1),
        )
        .build()
        .expect("expedition resources and stages conserved")
}

/// The exchange of `rate` once, each of its roles bound to the account
/// `world`'s rate fixes it to.
///
/// # Panics
///
/// If `rate` names no instance of `world`'s rates.
pub fn action(world: &World, rate: RateId) -> Action {
    let instance = world
        .instance(&rate)
        .unwrap_or_else(|_| panic!("the expedition has no rate {rate:?}"));
    instance.fixed_accounts().fold(
        Exchange::new(rate, Quantity::new(1)),
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
    Goal::new().require(AccountId::World, basket([(Asset::Closed, 1)]))
}
/// The most steps an expedition takes before it is cut short.
pub const HORIZON: usize = 500;

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_all(world: &mut World, rates: &[RateId]) {
        for rate in rates {
            world
                .apply(action(world, *rate))
                .unwrap_or_else(|error| panic!("{rate:?}: {error}"));
        }
    }

    #[test]
    fn hidden_scenarios_differ_only_in_what_the_agents_cannot_see() {
        let a = initial(false, 0);
        let b = initial(false, 17);
        assert_ne!(a.state_key(), b.state_key());
        for agent in AGENTS {
            let seen = [AccountId::World, AccountId::Agent(agent)];
            assert_eq!(
                a.view(seen).observation_key(),
                b.view(seen).observation_key()
            );
        }
    }
    #[test]
    fn base_resupply_does_not_teleport_medicine_to_an_absent_medic() {
        let mut world = initial(false, 17);
        world
            .apply(action(
                &world,
                RateId::Depart {
                    agent: Agent::Medic,
                    from: Location::Base,
                    to: Location::North,
                },
            ))
            .unwrap();
        world
            .apply(action(&world, RateId::Resupply(Agent::Scout)))
            .unwrap();
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Medic), Asset::MedicalKit),
            2
        );
        assert_eq!(value(&world, AccountId::World, Asset::MedicalKit), 2);
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Scout), Asset::Energy),
            20
        );
        assert!(!world.is_applicable(&action(&world, RateId::Resupply(Agent::Medic))));
    }
    #[test]
    fn private_discovery_requires_a_real_briefing_before_joint_extraction() {
        let east = Location::East;
        let known =
            |world: &World, agent| value(world, AccountId::Agent(agent), Asset::Known(0, east));
        let mut world = initial(false, 17);
        apply_all(
            &mut world,
            &[
                RateId::Scan,
                RateId::Report {
                    round: 0,
                    location: east,
                },
                RateId::Tick(0),
                RateId::Depart {
                    agent: Agent::Scout,
                    from: Location::Base,
                    to: east,
                },
                RateId::Tick(1),
                RateId::Tick(2),
                RateId::Arrive {
                    agent: Agent::Scout,
                    to: east,
                    hazardous: false,
                },
                RateId::Survey(east),
                RateId::Discover {
                    objective: 0,
                    location: east,
                },
            ],
        );
        let extract = action(
            &world,
            RateId::Extract {
                objective: 0,
                location: east,
            },
        );
        assert_eq!(known(&world, Agent::Scout), 1);
        assert_eq!(known(&world, Agent::Medic), 0);
        assert!(!world.is_applicable(&extract));

        apply_all(
            &mut world,
            &[
                RateId::Tick(3),
                RateId::Share {
                    objective: 0,
                    location: east,
                    remote: true,
                },
            ],
        );
        assert_eq!(known(&world, Agent::Medic), 1);
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            1
        );

        apply_all(
            &mut world,
            &[
                RateId::Depart {
                    agent: Agent::Medic,
                    from: Location::Base,
                    to: east,
                },
                RateId::Tick(4),
                RateId::Weather {
                    location: east,
                    opening: false,
                },
                RateId::Tick(5),
                RateId::Arrive {
                    agent: Agent::Medic,
                    to: east,
                    hazardous: true,
                },
                RateId::TreatPatient {
                    objective: 0,
                    location: east,
                    restore: false,
                },
                RateId::Tick(6),
            ],
        );
        assert_eq!(value(&world, AccountId::Objective(0), Asset::Stable), 1);
        for agent in AGENTS {
            assert_eq!(value(&world, AccountId::Agent(agent), Asset::At(east)), 1);
        }
        let rebound = extract
            .clone()
            .bind(Role::Medic, AccountId::Agent(Agent::Scout));
        assert!(!world.is_applicable(&rebound));
        world.apply(extract).unwrap();
    }
    #[test]
    fn movement_occupies_agents_and_scheduled_updates_cannot_be_skipped() {
        let mut world = initial(false, 0);
        world
            .apply(action(
                &world,
                RateId::Depart {
                    agent: Agent::Scout,
                    from: Location::Base,
                    to: Location::North,
                },
            ))
            .unwrap();
        world
            .apply(action(
                &world,
                RateId::Depart {
                    agent: Agent::Medic,
                    from: Location::Base,
                    to: Location::South,
                },
            ))
            .unwrap();
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Arrive {
                agent: Agent::Scout,
                to: Location::North,
                hazardous: false
            }
        )));
        assert!(!world.is_applicable(&action(&world, RateId::Survey(Location::North))));
        apply_all(
            &mut world,
            &[
                RateId::Tick(0),
                RateId::Arrive {
                    agent: Agent::Scout,
                    to: Location::North,
                    hazardous: false,
                },
                RateId::Arrive {
                    agent: Agent::Medic,
                    to: Location::South,
                    hazardous: false,
                },
            ],
        );
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
            world.apply(action(&world, RateId::Tick(tick))).unwrap();
        }
        assert!(!world.is_applicable(&action(&world, RateId::Tick(5))));
        assert!(!world.is_applicable(&action(&world, RateId::Survey(Location::North))));
        apply_all(
            &mut world,
            &[RateId::Weather {
                location: Location::North,
                opening: false,
            }],
        );
        for tick in 5..8 {
            world.apply(action(&world, RateId::Tick(tick))).unwrap();
        }
        assert!(!world.is_applicable(&action(&world, RateId::Tick(8))));
        assert!(!world.is_applicable(&action(&world, RateId::Survey(Location::North))));
        apply_all(
            &mut world,
            &[
                RateId::Clinical {
                    objective: 0,
                    day: 8,
                    active: true,
                },
                RateId::FinishClinical(8),
            ],
        );
        assert_eq!(value(&world, AccountId::Objective(0), Asset::Health), 2);
        assert!(world.is_applicable(&action(&world, RateId::Tick(8))));
    }
}
