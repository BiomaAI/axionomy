#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern,
    Quantity, QuantityExpression as Q, Rate, Rounding, Slot, basket,
};

mod continuous;
#[cfg(feature = "packages")]
pub mod package;

pub use continuous::clock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AgentId {
    Atlas,
    Bolt,
    Coda,
    Delta,
}

pub const AGENTS: [AgentId; 4] = [AgentId::Atlas, AgentId::Bolt, AgentId::Coda, AgentId::Delta];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub struct JobId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Depot,
    North,
    East,
    South,
    West,
    Workshop,
    Charger,
    Recycler,
}

pub const LOCATIONS: [Location; 8] = [
    Location::Depot,
    Location::North,
    Location::East,
    Location::South,
    Location::West,
    Location::Workshop,
    Location::Charger,
    Location::Recycler,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Facility {
    Workshop,
    Charger,
    Recycler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum WorkMode {
    Rush,
    Lean,
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Outcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Policy {
    Sprinter,
    Steward,
    ValueHunter,
    Resilient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Agent(AgentId),
    Job(JobId),
    Facility(Facility),
    Nature,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    SeasonActive,
    SeasonEnded,
    Tick,
    Horizon,
    ContractsOpen,
    ContractsClosed,
    Activities,
    Permit(AgentId),
    Acted(AgentId),
    Idle,
    Busy,
    FreeContract,
    ReadyAt,
    ReleaseAt,
    Deadline,
    LeaseUntil,
    Expired,
    Transferable,
    HandedOff,
    Assists,
    CompletedJob(JobId),
    Credit,
    Wear,
    Repairing,
    Vacant(Facility),
    Occupied(Facility, AgentId),
    Servicing(Facility),
    ChargingEnergy,
    ReclaimingMaterial,
    EmbodiedMaterial,
    RecyclingLoss,
    Policy(Policy),
    /// Where a worker stands, or where a contract's work is done.
    At(Location),
    /// What a contract pays on delivery: a fact of the contract.
    Reward,
    /// The energy, time and material a contract's work takes before its
    /// mode's adjustments: facts of the contract.
    WorkEnergy,
    WorkTime,
    WorkMaterial,
    Operational,
    Energy,
    TimeRemaining,
    Material,
    Available,
    Assigned(AgentId),
    Claimed(JobId),
    Pending,
    InProgress,
    Awaiting(JobId, WorkMode),
    Resolved(JobId, WorkMode, Outcome),
    Completed,
    Value,
    Attempts,
    Successes,
    Failures,
    SpentEnergy,
    ElapsedTime,
    MaterialSpent,
    Waste,
    RecycledWaste,
    Damage,
    RepairSupply,
    SpentRepairSupply,
    ChargeSupply,
    RecyclerCapacity,
    OutcomeWeight(JobId, WorkMode, Outcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Agent,
    Job,
    Nature,
    Facility,
    World,
    Partner,
    Worker(AgentId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Tick,
    Wait {
        agent: AgentId,
    },
    BeginService {
        agent: AgentId,
        facility: Facility,
        maintenance: bool,
    },
    EndService {
        agent: AgentId,
        facility: Facility,
    },
    Handoff {
        agent: AgentId,
        recipient: AgentId,
        job: JobId,
    },
    ReleaseClaim {
        agent: AgentId,
        job: JobId,
        expired: bool,
    },
    Expire {
        job: JobId,
        agent: Option<AgentId>,
        mode: Option<WorkMode>,
    },
    EndSeason,
    Claim {
        agent: AgentId,
        job: JobId,
    },
    Move {
        agent: AgentId,
        from: Location,
        to: Location,
    },
    Begin {
        agent: AgentId,
        job: JobId,
        mode: WorkMode,
    },
    Resolve {
        agent: AgentId,
        job: JobId,
        mode: WorkMode,
        outcome: Outcome,
    },
    Finish {
        agent: AgentId,
        job: JobId,
        mode: WorkMode,
        outcome: Outcome,
    },
    Repair {
        agent: AgentId,
    },
    Recharge {
        agent: AgentId,
    },
    Recycle {
        agent: AgentId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Micro,
    Showcase,
    Stress,
}

impl Profile {
    pub const fn agent_count(self) -> usize {
        match self {
            Self::Micro => 2,
            Self::Showcase | Self::Stress => 4,
        }
    }

    pub const fn job_count(self) -> usize {
        match self {
            Self::Micro => 4,
            Self::Showcase => 12,
            Self::Stress => 24,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSpec {
    pub id: JobId,
    pub location: Location,
    pub value: u64,
    pub energy: u64,
    pub time: u64,
    pub material: u64,
    pub risk: u64,
}

#[derive(Debug, Clone)]
pub struct League {
    initial: World,
    goal: Goal<Ontology>,
    agents: Vec<AgentId>,
    jobs: Vec<JobSpec>,
}

impl League {
    pub const fn initial(&self) -> &World {
        &self.initial
    }
    pub const fn goal(&self) -> &Goal<Ontology> {
        &self.goal
    }
    pub fn agents(&self) -> &[AgentId] {
        &self.agents
    }
    pub fn jobs(&self) -> &[JobSpec] {
        &self.jobs
    }
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `work-league` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "work-league";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn mixed_lineup() -> [Policy; 4] {
    [
        Policy::Sprinter,
        Policy::Steward,
        Policy::ValueHunter,
        Policy::Resilient,
    ]
}

pub fn throughput_lineup() -> [Policy; 4] {
    [
        Policy::Sprinter,
        Policy::Sprinter,
        Policy::ValueHunter,
        Policy::Sprinter,
    ]
}

pub fn sustainable_lineup() -> [Policy; 4] {
    [
        Policy::Steward,
        Policy::Steward,
        Policy::Resilient,
        Policy::Steward,
    ]
}

/// The worker a rate's identifier names, `{agent}`.
fn agent() -> Param<AgentId> {
    Param::new(path!("agent"))
}

/// The number of the contract a rate's identifier names, `{job}`.
fn job() -> Param<u8> {
    Param::new(path!("job"))
}

/// The mode a rate's identifier names, `{mode}`.
fn mode() -> Param<WorkMode> {
    Param::new(path!("mode"))
}

/// The contract `{job}` names.
fn contract() -> Slot<JobId> {
    Slot::Of(JobIdPattern(job().var()))
}

/// One unit of each asset `assets` names.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}

/// `rate` for each worker of `agents`, `{agent}`, its agent role fixed to
/// that worker's account.
fn by_worker(rate: Rate<Ontology>, agents: &[AgentId]) -> Rate<Ontology> {
    rate.variable(agent().values(agents.iter().copied()))
        .account(Role::Agent, AccountIdPattern::Agent(agent().var()).into())
}

/// `rate` for each of the `count` contracts, `{job}`, its job role fixed to
/// that contract's account.
fn on_contract(rate: Rate<Ontology>, count: u8) -> Rate<Ontology> {
    rate.variable(job().range(1, count))
        .account(Role::Job, AccountIdPattern::Job(contract()).into())
}

/// A contract's terms, the facts its laws read: where its work is done,
/// what it pays, and the energy, time and material its work takes.
fn terms(spec: &JobSpec) -> Basket<Asset> {
    basket([
        (Asset::At(spec.location), 1),
        (Asset::Reward, spec.value),
        (Asset::WorkEnergy, spec.energy),
        (Asset::WorkTime, spec.time),
        (Asset::WorkMaterial, spec.material),
    ])
}

/// The worker stands where the contract's work is done: it holds each place
/// as the contract does.
fn at_the_contract(rate: Rate<Ontology>) -> Rate<Ontology> {
    LOCATIONS.into_iter().fold(rate, |rate, place| {
        rate.preserve_computed(
            Role::Agent,
            Asset::At(place),
            Q::balance(Role::Job, Asset::At(place)),
        )
    })
}

/// What working a contract in a mode takes and leaves behind, from the
/// contract's terms: the law of the mode.
struct Cost {
    energy: Q<Ontology>,
    time: Q<Ontology>,
    material: Q<Ontology>,
    waste: Q<Ontology>,
}

/// Rush spends two more energy and one more material to finish a tick
/// sooner, leaving two waste; lean takes two ticks longer and leaves waste
/// on odd contracts; safe spends one more energy and one more tick, leaving
/// one.
fn cost(mode: WorkMode) -> Cost {
    let term = |asset| Q::balance(Role::Job, asset);
    let (energy, time, material) = (
        term(Asset::WorkEnergy),
        term(Asset::WorkTime),
        term(Asset::WorkMaterial),
    );
    match mode {
        WorkMode::Rush => Cost {
            energy: energy + Q::constant(2),
            time: Q::maximum(time - Q::constant(1), Q::constant(1)),
            material: material + Q::constant(1),
            waste: Q::constant(2),
        },
        WorkMode::Lean => Cost {
            energy,
            time: time + Q::constant(2),
            material,
            // The contract's number's parity.
            waste: job().quantity()
                - Q::divide(job().quantity(), Q::constant(2), Rounding::Floor) * Q::constant(2),
        },
        WorkMode::Safe => Cost {
            energy: energy + Q::constant(1),
            time: time + Q::constant(1),
            material,
            waste: Q::constant(1),
        },
    }
}

/// The exchange of one unit of `rate`, each role bound to the account
/// `world`'s rate fixes it to.
///
/// # Panics
///
/// If `rate` names no instance of `world`'s rates.
pub fn action(world: &World, rate: RateId) -> Action {
    let instance = world
        .instance(&rate)
        .unwrap_or_else(|_| panic!("the league has no rate {rate:?}"));
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

pub fn league(profile: Profile, lineup: [Policy; 4]) -> League {
    if profile != Profile::Micro {
        return continuous::build(profile, lineup);
    }
    let agents = AGENTS[..profile.agent_count()].to_vec();
    let jobs = (0..profile.job_count())
        .map(|index| job_spec(JobId(index as u8 + 1)))
        .collect::<Vec<_>>();
    let mut builder = EconomyBuilder::new();

    for (index, agent) in agents.iter().copied().enumerate() {
        builder = builder.account(
            AccountId::Agent(agent),
            Account::from(basket([
                (Asset::Policy(lineup[index]), 1),
                (Asset::At(Location::Depot), 1),
                (Asset::Operational, 1),
                (Asset::Energy, 96),
                (Asset::TimeRemaining, 220),
                (Asset::Material, 64),
            ])),
        );
    }

    let mut nature = vec![];
    for spec in &jobs {
        for mode in [WorkMode::Rush, WorkMode::Lean, WorkMode::Safe] {
            let failure = failure_weight(spec.risk, mode);
            nature.push((
                Asset::OutcomeWeight(spec.id, mode, Outcome::Failure),
                failure,
            ));
            nature.push((
                Asset::OutcomeWeight(spec.id, mode, Outcome::Success),
                10 - failure,
            ));
        }
    }
    builder = builder
        .account(
            AccountId::Nature,
            Account::from(
                Basket::try_from_entries(
                    nature
                        .into_iter()
                        .map(|(asset, quantity)| (asset, Quantity::new(quantity))),
                )
                .expect("nature weights are unique"),
            ),
        )
        .account(
            AccountId::Facility(Facility::Workshop),
            Account::from(basket([(
                Asset::RepairSupply,
                (profile.job_count() * 2) as u64,
            )])),
        )
        .account(
            AccountId::Facility(Facility::Charger),
            Account::from(basket([(
                Asset::ChargeSupply,
                (profile.job_count() * 20) as u64,
            )])),
        )
        .account(
            AccountId::Facility(Facility::Recycler),
            Account::from(basket([(Asset::RecyclerCapacity, 1)])),
        );

    let mut goal = Goal::new();
    for spec in &jobs {
        let mut contract = terms(spec);
        contract.insert(Asset::Available, Quantity::new(1));
        builder = builder.account(AccountId::Job(spec.id), Account::from(contract));
        goal = goal.require(AccountId::Job(spec.id), basket([(Asset::Completed, 1)]));
    }

    // Each rate serves every worker, `{agent}`, and every contract, `{job}`,
    // its roles fixed to their accounts.
    let count = jobs.len() as u8;
    let worker = |rate| by_worker(rate, &agents);
    let outcome = Param::<Outcome>::new(path!("outcome"));
    let assigned = || AssetPattern::Assigned(agent().var()).into();
    let claimed = || AssetPattern::Claimed(contract()).into();
    builder = builder.rate_at(
        RateIdPattern::Claim {
            agent: agent().var(),
            job: contract(),
        }
        .into(),
        on_contract(worker(Rate::new()), count)
            .produce_at(Role::Agent, one([claimed()]))
            .consume(Role::Job, basket([(Asset::Available, 1)]))
            .produce_at(
                Role::Job,
                one([assigned(), Pattern::literal(Asset::Pending)]),
            ),
    );
    for mode in [WorkMode::Rush, WorkMode::Lean, WorkMode::Safe] {
        let cost = cost(mode);
        builder = builder.rate_at(
            RateIdPattern::Begin {
                agent: agent().var(),
                job: contract(),
                mode: Slot::Is(mode),
            }
            .into(),
            at_the_contract(on_contract(worker(Rate::new()), count))
                .preserve(Role::Agent, basket([(Asset::Operational, 1)]))
                .preserve_at(Role::Agent, one([claimed()]))
                .consume_computed(Role::Agent, Asset::Energy, cost.energy.clone())
                .consume_computed(Role::Agent, Asset::TimeRemaining, cost.time.clone())
                .consume_computed(Role::Agent, Asset::Material, cost.material.clone())
                .produce_computed(Role::Agent, Asset::SpentEnergy, cost.energy)
                .produce_computed(Role::Agent, Asset::ElapsedTime, cost.time)
                .produce_computed(Role::Agent, Asset::MaterialSpent, cost.material)
                .produce_computed(Role::Agent, Asset::Waste, cost.waste)
                .produce(Role::Agent, basket([(Asset::Attempts, 1)]))
                .produce_at(
                    Role::Agent,
                    one([AssetPattern::Awaiting(contract(), Slot::Is(mode)).into()]),
                )
                .preserve_at(Role::Job, one([assigned()]))
                .consume(Role::Job, basket([(Asset::Pending, 1)]))
                .produce(Role::Job, basket([(Asset::InProgress, 1)])),
        );
    }
    // Nature resolves an attempt by its weight of the outcome, of the
    // contract in the mode it was worked in.
    builder = builder.rate_at(
        RateIdPattern::Resolve {
            agent: agent().var(),
            job: contract(),
            mode: mode().var(),
            outcome: outcome.var(),
        }
        .into(),
        worker(Rate::new())
            .variable(job().range(1, count))
            .variable(mode().every())
            .variable(outcome.every())
            .account(Role::Nature, Pattern::literal(AccountId::Nature))
            .consume_at(
                Role::Agent,
                one([AssetPattern::Awaiting(contract(), mode().var()).into()]),
            )
            .produce_at(
                Role::Agent,
                one([AssetPattern::Resolved(contract(), mode().var(), outcome.var()).into()]),
            )
            .preserve_at(
                Role::Nature,
                one([AssetPattern::OutcomeWeight(contract(), mode().var(), outcome.var()).into()]),
            ),
    );
    for result in [Outcome::Success, Outcome::Failure] {
        let resolved = AssetPattern::Resolved(contract(), mode().var(), Slot::Is(result)).into();
        let finish = on_contract(worker(Rate::new()), count).variable(mode().every());
        let finish = match result {
            Outcome::Success => finish
                .consume_at(Role::Agent, one([resolved, claimed()]))
                .produce_computed(
                    Role::Agent,
                    Asset::Value,
                    Q::balance(Role::Job, Asset::Reward),
                )
                .produce(
                    Role::Agent,
                    basket([(Asset::Completed, 1), (Asset::Successes, 1)]),
                )
                .consume_at(
                    Role::Job,
                    one([Pattern::literal(Asset::InProgress), assigned()]),
                )
                .produce(Role::Job, basket([(Asset::Completed, 1)])),
            Outcome::Failure => finish
                .preserve_at(Role::Agent, one([claimed()]))
                .consume_at(
                    Role::Agent,
                    one([resolved, Pattern::literal(Asset::Operational)]),
                )
                .produce(
                    Role::Agent,
                    basket([(Asset::Failures, 1), (Asset::Damage, 1)]),
                )
                .preserve_at(Role::Job, one([assigned()]))
                .consume(Role::Job, basket([(Asset::InProgress, 1)]))
                .produce(Role::Job, basket([(Asset::Pending, 1)])),
        };
        builder = builder.rate_at(
            RateIdPattern::Finish {
                agent: agent().var(),
                job: contract(),
                mode: mode().var(),
                outcome: Slot::Is(result),
            }
            .into(),
            finish,
        );
    }

    // A worker moves between any two places, a tick and an energy a move.
    let (from, to) = (
        Param::<Location>::new(path!("from")),
        Param::new(path!("to")),
    );
    builder = builder.rate_at(
        RateIdPattern::Move {
            agent: agent().var(),
            from: from.var(),
            to: to.var(),
        }
        .into(),
        worker(Rate::new())
            .variable(from.every())
            .variable(to.every())
            .distinct_variables(from.name().clone(), to.name().clone())
            .consume_at(
                Role::Agent,
                [
                    (AssetPattern::At(from.var()).into(), Quantity::new(1)),
                    (Pattern::literal(Asset::Energy), Quantity::new(1)),
                    (Pattern::literal(Asset::TimeRemaining), Quantity::new(1)),
                ],
            )
            .produce_at(
                Role::Agent,
                [
                    (AssetPattern::At(to.var()).into(), Quantity::new(1)),
                    (Pattern::literal(Asset::SpentEnergy), Quantity::new(1)),
                    (Pattern::literal(Asset::ElapsedTime), Quantity::new(1)),
                ],
            ),
    );
    // Each service at its facility, the facility role fixed to it.
    let serviced = |facility, rate: Rate<Ontology>| {
        worker(rate)
            .account(
                Role::Facility,
                Pattern::literal(AccountId::Facility(facility)),
            )
            .preserve(
                Role::Agent,
                basket([(Asset::At(service_location(facility)), 1)]),
            )
    };
    builder = builder
        .rate_at(
            RateIdPattern::Repair {
                agent: agent().var(),
            }
            .into(),
            serviced(Facility::Workshop, Rate::new())
                .consume(
                    Role::Agent,
                    basket([(Asset::Damage, 1), (Asset::TimeRemaining, 3)]),
                )
                .produce(
                    Role::Agent,
                    basket([(Asset::Operational, 1), (Asset::ElapsedTime, 3)]),
                )
                .consume(Role::Facility, basket([(Asset::RepairSupply, 1)]))
                .produce(Role::Facility, basket([(Asset::SpentRepairSupply, 1)])),
        )
        .rate_at(
            RateIdPattern::Recharge {
                agent: agent().var(),
            }
            .into(),
            serviced(Facility::Charger, Rate::new())
                .consume(Role::Agent, basket([(Asset::TimeRemaining, 2)]))
                .produce(
                    Role::Agent,
                    basket([(Asset::Energy, 12), (Asset::ElapsedTime, 2)]),
                )
                .consume(Role::Facility, basket([(Asset::ChargeSupply, 12)])),
        )
        .rate_at(
            RateIdPattern::Recycle {
                agent: agent().var(),
            }
            .into(),
            serviced(Facility::Recycler, Rate::new())
                .consume(
                    Role::Agent,
                    basket([
                        (Asset::Waste, 1),
                        (Asset::Energy, 1),
                        (Asset::TimeRemaining, 1),
                    ]),
                )
                .produce(
                    Role::Agent,
                    basket([
                        (Asset::RecycledWaste, 1),
                        (Asset::SpentEnergy, 1),
                        (Asset::ElapsedTime, 1),
                    ]),
                )
                .preserve(Role::Facility, basket([(Asset::RecyclerCapacity, 1)])),
        );

    builder = builder
        .invariant(
            LinearInvariant::new(path!("league-energy-accounting"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1)
                .weight(Asset::ChargeSupply, 1),
        )
        .invariant(
            LinearInvariant::new(path!("league-material-accounting"))
                .weight(Asset::Material, 1)
                .weight(Asset::MaterialSpent, 1),
        )
        .invariant(
            LinearInvariant::new(path!("league-time-accounting"))
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        )
        .invariant(
            LinearInvariant::new(path!("worker-operational-condition"))
                .weight(Asset::Operational, 1)
                .weight(Asset::Damage, 1),
        )
        .invariant(
            LinearInvariant::new(path!("repair-supply-accounting"))
                .weight(Asset::RepairSupply, 1)
                .weight(Asset::SpentRepairSupply, 1),
        );

    League {
        initial: builder.build().expect("work league model is valid"),
        goal,
        agents,
        jobs,
    }
}

pub fn job_spec(job: JobId) -> JobSpec {
    let index = u64::from(job.0.saturating_sub(1));
    let locations = [
        Location::North,
        Location::East,
        Location::South,
        Location::West,
    ];
    JobSpec {
        id: job,
        location: locations[index as usize % locations.len()],
        value: 8 + (index * 7 % 17),
        energy: 3 + (index * 3 % 5),
        time: 3 + (index * 3 % 6),
        material: 2 + (index * 2 % 4),
        risk: 1 + (index * 3 % 4),
    }
}

/// Where a facility serves.
const fn service_location(facility: Facility) -> Location {
    match facility {
        Facility::Workshop => Location::Workshop,
        Facility::Charger => Location::Charger,
        Facility::Recycler => Location::Recycler,
    }
}

fn failure_weight(risk: u64, mode: WorkMode) -> u64 {
    match mode {
        WorkMode::Rush => (risk + 2).min(8),
        WorkMode::Lean => risk.min(7),
        WorkMode::Safe => risk.saturating_sub(1).min(4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_binds_the_worker_and_contract_it_names() {
        let league = league(Profile::Micro, mixed_lineup());
        let claim = RateId::Claim {
            agent: AgentId::Atlas,
            job: JobId(1),
        };
        let wrong =
            action(league.initial(), claim).bind(Role::Agent, AccountId::Agent(AgentId::Bolt));
        assert!(!league.initial().is_applicable(&wrong));
        assert!(
            league
                .initial()
                .is_applicable(&action(league.initial(), claim))
        );
    }

    #[test]
    fn work_is_done_where_the_contract_is_at_its_modes_cost() {
        let mut world = league(Profile::Micro, mixed_lineup()).initial().clone();
        let agent = AgentId::Atlas;
        let (north, east) = (job_spec(JobId(1)), job_spec(JobId(2)));
        for rate in [
            RateId::Claim {
                agent,
                job: north.id,
            },
            RateId::Claim {
                agent,
                job: east.id,
            },
            RateId::Move {
                agent,
                from: Location::Depot,
                to: east.location,
            },
        ] {
            world.apply(action(&world, rate)).unwrap();
        }
        let begin = |job, mode| RateId::Begin { agent, job, mode };
        assert!(!world.is_applicable(&action(&world, begin(north.id, WorkMode::Lean))));
        let held = |world: &World, asset| world.balance(&AccountId::Agent(agent), &asset).to_u64();
        let energy = held(&world, Asset::Energy);
        world
            .apply(action(&world, begin(east.id, WorkMode::Lean)))
            .unwrap();
        assert_eq!(held(&world, Asset::Energy), energy - east.energy);
        assert_eq!(held(&world, Asset::ElapsedTime), 1 + east.time + 2);
        // Lean work leaves waste on odd contracts only.
        assert_eq!(held(&world, Asset::Waste), 0);
    }
}
