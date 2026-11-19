#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Goal, Quantity, basket};

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
    let reference = match profile {
        Profile::Micro => axionomy::path!("work-league/micro"),
        Profile::Showcase => axionomy::path!("work-league/showcase"),
        Profile::Stress => axionomy::path!("work-league/stress"),
    };
    let agents = AGENTS[..profile.agent_count()].to_vec();
    let jobs: Vec<_> = (1..=profile.job_count())
        .map(|n| job_spec(JobId(n as u8)))
        .collect();
    let initial = crate::fixtures::variant(reference, |instance| {
        for (&agent, policy) in agents.iter().zip(lineup) {
            crate::fixtures::set(
                instance,
                AccountId::Agent(agent),
                [
                    Policy::Sprinter,
                    Policy::Steward,
                    Policy::ValueHunter,
                    Policy::Resilient,
                ]
                .map(|p| (Asset::Policy(p), u64::from(p == policy))),
            );
        }
    });
    let goal = if profile == Profile::Micro {
        jobs.iter().fold(Goal::new(), |goal, spec| {
            goal.require(AccountId::Job(spec.id), basket([(Asset::Completed, 1)]))
        })
    } else {
        Goal::new().require(AccountId::World, basket([(Asset::SeasonEnded, 1)]))
    };
    League {
        initial,
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
