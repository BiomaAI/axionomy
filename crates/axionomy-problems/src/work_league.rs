//! A competitive multi-agent work league encoded entirely as economic state.
//!
//! Agents contend for a finite pool of jobs. Every claim, movement, attempt,
//! chance resolution, repair, recharge, and recycling operation is an atomic
//! exchange. Policies only propose those exchanges; they cannot bypass the
//! economy or manufacture a score.

use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, Quantity, Rate, Trace, basket,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentId {
    Atlas,
    Bolt,
    Coda,
    Delta,
}

pub const AGENTS: [AgentId; 4] = [AgentId::Atlas, AgentId::Bolt, AgentId::Coda, AgentId::Delta];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Facility {
    Workshop,
    Charger,
    Recycler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WorkMode {
    Rush,
    Lean,
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Outcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Policy {
    Sprinter,
    Steward,
    ValueHunter,
    Resilient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Agent(AgentId),
    Job(JobId),
    Facility(Facility),
    Nature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    AgentIdentity(AgentId),
    JobIdentity(JobId),
    Policy(Policy),
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
    RecyclerCapacity,
    OutcomeWeight(JobId, WorkMode, Outcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Agent,
    Job,
    Nature,
    Facility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
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
    goal: Goal<AccountId, Asset>,
    agents: Vec<AgentId>,
    jobs: Vec<JobSpec>,
}

impl League {
    pub const fn initial(&self) -> &World {
        &self.initial
    }
    pub const fn goal(&self) -> &Goal<AccountId, Asset> {
        &self.goal
    }
    pub fn agents(&self) -> &[AgentId] {
        &self.agents
    }
    pub fn jobs(&self) -> &[JobSpec] {
        &self.jobs
    }
}

#[derive(Debug, Clone)]
pub struct MatchOutcome {
    trace: Trace<RateId, Role, AccountId>,
    final_world: World,
}

impl MatchOutcome {
    pub const fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }
    pub const fn final_world(&self) -> &World {
        &self.final_world
    }
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;

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

pub fn league(profile: Profile, lineup: [Policy; 4]) -> League {
    let agents = AGENTS[..profile.agent_count()].to_vec();
    let jobs = (0..profile.job_count())
        .map(|index| job_spec(JobId(index as u8 + 1)))
        .collect::<Vec<_>>();
    let mut builder = EconomyBuilder::new();

    for (index, agent) in agents.iter().copied().enumerate() {
        builder = builder.account(
            AccountId::Agent(agent),
            Account::from(basket([
                (Asset::AgentIdentity(agent), 1),
                (Asset::Policy(lineup[index]), 1),
                (Asset::At(Location::Depot), 1),
                (Asset::Operational, 1),
                (
                    Asset::Energy,
                    if profile == Profile::Stress { 180 } else { 96 },
                ),
                (
                    Asset::TimeRemaining,
                    if profile == Profile::Stress { 420 } else { 220 },
                ),
                (
                    Asset::Material,
                    if profile == Profile::Stress { 120 } else { 64 },
                ),
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
        builder = builder.account(
            AccountId::Job(spec.id),
            Account::from(basket([
                (Asset::JobIdentity(spec.id), 1),
                (Asset::Available, 1),
            ])),
        );
        goal = goal.require(AccountId::Job(spec.id), basket([(Asset::Completed, 1)]));

        for agent in agents.iter().copied() {
            builder = builder.rate(
                RateId::Claim {
                    agent,
                    job: spec.id,
                },
                Rate::new()
                    .preserve(Role::Agent, basket([(Asset::AgentIdentity(agent), 1)]))
                    .produce(Role::Agent, basket([(Asset::Claimed(spec.id), 1)]))
                    .preserve(Role::Job, basket([(Asset::JobIdentity(spec.id), 1)]))
                    .consume(Role::Job, basket([(Asset::Available, 1)]))
                    .produce(
                        Role::Job,
                        basket([(Asset::Assigned(agent), 1), (Asset::Pending, 1)]),
                    )
                    .distinct(Role::Agent, Role::Job),
            );
            for mode in [WorkMode::Rush, WorkMode::Lean, WorkMode::Safe] {
                let cost = work_cost(*spec, mode);
                builder = builder.rate(
                    RateId::Begin {
                        agent,
                        job: spec.id,
                        mode,
                    },
                    Rate::new()
                        .preserve(
                            Role::Agent,
                            basket([
                                (Asset::AgentIdentity(agent), 1),
                                (Asset::Operational, 1),
                                (Asset::At(spec.location), 1),
                                (Asset::Claimed(spec.id), 1),
                            ]),
                        )
                        .consume(
                            Role::Agent,
                            basket([
                                (Asset::Energy, cost.energy),
                                (Asset::TimeRemaining, cost.time),
                                (Asset::Material, cost.material),
                            ]),
                        )
                        .produce(
                            Role::Agent,
                            basket([
                                (Asset::SpentEnergy, cost.energy),
                                (Asset::ElapsedTime, cost.time),
                                (Asset::MaterialSpent, cost.material),
                                (Asset::Waste, cost.waste),
                                (Asset::Attempts, 1),
                                (Asset::Awaiting(spec.id, mode), 1),
                            ]),
                        )
                        .preserve(
                            Role::Job,
                            basket([
                                (Asset::JobIdentity(spec.id), 1),
                                (Asset::Assigned(agent), 1),
                            ]),
                        )
                        .consume(Role::Job, basket([(Asset::Pending, 1)]))
                        .produce(Role::Job, basket([(Asset::InProgress, 1)]))
                        .distinct(Role::Agent, Role::Job),
                );
                for outcome in [Outcome::Success, Outcome::Failure] {
                    builder = builder.rate(
                        RateId::Resolve {
                            agent,
                            job: spec.id,
                            mode,
                            outcome,
                        },
                        Rate::new()
                            .preserve(Role::Agent, basket([(Asset::AgentIdentity(agent), 1)]))
                            .consume(Role::Agent, basket([(Asset::Awaiting(spec.id, mode), 1)]))
                            .produce(
                                Role::Agent,
                                basket([(Asset::Resolved(spec.id, mode, outcome), 1)]),
                            )
                            .preserve(
                                Role::Nature,
                                basket([(Asset::OutcomeWeight(spec.id, mode, outcome), 1)]),
                            )
                            .distinct(Role::Agent, Role::Nature),
                    );
                    let finish = match outcome {
                        Outcome::Success => Rate::new()
                            .preserve(Role::Agent, basket([(Asset::AgentIdentity(agent), 1)]))
                            .consume(
                                Role::Agent,
                                basket([
                                    (Asset::Resolved(spec.id, mode, outcome), 1),
                                    (Asset::Claimed(spec.id), 1),
                                ]),
                            )
                            .produce(
                                Role::Agent,
                                basket([
                                    (Asset::Value, spec.value),
                                    (Asset::Completed, 1),
                                    (Asset::Successes, 1),
                                ]),
                            )
                            .preserve(Role::Job, basket([(Asset::JobIdentity(spec.id), 1)]))
                            .consume(
                                Role::Job,
                                basket([(Asset::InProgress, 1), (Asset::Assigned(agent), 1)]),
                            )
                            .produce(Role::Job, basket([(Asset::Completed, 1)]))
                            .distinct(Role::Agent, Role::Job),
                        Outcome::Failure => Rate::new()
                            .preserve(
                                Role::Agent,
                                basket([
                                    (Asset::AgentIdentity(agent), 1),
                                    (Asset::Claimed(spec.id), 1),
                                ]),
                            )
                            .consume(
                                Role::Agent,
                                basket([
                                    (Asset::Resolved(spec.id, mode, outcome), 1),
                                    (Asset::Operational, 1),
                                ]),
                            )
                            .produce(
                                Role::Agent,
                                basket([(Asset::Failures, 1), (Asset::Damage, 1)]),
                            )
                            .preserve(
                                Role::Job,
                                basket([
                                    (Asset::JobIdentity(spec.id), 1),
                                    (Asset::Assigned(agent), 1),
                                ]),
                            )
                            .consume(Role::Job, basket([(Asset::InProgress, 1)]))
                            .produce(Role::Job, basket([(Asset::Pending, 1)]))
                            .distinct(Role::Agent, Role::Job),
                    };
                    builder = builder.rate(
                        RateId::Finish {
                            agent,
                            job: spec.id,
                            mode,
                            outcome,
                        },
                        finish,
                    );
                }
            }
        }
    }

    for agent in agents.iter().copied() {
        for from in LOCATIONS {
            for to in LOCATIONS {
                if from == to {
                    continue;
                }
                builder = builder.rate(
                    RateId::Move { agent, from, to },
                    Rate::new()
                        .preserve(Role::Agent, basket([(Asset::AgentIdentity(agent), 1)]))
                        .consume(
                            Role::Agent,
                            basket([
                                (Asset::At(from), 1),
                                (Asset::Energy, 1),
                                (Asset::TimeRemaining, 1),
                            ]),
                        )
                        .produce(
                            Role::Agent,
                            basket([
                                (Asset::At(to), 1),
                                (Asset::SpentEnergy, 1),
                                (Asset::ElapsedTime, 1),
                            ]),
                        ),
                );
            }
        }
        builder = builder
            .rate(
                RateId::Repair { agent },
                Rate::new()
                    .preserve(
                        Role::Agent,
                        basket([
                            (Asset::AgentIdentity(agent), 1),
                            (Asset::At(Location::Workshop), 1),
                        ]),
                    )
                    .consume(
                        Role::Agent,
                        basket([(Asset::Damage, 1), (Asset::TimeRemaining, 3)]),
                    )
                    .produce(
                        Role::Agent,
                        basket([(Asset::Operational, 1), (Asset::ElapsedTime, 3)]),
                    )
                    .consume(Role::Facility, basket([(Asset::RepairSupply, 1)]))
                    .produce(Role::Facility, basket([(Asset::SpentRepairSupply, 1)]))
                    .distinct(Role::Agent, Role::Facility),
            )
            .rate(
                RateId::Recharge { agent },
                Rate::new()
                    .preserve(
                        Role::Agent,
                        basket([
                            (Asset::AgentIdentity(agent), 1),
                            (Asset::At(Location::Charger), 1),
                        ]),
                    )
                    .consume(Role::Agent, basket([(Asset::TimeRemaining, 2)]))
                    .produce(
                        Role::Agent,
                        basket([(Asset::Energy, 12), (Asset::ElapsedTime, 2)]),
                    )
                    .consume(Role::Facility, basket([(Asset::ChargeSupply, 12)]))
                    .distinct(Role::Agent, Role::Facility),
            )
            .rate(
                RateId::Recycle { agent },
                Rate::new()
                    .preserve(
                        Role::Agent,
                        basket([
                            (Asset::AgentIdentity(agent), 1),
                            (Asset::At(Location::Recycler), 1),
                        ]),
                    )
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
                    .preserve(Role::Facility, basket([(Asset::RecyclerCapacity, 1)]))
                    .distinct(Role::Agent, Role::Facility),
            );
    }

    League {
        initial: builder.build().expect("work league model is valid"),
        goal,
        agents,
        jobs,
    }
}

pub fn run(league: &League, seed: u64) -> Result<MatchOutcome, String> {
    let mut world = league.initial.fork();
    let mut trace = Trace::new();
    let mut available = league.jobs.iter().map(|spec| spec.id).collect::<Vec<_>>();
    let mut turn = 0usize;

    while !available.is_empty() {
        let agent = league.agents[turn % league.agents.len()];
        let policy =
            policy_of(&world, agent).ok_or_else(|| format!("missing policy for {agent:?}"))?;
        let job = choose_job(&available, policy);
        available.retain(|candidate| *candidate != job);
        let spec = job_spec(job);
        apply(&mut world, &mut trace, action(RateId::Claim { agent, job }))?;

        move_to(&mut world, &mut trace, agent, spec.location)?;
        let mut mode = mode_for(policy);
        let mut attempt = 0u64;
        loop {
            attempt += 1;
            apply(
                &mut world,
                &mut trace,
                action(RateId::Begin { agent, job, mode }),
            )?;
            let outcome = sampled_outcome(seed, agent, job, mode, attempt, spec.risk);
            apply(
                &mut world,
                &mut trace,
                action(RateId::Resolve {
                    agent,
                    job,
                    mode,
                    outcome,
                }),
            )?;
            apply(
                &mut world,
                &mut trace,
                action(RateId::Finish {
                    agent,
                    job,
                    mode,
                    outcome,
                }),
            )?;
            if outcome == Outcome::Success {
                break;
            }
            move_to(&mut world, &mut trace, agent, Location::Workshop)?;
            apply(
                &mut world,
                &mut trace,
                action(RateId::Repair { agent })
                    .bind(Role::Facility, AccountId::Facility(Facility::Workshop)),
            )?;
            move_to(&mut world, &mut trace, agent, spec.location)?;
            mode = WorkMode::Safe;
        }

        if policy == Policy::Steward
            && world.balance(&AccountId::Agent(agent), &Asset::Waste).get() > 0
        {
            move_to(&mut world, &mut trace, agent, Location::Recycler)?;
            apply(
                &mut world,
                &mut trace,
                action(RateId::Recycle { agent })
                    .bind(Role::Facility, AccountId::Facility(Facility::Recycler)),
            )?;
        }
        turn += 1;
    }

    Ok(MatchOutcome {
        trace,
        final_world: world,
    })
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
        energy: 3 + (index * 5 % 5),
        time: 3 + (index * 3 % 6),
        material: 2 + (index * 2 % 4),
        risk: 1 + (index * 3 % 4),
    }
}

pub fn policy(world: &World, agent: AgentId) -> Option<Policy> {
    policy_of(world, agent)
}

pub fn location(world: &World, agent: AgentId) -> Option<Location> {
    LOCATIONS.into_iter().find(|location| {
        !world
            .balance(&AccountId::Agent(agent), &Asset::At(*location))
            .is_zero()
    })
}

fn policy_of(world: &World, agent: AgentId) -> Option<Policy> {
    [
        Policy::Sprinter,
        Policy::Steward,
        Policy::ValueHunter,
        Policy::Resilient,
    ]
    .into_iter()
    .find(|policy| {
        !world
            .balance(&AccountId::Agent(agent), &Asset::Policy(*policy))
            .is_zero()
    })
}

fn choose_job(available: &[JobId], policy: Policy) -> JobId {
    let mut jobs = available.iter().copied().collect::<Vec<_>>();
    jobs.sort_by_key(|job| {
        let spec = job_spec(*job);
        match policy {
            Policy::Sprinter => (
                spec.time,
                u64::MAX - spec.value,
                spec.risk,
                u64::from(job.0),
            ),
            Policy::Steward => (spec.material, spec.energy, spec.risk, u64::from(job.0)),
            Policy::ValueHunter => (
                u64::MAX - spec.value,
                spec.risk,
                spec.time,
                u64::from(job.0),
            ),
            Policy::Resilient => (
                spec.risk,
                spec.energy,
                u64::MAX - spec.value,
                u64::from(job.0),
            ),
        }
    });
    jobs[0]
}

fn mode_for(policy: Policy) -> WorkMode {
    match policy {
        Policy::Sprinter | Policy::ValueHunter => WorkMode::Rush,
        Policy::Steward => WorkMode::Lean,
        Policy::Resilient => WorkMode::Safe,
    }
}

#[derive(Debug, Clone, Copy)]
struct WorkCost {
    energy: u64,
    time: u64,
    material: u64,
    waste: u64,
}

fn work_cost(spec: JobSpec, mode: WorkMode) -> WorkCost {
    match mode {
        WorkMode::Rush => WorkCost {
            energy: spec.energy + 2,
            time: spec.time.saturating_sub(1).max(1),
            material: spec.material + 1,
            waste: 2,
        },
        WorkMode::Lean => WorkCost {
            energy: spec.energy,
            time: spec.time + 2,
            material: spec.material,
            waste: index_parity(spec.id),
        },
        WorkMode::Safe => WorkCost {
            energy: spec.energy + 1,
            time: spec.time + 1,
            material: spec.material,
            waste: 1,
        },
    }
}

const fn index_parity(job: JobId) -> u64 {
    (job.0 as u64) % 2
}

fn failure_weight(risk: u64, mode: WorkMode) -> u64 {
    match mode {
        WorkMode::Rush => (risk + 2).min(8),
        WorkMode::Lean => risk.min(7),
        WorkMode::Safe => risk.saturating_sub(1).min(4),
    }
}

fn sampled_outcome(
    seed: u64,
    agent: AgentId,
    job: JobId,
    mode: WorkMode,
    attempt: u64,
    risk: u64,
) -> Outcome {
    // SplitMix64 gives deterministic, platform-independent tickets without
    // making an RNG part of authoritative state. Nature's weight assets remain
    // the inspectable distribution the caller samples from.
    let mut value =
        seed ^ (u64::from(job.0) << 17) ^ ((agent as u64) << 9) ^ ((mode as u64) << 5) ^ attempt;
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let ticket = (value ^ (value >> 31)) % 10;
    if ticket < failure_weight(risk, mode) {
        Outcome::Failure
    } else {
        Outcome::Success
    }
}

fn move_to(
    world: &mut World,
    trace: &mut Trace<RateId, Role, AccountId>,
    agent: AgentId,
    to: Location,
) -> Result<(), String> {
    let from = location(world, agent).ok_or_else(|| format!("{agent:?} has no location"))?;
    if from != to {
        apply(world, trace, action(RateId::Move { agent, from, to }))?;
    }
    Ok(())
}

fn action(rate: RateId) -> Action {
    let mut exchange = Exchange::new(rate, Quantity::new(1));
    let (agent, job, nature) = match rate {
        RateId::Claim { agent, job }
        | RateId::Begin { agent, job, .. }
        | RateId::Finish { agent, job, .. } => (agent, Some(job), false),
        RateId::Resolve { agent, .. } => (agent, None, true),
        RateId::Move { agent, .. }
        | RateId::Repair { agent }
        | RateId::Recharge { agent }
        | RateId::Recycle { agent } => (agent, None, false),
    };
    exchange = exchange.bind(Role::Agent, AccountId::Agent(agent));
    if let Some(job) = job {
        exchange = exchange.bind(Role::Job, AccountId::Job(job));
    }
    if nature {
        exchange = exchange.bind(Role::Nature, AccountId::Nature);
    }
    exchange
}

fn apply(
    world: &mut World,
    trace: &mut Trace<RateId, Role, AccountId>,
    exchange: Action,
) -> Result<(), String> {
    world
        .apply(exchange.clone())
        .map_err(|error| format!("{:?}: {error:?}", exchange.rate()))?;
    trace.push(exchange);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showcase_is_a_substantial_replay_verified_competition() {
        let league = league(Profile::Showcase, mixed_lineup());
        let outcome = run(&league, 17).unwrap();
        assert!(outcome.trace().exchanges().len() >= 50);
        assert!(outcome.final_world().matches(league.goal()));
        let mut replay = league.initial().fork();
        replay.replay(outcome.trace()).unwrap();
        assert_eq!(
            replay.state_fingerprint(),
            outcome.final_world().state_fingerprint()
        );
        assert!(
            AGENTS
                .into_iter()
                .filter(|agent| {
                    outcome
                        .final_world()
                        .balance(&AccountId::Agent(*agent), &Asset::Completed)
                        .get()
                        > 0
                })
                .count()
                >= 3
        );
    }

    #[test]
    fn identity_assets_reject_rebinding_a_claim_to_the_wrong_agent() {
        let league = league(Profile::Micro, mixed_lineup());
        let wrong = Exchange::new(
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(1),
            },
            Quantity::new(1),
        )
        .bind(Role::Agent, AccountId::Agent(AgentId::Bolt))
        .bind(Role::Job, AccountId::Job(JobId(1)));
        assert!(!league.initial().is_applicable(&wrong));
    }

    #[test]
    fn stress_is_materially_larger_than_showcase() {
        let showcase = run(&league(Profile::Showcase, mixed_lineup()), 17).unwrap();
        let stress = run(&league(Profile::Stress, mixed_lineup()), 17).unwrap();
        assert!(stress.trace().exchanges().len() > showcase.trace().exchanges().len() + 35);
    }
}
