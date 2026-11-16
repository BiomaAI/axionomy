#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, OneSegment, Param, Pattern,
    Quantity, QuantityComparison, QuantityExpression as Q, Rate, Relation, Rounding, Slot, basket,
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
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    SeasonActive,
    SeasonEnded,
    Tick,
    ContractsOpen,
    ContractsClosed,
    Activities,
    Permit(AgentId),
    Acted(AgentId),
    Idle,
    Busy,
    FreeContract,
    ReadyAt,
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
    /// Where a worker stands.
    At(Location),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Agent,
    Job,
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
    /// A worker beginning a contract's work in a mode, where the contract's
    /// terms say its work is done.
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

/// `rate` for each of `jobs`, `{job}`, its job role fixed to that
/// contract's account, and its terms given by their relation.
fn on_contract(rate: Rate<Ontology>, jobs: &[JobSpec]) -> Rate<Ontology> {
    rate.variable(job().range(1, jobs.len() as u8))
        .account(Role::Job, AccountIdPattern::Job(contract()).into())
        .relation(terms(jobs))
}

/// Where a contract's work is done, `{place}`, which its terms give.
fn place() -> Param<Location> {
    Param::new(path!("place"))
}

/// The numbers a contract's terms give its laws: what it pays on delivery,
/// `{reward}`, and the energy, time and material its work takes before its
/// mode's adjustments.
fn reward() -> Param<u64> {
    Param::new(path!("reward"))
}
fn work_energy() -> Param<u64> {
    Param::new(path!("energy"))
}
fn work_time() -> Param<u64> {
    Param::new(path!("time"))
}
fn work_material() -> Param<u64> {
    Param::new(path!("material"))
}

/// Each of `jobs`' terms: law structure that never changes, a row for each
/// contract giving where its work is done, a place, and its numbers.
fn terms(jobs: &[JobSpec]) -> Relation {
    Relation::giving(
        [job().name().clone()],
        [
            place().name().clone(),
            reward().name().clone(),
            work_energy().name().clone(),
            work_time().name().clone(),
            work_material().name().clone(),
        ],
        jobs.iter().map(|spec| {
            [
                spec.id.0.segment(),
                spec.location.segment(),
                spec.value.into(),
                spec.energy.into(),
                spec.time.into(),
                spec.material.into(),
            ]
        }),
    )
}

/// The worker stands where the contract's work is done, `{place}`, which
/// its terms give.
fn at_the_contract(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.preserve_at(Role::Agent, one([AssetPattern::At(place().var()).into()]))
}

/// Nature's weight of an attempt's outcome, `{weight}`, out of ten: the
/// outcome can happen only with some weight.
fn weight() -> Param<u64> {
    Param::new(path!("weight"))
}

/// Every attempt's case: each of `jobs` worked in each mode with each
/// outcome.
fn cases(jobs: &[JobSpec]) -> impl Iterator<Item = (&JobSpec, WorkMode, Outcome)> {
    jobs.iter().flat_map(|spec| {
        [WorkMode::Rush, WorkMode::Lean, WorkMode::Safe]
            .into_iter()
            .flat_map(move |mode| {
                [Outcome::Success, Outcome::Failure]
                    .into_iter()
                    .map(move |outcome| (spec, mode, outcome))
            })
    })
}

/// The weight of each outcome, `{outcome}`, of an attempt at each of
/// `jobs` in each mode: law structure that never changes, which Nature
/// draws by.
fn odds(jobs: &[JobSpec], outcome: &Param<Outcome>) -> Relation {
    Relation::giving(
        [
            job().name().clone(),
            mode().name().clone(),
            outcome.name().clone(),
        ],
        [weight().name().clone()],
        cases(jobs).map(|(spec, mode, outcome)| {
            [
                spec.id.0.segment(),
                mode.segment(),
                outcome.segment(),
                Quantity::new(outcome_weight(spec, mode, outcome)).into(),
            ]
        }),
    )
}

/// The weight of `outcome` of an attempt at each of `jobs` in each mode,
/// for a law whose identifier writes the outcome out.
fn odds_of(jobs: &[JobSpec], outcome: Outcome) -> Relation {
    Relation::pairs_giving(
        &job(),
        &mode(),
        [weight().name().clone()],
        cases(jobs)
            .filter(|(_, _, case)| *case == outcome)
            .map(|(spec, mode, outcome)| (spec.id.0, mode, [outcome_weight(spec, mode, outcome)])),
    )
}

/// An attempt's outcome happens only where Nature weighs it at all.
fn possible(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.condition(
        path!("the-outcome-can-happen"),
        weight().quantity(),
        QuantityComparison::GreaterThanOrEqual,
        Q::constant(1),
    )
}

/// What working a contract in a mode takes and leaves behind, from the
/// contract's terms: the law of the mode, or the names a beginning's law
/// gives it ([`working`]).
struct Cost {
    energy: Q<Ontology>,
    time: Q<Ontology>,
    material: Q<Ontology>,
    waste: Q<Ontology>,
}

/// What working in a mode takes, as a beginning's law names it: the energy,
/// `{takes-energy}`, the ticks, `{takes-time}`, the material,
/// `{takes-material}`, and the waste it leaves, `{leaves-waste}`.
fn takes_energy() -> Param<u64> {
    Param::new(path!("takes-energy"))
}
fn takes_time() -> Param<u64> {
    Param::new(path!("takes-time"))
}
fn takes_material() -> Param<u64> {
    Param::new(path!("takes-material"))
}
fn leaves_waste() -> Param<u64> {
    Param::new(path!("leaves-waste"))
}

/// `rate`, a beginning of work in `mode`, with what working in that mode
/// takes named as its derived values, and that cost as its amounts and
/// conditions read it: the law says the numbers once, and a policy reads
/// them from the law (`state.values`).
fn working(rate: Rate<Ontology>, mode: WorkMode) -> (Rate<Ontology>, Cost) {
    let cost = cost(mode);
    let rate = rate
        .derive(takes_energy().name().clone(), cost.energy)
        .derive(takes_time().name().clone(), cost.time)
        .derive(takes_material().name().clone(), cost.material)
        .derive(leaves_waste().name().clone(), cost.waste);
    let named = Cost {
        energy: takes_energy().quantity(),
        time: takes_time().quantity(),
        material: takes_material().quantity(),
        waste: leaves_waste().quantity(),
    };
    (rate, named)
}

/// Rush spends two more energy and one more material to finish a tick
/// sooner, leaving two waste; lean takes two ticks longer and leaves waste
/// on odd contracts; safe spends one more energy and one more tick, leaving
/// one.
fn cost(mode: WorkMode) -> Cost {
    let (energy, time, material) = (
        work_energy().quantity(),
        work_time().quantity(),
        work_material().quantity(),
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

    builder = builder
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
        // The recycler holds nothing: its law fixes the account, which no
        // token need prove.
        .account(AccountId::Facility(Facility::Recycler), Account::default());

    let mut goal = Goal::new();
    for spec in &jobs {
        builder = builder.account(
            AccountId::Job(spec.id),
            Account::from(basket([(Asset::Available, 1)])),
        );
        goal = goal.require(AccountId::Job(spec.id), basket([(Asset::Completed, 1)]));
    }

    // Each rate serves every worker, `{agent}`, and every contract, `{job}`,
    // its roles fixed to their accounts.
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
        on_contract(worker(Rate::new()), &jobs)
            .produce_at(Role::Agent, one([claimed()]))
            .consume(Role::Job, basket([(Asset::Available, 1)]))
            .produce_at(
                Role::Job,
                one([assigned(), Pattern::literal(Asset::Pending)]),
            ),
    );
    for mode in [WorkMode::Rush, WorkMode::Lean, WorkMode::Safe] {
        let (begin, cost) = working(on_contract(worker(Rate::new()), &jobs), mode);
        builder = builder.rate_at(
            RateIdPattern::Begin {
                agent: agent().var(),
                job: contract(),
                mode: Slot::Is(mode),
            }
            .into(),
            at_the_contract(begin)
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
        possible(worker(Rate::new()))
            .variable(job().range(1, jobs.len() as u8))
            .variable(mode().every())
            .variable(outcome.every())
            .relation(odds(&jobs, &outcome))
            .consume_at(
                Role::Agent,
                one([AssetPattern::Awaiting(contract(), mode().var()).into()]),
            )
            .produce_at(
                Role::Agent,
                one([AssetPattern::Resolved(contract(), mode().var(), outcome.var()).into()]),
            ),
    );
    for result in [Outcome::Success, Outcome::Failure] {
        let resolved = AssetPattern::Resolved(contract(), mode().var(), Slot::Is(result)).into();
        let finish = on_contract(worker(Rate::new()), &jobs).variable(mode().every());
        let finish = match result {
            Outcome::Success => finish
                .consume_at(Role::Agent, one([resolved, claimed()]))
                .produce_computed(Role::Agent, Asset::Value, reward().quantity())
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
                ),
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

/// Nature's weight, out of ten, of `outcome` of an attempt at `spec` in
/// `mode`.
fn outcome_weight(spec: &JobSpec, mode: WorkMode, outcome: Outcome) -> u64 {
    let failure = failure_weight(spec.risk, mode);
    match outcome {
        Outcome::Failure => failure,
        Outcome::Success => 10 - failure,
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
