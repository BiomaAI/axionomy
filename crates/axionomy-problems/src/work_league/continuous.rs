//! Shared-time work economy. The runner proposes events and worker decisions;
//! time, deadlines, leases, activity duration and facility ownership are laws.

use super::*;
use axionomy::{QuantityComparison as C, QuantityExpression as Q};

const FACILITIES: [Facility; 3] = [Facility::Workshop, Facility::Charger, Facility::Recycler];
const MODES: [WorkMode; 3] = [WorkMode::Rush, WorkMode::Lean, WorkMode::Safe];
const LEASE: u64 = 6;
const ROUTES: [(Location, Location); 9] = [
    (Location::Depot, Location::North),
    (Location::Depot, Location::East),
    (Location::Depot, Location::South),
    (Location::Depot, Location::West),
    (Location::West, Location::Workshop),
    (Location::East, Location::Charger),
    (Location::North, Location::Recycler),
    (Location::South, Location::Workshop),
    (Location::South, Location::Charger),
];

fn q(role: Role, asset: Asset) -> Q<Role, Asset> {
    Q::balance(role, asset)
}
fn now() -> Q<Role, Asset> {
    q(Role::World, Asset::Tick)
}
fn law() -> Rate<Role, Asset> {
    Rate::new()
        .preserve(
            Role::World,
            basket([(Asset::WorldIdentity, 1), (Asset::SeasonActive, 1)]),
        )
        .condition("one event at a time", Q::units(), C::Equal, Q::constant(1))
}
fn actor(rate: Rate<Role, Asset>, agent: AgentId) -> Rate<Role, Asset> {
    rate.preserve(Role::Agent, basket([(Asset::AgentIdentity(agent), 1)]))
        .distinct(Role::Agent, Role::World)
}
fn turn(agent: AgentId) -> Rate<Role, Asset> {
    actor(law(), agent)
        .consume(Role::World, basket([(Asset::Permit(agent), 1)]))
        .produce(Role::World, basket([(Asset::Acted(agent), 1)]))
}
fn job(rate: Rate<Role, Asset>, id: JobId) -> Rate<Role, Asset> {
    rate.preserve(Role::Job, basket([(Asset::JobIdentity(id), 1)]))
        .distinct(Role::Job, Role::World)
}
fn clear(rate: Rate<Role, Asset>, role: Role, asset: Asset) -> Rate<Role, Asset> {
    rate.consume_computed(role, asset, q(role, asset))
}
fn occupy(rate: Rate<Role, Asset>, duration: u64) -> Rate<Role, Asset> {
    rate.consume(Role::Agent, basket([(Asset::Idle, 1)]))
        .produce(Role::Agent, basket([(Asset::Busy, 1)]))
        .produce(Role::World, basket([(Asset::Activities, 1)]))
        .produce_computed(Role::Agent, Asset::ReadyAt, now() + Q::constant(duration))
        .condition(
            "activity fits the season",
            now() + Q::constant(duration),
            C::LessThanOrEqual,
            q(Role::World, Asset::Horizon),
        )
}
fn finish_activity(rate: Rate<Role, Asset>) -> Rate<Role, Asset> {
    clear(rate, Role::Agent, Asset::ReadyAt)
        .condition(
            "activity is due",
            now(),
            C::GreaterThanOrEqual,
            q(Role::Agent, Asset::ReadyAt),
        )
        .consume(Role::Agent, basket([(Asset::Busy, 1)]))
        .produce(Role::Agent, basket([(Asset::Idle, 1)]))
        .consume(Role::World, basket([(Asset::Activities, 1)]))
}
fn service_location(facility: Facility) -> Location {
    match facility {
        Facility::Workshop => Location::Workshop,
        Facility::Charger => Location::Charger,
        Facility::Recycler => Location::Recycler,
    }
}
fn dependency(id: JobId) -> Option<JobId> {
    (id.0 % 4 == 0).then(|| JobId(id.0 - 3))
}
fn prerequisite(rate: Rate<Role, Asset>, id: JobId) -> Rate<Role, Asset> {
    if let Some(before) = dependency(id) {
        rate.preserve(Role::World, basket([(Asset::CompletedJob(before), 1)]))
    } else {
        rate
    }
}

pub(super) fn build(profile: Profile, lineup: [Policy; 4]) -> League {
    let agents = AGENTS.to_vec();
    let jobs = (1..=profile.job_count())
        .map(|i| job_spec(JobId(i as u8)))
        .collect::<Vec<_>>();
    let horizon = if profile == Profile::Stress { 78 } else { 48 };
    let mut world_assets = basket([
        (Asset::WorldIdentity, 1),
        (Asset::SeasonActive, 1),
        (Asset::Horizon, horizon),
        (Asset::ContractsOpen, jobs.len() as u64),
    ]);
    let mut builder = EconomyBuilder::new();
    for (index, agent) in AGENTS.into_iter().enumerate() {
        world_assets.insert(Asset::Permit(agent), Quantity::new(1));
        builder = builder.account(
            AccountId::Agent(agent),
            Account::from(basket([
                (Asset::AgentIdentity(agent), 1),
                (Asset::Policy(lineup[index]), 1),
                (Asset::At(Location::Depot), 1),
                (Asset::Operational, 1),
                (Asset::Idle, 1),
                (Asset::FreeContract, 1),
                (Asset::Energy, 14),
                (Asset::Material, 11),
                (Asset::Credit, 5),
                (Asset::TimeRemaining, horizon),
            ])),
        );
    }
    builder = builder.account(AccountId::World, Account::from(world_assets));
    let mut nature = Basket::new();
    for spec in &jobs {
        for mode in MODES {
            let failure = failure_weight(spec.risk, mode);
            nature.insert(
                Asset::OutcomeWeight(spec.id, mode, Outcome::Failure),
                Quantity::new(failure),
            );
            nature.insert(
                Asset::OutcomeWeight(spec.id, mode, Outcome::Success),
                Quantity::new(10 - failure),
            );
        }
    }
    builder = builder.account(AccountId::Nature, Account::from(nature));
    for facility in FACILITIES {
        let mut assets = basket([
            (Asset::FacilityIdentity(facility), 1),
            (Asset::Vacant(facility), 1),
        ]);
        match facility {
            Facility::Workshop => {
                assets.insert(Asset::RepairSupply, Quantity::new(jobs.len() as u64));
            }
            Facility::Charger => {
                assets.insert(Asset::ChargeSupply, Quantity::new(jobs.len() as u64 * 12));
            }
            Facility::Recycler => {}
        }
        builder = builder.account(AccountId::Facility(facility), Account::from(assets));
        let mut invariant = LinearInvariant::new(format!("one {facility:?} service slot"))
            .weight(Asset::Vacant(facility), 1);
        for agent in AGENTS {
            invariant = invariant.weight(Asset::Occupied(facility, agent), 1);
        }
        builder = builder.invariant(invariant);
    }

    for spec in &jobs {
        let release = u64::from((spec.id.0 - 1) / 4) * 6;
        let deadline = release + 20 + u64::from(spec.id.0 % 3) * 3;
        builder = builder.account(
            AccountId::Job(spec.id),
            Account::from(basket([
                (Asset::JobIdentity(spec.id), 1),
                (Asset::Available, 1),
                (Asset::Transferable, 1),
                (Asset::Credit, spec.value),
                (Asset::ReleaseAt, release),
                (Asset::Deadline, deadline),
            ])),
        );
        builder = builder.rate(
            RateId::Expire {
                job: spec.id,
                agent: None,
                mode: None,
            },
            expire_rate(spec.id, None, None),
        );
        for agent in AGENTS {
            builder = builder.rate(
                RateId::Claim {
                    agent,
                    job: spec.id,
                },
                prerequisite(job(turn(agent), spec.id), spec.id)
                    .preserve(
                        Role::Agent,
                        basket([(Asset::Idle, 1), (Asset::Operational, 1)]),
                    )
                    .consume(Role::Agent, basket([(Asset::FreeContract, 1)]))
                    .produce(Role::Agent, basket([(Asset::Claimed(spec.id), 1)]))
                    .consume(Role::Job, basket([(Asset::Available, 1)]))
                    .produce(
                        Role::Job,
                        basket([(Asset::Assigned(agent), 1), (Asset::Pending, 1)]),
                    )
                    .produce_computed(Role::Job, Asset::LeaseUntil, now() + Q::constant(LEASE))
                    .condition(
                        "contract has arrived",
                        now(),
                        C::GreaterThanOrEqual,
                        q(Role::Job, Asset::ReleaseAt),
                    )
                    .condition(
                        "contract remains open",
                        now() + Q::constant(1),
                        C::LessThanOrEqual,
                        q(Role::Job, Asset::Deadline),
                    )
                    .distinct(Role::Agent, Role::Job),
            );
            for expired in [false, true] {
                let base = if expired {
                    actor(law(), agent)
                } else {
                    turn(agent)
                };
                let mut release = clear(job(base, spec.id), Role::Job, Asset::LeaseUntil)
                    .consume(Role::Agent, basket([(Asset::Claimed(spec.id), 1)]))
                    .produce(Role::Agent, basket([(Asset::FreeContract, 1)]))
                    .consume(
                        Role::Job,
                        basket([(Asset::Assigned(agent), 1), (Asset::Pending, 1)]),
                    )
                    .produce(Role::Job, basket([(Asset::Available, 1)]))
                    .distinct(Role::Agent, Role::Job);
                if expired {
                    release = release.condition(
                        "claim lease has elapsed",
                        now(),
                        C::GreaterThanOrEqual,
                        q(Role::Job, Asset::LeaseUntil) + Q::constant(1),
                    );
                } else {
                    release = release.preserve(Role::Agent, basket([(Asset::Idle, 1)]));
                }
                builder = builder.rate(
                    RateId::ReleaseClaim {
                        agent,
                        job: spec.id,
                        expired,
                    },
                    release,
                );
            }
            builder = builder.rate(
                RateId::Expire {
                    job: spec.id,
                    agent: Some(agent),
                    mode: None,
                },
                expire_rate(spec.id, Some(agent), None),
            );
            for mode in MODES {
                let cost = work_cost(*spec, mode);
                let begin = occupy(
                    clear(
                        prerequisite(job(turn(agent), spec.id), spec.id),
                        Role::Job,
                        Asset::LeaseUntil,
                    ),
                    cost.time,
                )
                .preserve(
                    Role::Agent,
                    basket([
                        (Asset::Operational, 1),
                        (Asset::At(spec.location), 1),
                        (Asset::Claimed(spec.id), 1),
                    ]),
                )
                .consume(
                    Role::Agent,
                    basket([
                        (Asset::Energy, cost.energy),
                        (Asset::Material, cost.material),
                    ]),
                )
                .produce(
                    Role::Agent,
                    basket([
                        (Asset::SpentEnergy, cost.energy),
                        (Asset::MaterialSpent, cost.material),
                        (Asset::Waste, cost.waste),
                        (Asset::EmbodiedMaterial, cost.material - cost.waste),
                        (Asset::Attempts, 1),
                        (Asset::Awaiting(spec.id, mode), 1),
                        (Asset::Wear, if mode == WorkMode::Rush { 2 } else { 1 }),
                    ]),
                )
                .preserve(Role::Job, basket([(Asset::Assigned(agent), 1)]))
                .consume(Role::Job, basket([(Asset::Pending, 1)]))
                .produce(
                    Role::Job,
                    basket([(Asset::InProgress, 1), (Asset::Attempts, 1)]),
                )
                .condition(
                    "claim lease remains valid",
                    now(),
                    C::LessThanOrEqual,
                    q(Role::Job, Asset::LeaseUntil),
                )
                .condition(
                    "work finishes by deadline",
                    now() + Q::constant(cost.time),
                    C::LessThanOrEqual,
                    q(Role::Job, Asset::Deadline),
                )
                .condition(
                    "equipment needs maintenance below wear six",
                    q(Role::Agent, Asset::Wear),
                    C::LessThanOrEqual,
                    Q::constant(5),
                )
                .distinct(Role::Agent, Role::Job);
                builder = builder.rate(
                    RateId::Begin {
                        agent,
                        job: spec.id,
                        mode,
                    },
                    begin,
                );
                for outcome in [Outcome::Success, Outcome::Failure] {
                    let mut finish = finish_activity(job(actor(law(), agent), spec.id))
                        .preserve(
                            Role::Nature,
                            basket([(Asset::OutcomeWeight(spec.id, mode, outcome), 1)]),
                        )
                        .consume(Role::Agent, basket([(Asset::Awaiting(spec.id, mode), 1)]))
                        .consume(Role::Job, basket([(Asset::InProgress, 1)]))
                        .condition(
                            "outcome settles before expiry",
                            now(),
                            C::LessThanOrEqual,
                            q(Role::Job, Asset::Deadline),
                        )
                        .distinct(Role::Agent, Role::Job)
                        .distinct(Role::Nature, Role::World);
                    finish = match outcome {
                        Outcome::Success => finish
                            .consume(Role::Agent, basket([(Asset::Claimed(spec.id), 1)]))
                            .produce(
                                Role::Agent,
                                basket([
                                    (Asset::FreeContract, 1),
                                    (Asset::Value, spec.value),
                                    (Asset::Credit, spec.value),
                                    (Asset::Completed, 1),
                                    (Asset::Successes, 1),
                                ]),
                            )
                            .consume(
                                Role::Job,
                                basket([(Asset::Assigned(agent), 1), (Asset::Credit, spec.value)]),
                            )
                            .produce(Role::Job, basket([(Asset::Completed, 1)]))
                            .consume(Role::World, basket([(Asset::ContractsOpen, 1)]))
                            .produce(
                                Role::World,
                                basket([
                                    (Asset::ContractsClosed, 1),
                                    (Asset::CompletedJob(spec.id), 1),
                                ]),
                            ),
                        Outcome::Failure => finish
                            .preserve(Role::Agent, basket([(Asset::Claimed(spec.id), 1)]))
                            .consume(Role::Agent, basket([(Asset::Operational, 1)]))
                            .produce(
                                Role::Agent,
                                basket([(Asset::Damage, 1), (Asset::Failures, 1)]),
                            )
                            .preserve(Role::Job, basket([(Asset::Assigned(agent), 1)]))
                            .produce(Role::Job, basket([(Asset::Pending, 1)]))
                            .produce_computed(
                                Role::Job,
                                Asset::LeaseUntil,
                                now() + Q::constant(LEASE),
                            ),
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
                builder = builder.rate(
                    RateId::Expire {
                        job: spec.id,
                        agent: Some(agent),
                        mode: Some(mode),
                    },
                    expire_rate(spec.id, Some(agent), Some(mode)),
                );
            }
            for recipient in AGENTS.into_iter().filter(|other| *other != agent) {
                // A joint proposal spends both actors' decision permissions. A
                // contract can be handed off once, preventing referral farming.
                builder = builder.rate(
                    RateId::Handoff {
                        agent,
                        recipient,
                        job: spec.id,
                    },
                    clear(job(turn(agent), spec.id), Role::Job, Asset::LeaseUntil)
                        .consume(Role::World, basket([(Asset::Permit(recipient), 1)]))
                        .produce(Role::World, basket([(Asset::Acted(recipient), 1)]))
                        .preserve(Role::Agent, basket([(Asset::Idle, 1)]))
                        .consume(Role::Agent, basket([(Asset::Claimed(spec.id), 1)]))
                        .produce(
                            Role::Agent,
                            basket([
                                (Asset::FreeContract, 1),
                                (Asset::Credit, 2),
                                (Asset::Assists, 1),
                            ]),
                        )
                        .preserve(
                            Role::Partner,
                            basket([
                                (Asset::AgentIdentity(recipient), 1),
                                (Asset::Idle, 1),
                                (Asset::Operational, 1),
                            ]),
                        )
                        .consume(
                            Role::Partner,
                            basket([(Asset::FreeContract, 1), (Asset::Credit, 2)]),
                        )
                        .produce(Role::Partner, basket([(Asset::Claimed(spec.id), 1)]))
                        .preserve(Role::Job, basket([(Asset::Pending, 1)]))
                        .consume(
                            Role::Job,
                            basket([(Asset::Assigned(agent), 1), (Asset::Transferable, 1)]),
                        )
                        .produce(
                            Role::Job,
                            basket([(Asset::Assigned(recipient), 1), (Asset::HandedOff, 1)]),
                        )
                        .produce_computed(Role::Job, Asset::LeaseUntil, now() + Q::constant(LEASE))
                        .condition(
                            "handoff before deadline",
                            now() + Q::constant(1),
                            C::LessThanOrEqual,
                            q(Role::Job, Asset::Deadline),
                        )
                        .distinct(Role::Agent, Role::Partner)
                        .distinct(Role::Partner, Role::Job)
                        .distinct(Role::Agent, Role::Job),
                );
            }
        }
    }
    for agent in AGENTS {
        builder = builder.rate(RateId::Wait { agent }, turn(agent));
        for from in LOCATIONS {
            for to in LOCATIONS
                .into_iter()
                .filter(|to| ROUTES.contains(&(from, *to)) || ROUTES.contains(&(*to, from)))
            {
                builder = builder.rate(
                    RateId::Move { agent, from, to },
                    turn(agent)
                        .preserve(Role::Agent, basket([(Asset::Idle, 1)]))
                        .consume(
                            Role::Agent,
                            basket([(Asset::At(from), 1), (Asset::Energy, 1)]),
                        )
                        .produce(
                            Role::Agent,
                            basket([(Asset::At(to), 1), (Asset::SpentEnergy, 1)]),
                        ),
                );
            }
        }
        for facility in FACILITIES {
            for maintenance in [false, true] {
                if maintenance && facility != Facility::Workshop {
                    continue;
                }
                builder = builder.rate(
                    RateId::BeginService {
                        agent,
                        facility,
                        maintenance,
                    },
                    service_rate(agent, facility, maintenance),
                );
            }
            let mut end = finish_activity(actor(law(), agent))
                .consume(Role::Agent, basket([(Asset::Servicing(facility), 1)]))
                .preserve(
                    Role::Facility,
                    basket([(Asset::FacilityIdentity(facility), 1)]),
                )
                .consume(
                    Role::Facility,
                    basket([(Asset::Occupied(facility, agent), 1)]),
                )
                .produce(Role::Facility, basket([(Asset::Vacant(facility), 1)]))
                .distinct(Role::Facility, Role::Agent)
                .distinct(Role::Facility, Role::World);
            end = match facility {
                Facility::Workshop => clear(end, Role::Agent, Asset::Wear)
                    .consume(Role::Agent, basket([(Asset::Repairing, 1)]))
                    .produce(Role::Agent, basket([(Asset::Operational, 1)])),
                Facility::Charger => end
                    .consume(Role::Agent, basket([(Asset::ChargingEnergy, 12)]))
                    .produce(Role::Agent, basket([(Asset::Energy, 12)])),
                Facility::Recycler => end
                    .consume(Role::Agent, basket([(Asset::ReclaimingMaterial, 1)]))
                    .produce(Role::Agent, basket([(Asset::Material, 1)])),
            };
            builder = builder.rate(RateId::EndService { agent, facility }, end);
        }
        builder = builder.invariant(
            LinearInvariant::new(format!("{agent:?} decision permission"))
                .weight(Asset::Permit(agent), 1)
                .weight(Asset::Acted(agent), 1),
        );
    }
    let mut tick = law()
        .condition(
            "season has time remaining",
            now() + Q::constant(1),
            C::LessThanOrEqual,
            q(Role::World, Asset::Horizon),
        )
        .produce(Role::World, basket([(Asset::Tick, 1)]));
    for agent in AGENTS {
        tick = tick
            .consume(Role::World, basket([(Asset::Acted(agent), 1)]))
            .produce(Role::World, basket([(Asset::Permit(agent), 1)]))
            .preserve(
                Role::Worker(agent),
                basket([(Asset::AgentIdentity(agent), 1)]),
            )
            .consume(Role::Worker(agent), basket([(Asset::TimeRemaining, 1)]))
            .produce(Role::Worker(agent), basket([(Asset::ElapsedTime, 1)]));
    }
    builder = builder
        .rate(RateId::Tick, tick)
        .rate(
            RateId::EndSeason,
            Rate::new()
                .preserve(Role::World, basket([(Asset::WorldIdentity, 1)]))
                .consume(Role::World, basket([(Asset::SeasonActive, 1)]))
                .produce(Role::World, basket([(Asset::SeasonEnded, 1)]))
                .condition(
                    "all contracts resolved",
                    q(Role::World, Asset::ContractsOpen),
                    C::Equal,
                    Q::constant(0),
                )
                .condition(
                    "all activities settled",
                    q(Role::World, Asset::Activities),
                    C::Equal,
                    Q::constant(0),
                ),
        )
        .invariant(
            LinearInvariant::new("season lifecycle")
                .weight(Asset::SeasonActive, 1)
                .weight(Asset::SeasonEnded, 1),
        )
        .invariant(
            LinearInvariant::new("finite contracts")
                .weight(Asset::ContractsOpen, 1)
                .weight(Asset::ContractsClosed, 1),
        )
        .invariant(
            LinearInvariant::new("worker availability")
                .weight(Asset::Idle, 1)
                .weight(Asset::Busy, 1),
        )
        .invariant(
            LinearInvariant::new("worker condition")
                .weight(Asset::Operational, 1)
                .weight(Asset::Damage, 1)
                .weight(Asset::Repairing, 1),
        )
        .invariant(
            LinearInvariant::new("energy lifecycle")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1)
                .weight(Asset::ChargeSupply, 1)
                .weight(Asset::ChargingEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("material lifecycle")
                .weight(Asset::Material, 1)
                .weight(Asset::Waste, 1)
                .weight(Asset::EmbodiedMaterial, 1)
                .weight(Asset::ReclaimingMaterial, 1)
                .weight(Asset::RecyclingLoss, 1),
        )
        .invariant(LinearInvariant::new("closed settlement credit").weight(Asset::Credit, 1))
        .invariant(
            LinearInvariant::new("repair supplies")
                .weight(Asset::RepairSupply, 1)
                .weight(Asset::SpentRepairSupply, 1),
        )
        .invariant(
            LinearInvariant::new("worker time")
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        );
    let mut claims = LinearInvariant::new("one contract per worker").weight(Asset::FreeContract, 1);
    for spec in &jobs {
        claims = claims.weight(Asset::Claimed(spec.id), 1);
    }
    builder = builder.invariant(claims);
    League {
        initial: builder.build().expect("continuous league is valid"),
        goal: Goal::new().require(AccountId::World, basket([(Asset::SeasonEnded, 1)])),
        agents,
        jobs,
    }
}

fn expire_rate(id: JobId, owner: Option<AgentId>, mode: Option<WorkMode>) -> Rate<Role, Asset> {
    let mut rate = job(law(), id)
        .condition(
            "contract deadline has elapsed",
            now(),
            C::GreaterThanOrEqual,
            q(Role::Job, Asset::Deadline) + Q::constant(1),
        )
        .produce(Role::Job, basket([(Asset::Expired, 1)]))
        .consume(Role::World, basket([(Asset::ContractsOpen, 1)]))
        .produce(Role::World, basket([(Asset::ContractsClosed, 1)]));
    if let Some(agent) = owner {
        rate = actor(rate, agent)
            .consume(Role::Agent, basket([(Asset::Claimed(id), 1)]))
            .produce(Role::Agent, basket([(Asset::FreeContract, 1)]))
            .consume(Role::Job, basket([(Asset::Assigned(agent), 1)]))
            .distinct(Role::Agent, Role::Job);
        if let Some(mode) = mode {
            rate = finish_activity(rate)
                .consume(Role::Agent, basket([(Asset::Awaiting(id, mode), 1)]))
                .consume(Role::Job, basket([(Asset::InProgress, 1)]));
        } else {
            rate = clear(rate, Role::Job, Asset::LeaseUntil)
                .consume(Role::Job, basket([(Asset::Pending, 1)]));
        }
    } else {
        rate = rate.consume(Role::Job, basket([(Asset::Available, 1)]));
    }
    rate
}

fn service_rate(agent: AgentId, facility: Facility, maintenance: bool) -> Rate<Role, Asset> {
    let duration = match facility {
        Facility::Workshop => 4,
        Facility::Charger => 3,
        Facility::Recycler => 2,
    };
    let rate = occupy(turn(agent), duration)
        .preserve(
            Role::Agent,
            basket([(Asset::At(service_location(facility)), 1)]),
        )
        .produce(Role::Agent, basket([(Asset::Servicing(facility), 1)]))
        .preserve(
            Role::Facility,
            basket([(Asset::FacilityIdentity(facility), 1)]),
        )
        .consume(Role::Facility, basket([(Asset::Vacant(facility), 1)]))
        .produce(
            Role::Facility,
            basket([(Asset::Occupied(facility, agent), 1)]),
        )
        .distinct(Role::Agent, Role::Facility)
        .distinct(Role::Facility, Role::World);
    match facility {
        Facility::Workshop => rate
            .consume(
                Role::Agent,
                basket([
                    (
                        if maintenance {
                            Asset::Operational
                        } else {
                            Asset::Damage
                        },
                        1,
                    ),
                    (Asset::Credit, 2),
                ]),
            )
            .produce(Role::Agent, basket([(Asset::Repairing, 1)]))
            .consume(Role::Facility, basket([(Asset::RepairSupply, 1)]))
            .produce(
                Role::Facility,
                basket([(Asset::SpentRepairSupply, 1), (Asset::Credit, 2)]),
            ),
        Facility::Charger => rate
            .consume(Role::Agent, basket([(Asset::Credit, 1)]))
            .produce(Role::Agent, basket([(Asset::ChargingEnergy, 12)]))
            .consume(Role::Facility, basket([(Asset::ChargeSupply, 12)]))
            .produce(Role::Facility, basket([(Asset::Credit, 1)])),
        Facility::Recycler => rate
            .consume(Role::Agent, basket([(Asset::Waste, 2), (Asset::Energy, 1)]))
            .produce(
                Role::Agent,
                basket([
                    (Asset::SpentEnergy, 1),
                    (Asset::ReclaimingMaterial, 1),
                    (Asset::RecyclingLoss, 1),
                    (Asset::RecycledWaste, 2),
                ]),
            ),
    }
}

/// Concrete bindings for the shared-time model. All callers still pass through
/// the same assessment, identity checks, timing conditions, and invariants.
pub fn proposal(rate: RateId) -> Action {
    let mut action = Exchange::new(rate, Quantity::new(1)).bind(Role::World, AccountId::World);
    match rate {
        RateId::Claim { agent, job }
        | RateId::Begin { agent, job, .. }
        | RateId::ReleaseClaim { agent, job, .. } => {
            action = action
                .bind(Role::Agent, AccountId::Agent(agent))
                .bind(Role::Job, AccountId::Job(job));
        }
        RateId::Finish { agent, job, .. } => {
            action = action
                .bind(Role::Agent, AccountId::Agent(agent))
                .bind(Role::Job, AccountId::Job(job))
                .bind(Role::Nature, AccountId::Nature);
        }
        RateId::Move { agent, .. } | RateId::Wait { agent } => {
            action = action.bind(Role::Agent, AccountId::Agent(agent));
        }
        RateId::BeginService {
            agent, facility, ..
        }
        | RateId::EndService { agent, facility } => {
            action = action
                .bind(Role::Agent, AccountId::Agent(agent))
                .bind(Role::Facility, AccountId::Facility(facility));
        }
        RateId::Handoff {
            agent,
            recipient,
            job,
            ..
        } => {
            action = action
                .bind(Role::Agent, AccountId::Agent(agent))
                .bind(Role::Partner, AccountId::Agent(recipient))
                .bind(Role::Job, AccountId::Job(job));
        }
        RateId::Expire { job, agent, .. } => {
            action = action.bind(Role::Job, AccountId::Job(job));
            if let Some(agent) = agent {
                action = action.bind(Role::Agent, AccountId::Agent(agent));
            }
        }
        RateId::Tick => {
            for agent in AGENTS {
                action = action.bind(Role::Worker(agent), AccountId::Agent(agent));
            }
        }
        RateId::EndSeason => {}
        _ => return super::action(rate),
    }
    action
}

pub fn clock(world: &World) -> u64 {
    world.balance(&AccountId::World, &Asset::Tick).get()
}
fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
    world.balance(&AccountId::Agent(agent), &asset).get()
}
fn job_balance(world: &World, id: JobId, asset: Asset) -> u64 {
    world.balance(&AccountId::Job(id), &asset).get()
}
fn held_job(league: &League, world: &World, agent: AgentId) -> Option<JobId> {
    league
        .jobs
        .iter()
        .find(|spec| balance(world, agent, Asset::Claimed(spec.id)) > 0)
        .map(|spec| spec.id)
}
fn preferred_modes(policy: Policy) -> [WorkMode; 3] {
    match policy {
        Policy::Sprinter | Policy::ValueHunter => [WorkMode::Rush, WorkMode::Safe, WorkMode::Lean],
        Policy::Steward => [WorkMode::Lean, WorkMode::Safe, WorkMode::Rush],
        Policy::Resilient => [WorkMode::Safe, WorkMode::Lean, WorkMode::Rush],
    }
}
fn can_finish(world: &World, agent: AgentId, spec: JobSpec, mode: WorkMode, overhead: u64) -> bool {
    let cost = work_cost(spec, mode);
    clock(world) + overhead + cost.time <= job_balance(world, spec.id, Asset::Deadline)
        && balance(world, agent, Asset::Material) >= cost.material
        && balance(world, agent, Asset::Energy)
            >= cost.energy
                + route(world, agent, spec.location).map_or(100, |(_, distance)| distance)
                + 1
}
fn route(world: &World, agent: AgentId, to: Location) -> Option<(Location, u64)> {
    let from = location(world, agent)?;
    let mut visited = std::collections::BTreeSet::from([from]);
    let mut pending = std::collections::VecDeque::from([(from, from, 0)]);
    while let Some((at, first, distance)) = pending.pop_front() {
        if at == to {
            return Some((first, distance));
        }
        for next in LOCATIONS {
            if world
                .rate(&RateId::Move {
                    agent,
                    from: at,
                    to: next,
                })
                .is_some()
                && visited.insert(next)
            {
                pending.push_back((next, if distance == 0 { next } else { first }, distance + 1));
            }
        }
    }
    None
}
fn travel(world: &World, agent: AgentId, to: Location) -> Option<Action> {
    let from = location(world, agent)?;
    let (to, _) = route(world, agent, to)?;
    let action = proposal(RateId::Move { agent, from, to });
    (from != to && world.is_applicable(&action)).then_some(action)
}
fn service(world: &World, agent: AgentId, facility: Facility) -> Option<Action> {
    if location(world, agent) != Some(service_location(facility)) {
        return travel(world, agent, service_location(facility));
    }
    let action = proposal(RateId::BeginService {
        agent,
        facility,
        maintenance: facility == Facility::Workshop && balance(world, agent, Asset::Damage) == 0,
    });
    world.is_applicable(&action).then_some(action)
}

/// One state-responsive decision. The policy can be replaced without changing
/// turn permissions, action duration, deadlines, or Nature's legal outcomes.
pub fn decision(league: &League, world: &World, agent: AgentId) -> Option<Action> {
    if world
        .balance(&AccountId::World, &Asset::Permit(agent))
        .is_zero()
    {
        return None;
    }
    let wait = || Some(proposal(RateId::Wait { agent }));
    if balance(world, agent, Asset::Busy) > 0 {
        return wait();
    }
    let policy = policy_of(world, agent)?;
    let held = held_job(league, world, agent);
    let maintenance =
        balance(world, agent, Asset::Wear) >= if policy == Policy::Resilient { 4 } else { 6 };
    if balance(world, agent, Asset::Damage) > 0 || maintenance {
        if let Some(id) = held {
            for recipient in AGENTS.into_iter().filter(|other| *other != agent) {
                let exchange = proposal(RateId::Handoff {
                    agent,
                    recipient,
                    job: id,
                });
                if world.is_applicable(&exchange)
                    && preferred_modes(policy_of(world, recipient)?)
                        .into_iter()
                        .any(|mode| {
                            can_finish(
                                world,
                                recipient,
                                job_spec(id),
                                mode,
                                1 + route(world, recipient, job_spec(id).location)
                                    .map_or(100, |(_, distance)| distance),
                            )
                        })
                {
                    return Some(exchange);
                }
            }
        }
        return service(world, agent, Facility::Workshop).or_else(wait);
    }
    if let Some(id) = held {
        let spec = job_spec(id);
        let travel_time = route(world, agent, spec.location)?.1;
        let modes = preferred_modes(policy);
        // Release impossible promises while another worker still has time.
        if !modes.into_iter().any(|mode| {
            clock(world) + travel_time + work_cost(spec, mode).time
                <= job_balance(world, id, Asset::Deadline)
        }) {
            return Some(proposal(RateId::ReleaseClaim {
                agent,
                job: id,
                expired: false,
            }));
        }
        let minimum_energy = modes
            .into_iter()
            .map(|mode| work_cost(spec, mode).energy)
            .min()?
            + travel_time
            + 1;
        if balance(world, agent, Asset::Energy) < minimum_energy {
            return service(world, agent, Facility::Charger).or_else(wait);
        }
        let minimum_material = modes
            .into_iter()
            .map(|mode| work_cost(spec, mode).material)
            .min()?;
        if balance(world, agent, Asset::Material) < minimum_material {
            return service(world, agent, Facility::Recycler).or_else(|| {
                Some(proposal(RateId::ReleaseClaim {
                    agent,
                    job: id,
                    expired: false,
                }))
            });
        }
        if let Some(action) = travel(world, agent, spec.location) {
            return Some(action);
        }
        for mode in modes {
            let action = proposal(RateId::Begin {
                agent,
                job: id,
                mode,
            });
            if world.is_applicable(&action) {
                return Some(action);
            }
        }
        return wait();
    }
    if world
        .balance(&AccountId::World, &Asset::ContractsOpen)
        .is_zero()
    {
        return wait();
    }
    if balance(world, agent, Asset::Energy) < 9 {
        return service(world, agent, Facility::Charger).or_else(wait);
    }
    if balance(world, agent, Asset::Waste) >= 2
        && (balance(world, agent, Asset::Material) < 5 || policy == Policy::Steward)
    {
        return service(world, agent, Facility::Recycler).or_else(wait);
    }
    let mut options = league
        .jobs
        .iter()
        .filter_map(|spec| {
            let claim = proposal(RateId::Claim {
                agent,
                job: spec.id,
            });
            let travel = route(world, agent, spec.location)?.1;
            let mode = preferred_modes(policy)
                .into_iter()
                .find(|mode| can_finish(world, agent, *spec, *mode, 1 + travel))?;
            if !world.is_applicable(&claim) {
                return None;
            }
            let cost = work_cost(*spec, mode);
            let urgency = job_balance(world, spec.id, Asset::Deadline) - clock(world);
            let score = match policy {
                Policy::Sprinter => (cost.time + travel, urgency, u64::MAX - spec.value),
                Policy::ValueHunter => (
                    u64::MAX - spec.value * 100 / (cost.time + travel + 1),
                    urgency,
                    spec.risk,
                ),
                Policy::Steward => (cost.material + cost.waste, travel, urgency),
                Policy::Resilient => (failure_weight(spec.risk, mode), urgency, travel),
            };
            Some((score, spec.id, claim))
        })
        .collect::<Vec<_>>();
    options.sort_by_key(|(score, id, _)| (*score, *id));
    options
        .into_iter()
        .next()
        .map(|(_, _, action)| action)
        .or_else(wait)
}

fn sampled_result(world: &World, seed: u64, id: JobId, mode: WorkMode) -> Outcome {
    // Common random numbers: the same job and attempt receive the same latent
    // ticket across lineups, independently of actor, mode, and action ordering.
    let attempt = job_balance(world, id, Asset::Attempts);
    let mut value = seed ^ (u64::from(id.0) << 32) ^ attempt;
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let failure = world
        .balance(
            &AccountId::Nature,
            &Asset::OutcomeWeight(id, mode, Outcome::Failure),
        )
        .get();
    let success = world
        .balance(
            &AccountId::Nature,
            &Asset::OutcomeWeight(id, mode, Outcome::Success),
        )
        .get();
    if (value ^ (value >> 31)) % (failure + success) < failure {
        Outcome::Failure
    } else {
        Outcome::Success
    }
}

pub(super) fn run(league: &League, seed: u64) -> Result<MatchOutcome, String> {
    let mut world = league.initial.fork();
    let mut trace = Trace::new();
    loop {
        // Due events are proposed from balances; every early, stale, or wrongly
        // bound event is independently rejected by the economy.
        for spec in &league.jobs {
            let owner = AGENTS
                .into_iter()
                .find(|agent| job_balance(&world, spec.id, Asset::Assigned(*agent)) > 0);
            let mode = owner.and_then(|agent| {
                MODES
                    .into_iter()
                    .find(|mode| balance(&world, agent, Asset::Awaiting(spec.id, *mode)) > 0)
            });
            if job_balance(&world, spec.id, Asset::Completed) > 0
                || job_balance(&world, spec.id, Asset::Expired) > 0
            {
                continue;
            }
            if clock(&world) > job_balance(&world, spec.id, Asset::Deadline) {
                apply(
                    &mut world,
                    &mut trace,
                    proposal(RateId::Expire {
                        job: spec.id,
                        agent: owner,
                        mode,
                    }),
                )?;
            } else if let (Some(agent), Some(mode)) = (owner, mode) {
                if clock(&world) >= balance(&world, agent, Asset::ReadyAt) {
                    let outcome = sampled_result(&world, seed, spec.id, mode);
                    apply(
                        &mut world,
                        &mut trace,
                        proposal(RateId::Finish {
                            agent,
                            job: spec.id,
                            mode,
                            outcome,
                        }),
                    )?;
                }
            } else if let Some(agent) = owner {
                let release = proposal(RateId::ReleaseClaim {
                    agent,
                    job: spec.id,
                    expired: true,
                });
                if world.is_applicable(&release) {
                    apply(&mut world, &mut trace, release)?;
                }
            }
        }
        for agent in AGENTS {
            for facility in FACILITIES {
                let finish = proposal(RateId::EndService { agent, facility });
                if world.is_applicable(&finish) {
                    apply(&mut world, &mut trace, finish)?;
                }
            }
        }
        let end = proposal(RateId::EndSeason);
        if world.is_applicable(&end) {
            apply(&mut world, &mut trace, end)?;
            break;
        }
        // Rotate access to same-tick contention, reproducibly and independently
        // of policy identity. Busy workers cannot freeze the other workers.
        let first = clock(&world) as usize % AGENTS.len();
        for offset in 0..AGENTS.len() {
            let agent = AGENTS[(first + offset) % AGENTS.len()];
            if let Some(action) = decision(league, &world, agent) {
                apply(&mut world, &mut trace, action)?;
            }
        }
        apply(&mut world, &mut trace, proposal(RateId::Tick))?;
    }
    let replay = league
        .initial
        .replayed(&trace)
        .map_err(|error| format!("league replay: {error:?}"))?;
    if replay.state_key() != world.state_key() {
        return Err("league replay diverged".into());
    }
    Ok(MatchOutcome {
        trace,
        final_world: world,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        build(Profile::Showcase, mixed_lineup()).initial
    }
    fn act(world: &mut World, rate: RateId) {
        world.apply(proposal(rate)).unwrap();
    }
    fn tick(world: &mut World) {
        for agent in AGENTS {
            let wait = proposal(RateId::Wait { agent });
            if world.is_applicable(&wait) {
                world.apply(wait).unwrap();
            }
        }
        act(world, RateId::Tick);
    }
    fn until(world: &mut World, at: u64) {
        while clock(world) < at {
            tick(world);
        }
    }
    fn rejected_unchanged(world: &mut World, action: Action) {
        let before = world.state_key();
        assert!(world.apply(action).is_err());
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn workers_overlap_but_cannot_act_or_settle_early() {
        let mut world = world();
        for (agent, id) in [(AgentId::Atlas, JobId(1)), (AgentId::Bolt, JobId(2))] {
            act(&mut world, RateId::Claim { agent, job: id });
        }
        rejected_unchanged(
            &mut world,
            proposal(RateId::Claim {
                agent: AgentId::Coda,
                job: JobId(1),
            }),
        );
        rejected_unchanged(&mut world, proposal(RateId::Tick));
        tick(&mut world);
        for (agent, to) in [
            (AgentId::Atlas, Location::North),
            (AgentId::Bolt, Location::East),
        ] {
            act(
                &mut world,
                RateId::Move {
                    agent,
                    from: Location::Depot,
                    to,
                },
            );
        }
        tick(&mut world);
        for (agent, id) in [(AgentId::Atlas, JobId(1)), (AgentId::Bolt, JobId(2))] {
            act(
                &mut world,
                RateId::Begin {
                    agent,
                    job: id,
                    mode: WorkMode::Rush,
                },
            );
        }
        assert_eq!(
            world.balance(&AccountId::World, &Asset::Activities).get(),
            2
        );
        let finish = RateId::Finish {
            agent: AgentId::Atlas,
            job: JobId(1),
            mode: WorkMode::Rush,
            outcome: Outcome::Success,
        };
        rejected_unchanged(&mut world, proposal(finish));
        tick(&mut world);
        rejected_unchanged(
            &mut world,
            proposal(RateId::Move {
                agent: AgentId::Atlas,
                from: Location::North,
                to: Location::Depot,
            }),
        );
        until(&mut world, 4);
        act(&mut world, finish);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::Completed), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::Busy), 1);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::ElapsedTime), 4);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::ElapsedTime), 4);
    }

    #[test]
    fn shared_service_slot_stays_occupied_for_its_duration() {
        let mut world = world();
        for agent in [AgentId::Atlas, AgentId::Bolt] {
            act(
                &mut world,
                RateId::Move {
                    agent,
                    from: Location::Depot,
                    to: Location::West,
                },
            );
        }
        tick(&mut world);
        for agent in [AgentId::Atlas, AgentId::Bolt] {
            act(
                &mut world,
                RateId::Move {
                    agent,
                    from: Location::West,
                    to: Location::Workshop,
                },
            );
        }
        tick(&mut world);
        let start = |agent| RateId::BeginService {
            agent,
            facility: Facility::Workshop,
            maintenance: true,
        };
        act(&mut world, start(AgentId::Atlas));
        rejected_unchanged(&mut world, proposal(start(AgentId::Bolt)));
        let end = RateId::EndService {
            agent: AgentId::Atlas,
            facility: Facility::Workshop,
        };
        rejected_unchanged(&mut world, proposal(end));
        until(&mut world, 6);
        act(&mut world, end);
        act(&mut world, start(AgentId::Bolt));
        assert_eq!(
            world
                .balance(
                    &AccountId::Facility(Facility::Workshop),
                    &Asset::Occupied(Facility::Workshop, AgentId::Bolt)
                )
                .get(),
            1
        );
        rejected_unchanged(
            &mut world,
            proposal(start(AgentId::Atlas))
                .bind(Role::Facility, AccountId::Facility(Facility::Charger)),
        );
    }

    #[test]
    fn travel_uses_the_declared_road_network_and_policy_prices_each_hop() {
        let world = world();
        let agent = AgentId::Atlas;
        assert!(
            world
                .rate(&RateId::Move {
                    agent,
                    from: Location::Depot,
                    to: Location::Workshop
                })
                .is_none()
        );
        let (first, distance) = route(&world, agent, Location::Workshop).unwrap();
        assert_eq!(distance, 2);
        assert!(matches!(first, Location::South | Location::West));
        let action = travel(&world, agent, Location::Workshop).unwrap();
        assert_eq!(
            *action.rate(),
            RateId::Move {
                agent,
                from: Location::Depot,
                to: first
            }
        );
        assert_eq!(
            world
                .rate_ids()
                .filter(|id| matches!(
                    id,
                    RateId::Move {
                        agent: AgentId::Atlas,
                        ..
                    }
                ))
                .count(),
            18
        );
    }

    #[test]
    fn future_jobs_dependencies_leases_and_deadlines_are_core_enforced() {
        let mut world = world();
        rejected_unchanged(
            &mut world,
            proposal(RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(5),
            }),
        );
        rejected_unchanged(
            &mut world,
            proposal(RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(4),
            }),
        );
        act(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(1),
            },
        );
        let release = RateId::ReleaseClaim {
            agent: AgentId::Atlas,
            job: JobId(1),
            expired: true,
        };
        rejected_unchanged(&mut world, proposal(release));
        until(&mut world, 7);
        act(&mut world, release);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::FreeContract), 1);
        act(
            &mut world,
            RateId::Claim {
                agent: AgentId::Bolt,
                job: JobId(1),
            },
        );
        let expiry = RateId::Expire {
            job: JobId(1),
            agent: Some(AgentId::Bolt),
            mode: None,
        };
        rejected_unchanged(&mut world, proposal(expiry));
        until(&mut world, 24);
        act(&mut world, expiry);
        assert_eq!(job_balance(&world, JobId(1), Asset::Expired), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::FreeContract), 1);
        rejected_unchanged(&mut world, proposal(expiry));
        rejected_unchanged(
            &mut world,
            proposal(RateId::Claim {
                agent: AgentId::Coda,
                job: JobId(1),
            }),
        );
        rejected_unchanged(&mut world, proposal(RateId::EndSeason));
    }

    #[test]
    fn joint_handoff_moves_the_claim_and_payment_once() {
        let mut world = world();
        act(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(1),
            },
        );
        tick(&mut world);
        let handoff = RateId::Handoff {
            agent: AgentId::Atlas,
            recipient: AgentId::Bolt,
            job: JobId(1),
        };
        let wrong = proposal(handoff).bind(Role::Partner, AccountId::Agent(AgentId::Coda));
        rejected_unchanged(&mut world, wrong);
        act(&mut world, handoff);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::Credit), 7);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::Credit), 3);
        assert_eq!(balance(&world, AgentId::Atlas, Asset::FreeContract), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::Claimed(JobId(1))), 1);
        assert_eq!(
            job_balance(&world, JobId(1), Asset::Assigned(AgentId::Bolt)),
            1
        );
        assert!(
            decision(
                &build(Profile::Showcase, mixed_lineup()),
                &world,
                AgentId::Bolt
            )
            .is_none()
        );
        tick(&mut world);
        rejected_unchanged(
            &mut world,
            proposal(RateId::Handoff {
                agent: AgentId::Bolt,
                recipient: AgentId::Atlas,
                job: JobId(1),
            }),
        );
    }

    #[test]
    fn recycling_recovers_material_with_a_conserved_loss() {
        let mut world = world();
        let agent = AgentId::Atlas;
        act(
            &mut world,
            RateId::Claim {
                agent,
                job: JobId(1),
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::Depot,
                to: Location::North,
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Begin {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
            },
        );
        until(&mut world, 4);
        act(
            &mut world,
            RateId::Finish {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
                outcome: Outcome::Success,
            },
        );
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::North,
                to: Location::Recycler,
            },
        );
        tick(&mut world);
        let material = balance(&world, agent, Asset::Material);
        act(
            &mut world,
            RateId::BeginService {
                agent,
                facility: Facility::Recycler,
                maintenance: false,
            },
        );
        assert_eq!(balance(&world, agent, Asset::Material), material);
        until(&mut world, 7);
        act(
            &mut world,
            RateId::EndService {
                agent,
                facility: Facility::Recycler,
            },
        );
        assert_eq!(balance(&world, agent, Asset::Material), material + 1);
        assert_eq!(balance(&world, agent, Asset::Waste), 0);
        assert_eq!(balance(&world, agent, Asset::RecyclingLoss), 1);
        assert_eq!(balance(&world, agent, Asset::RecycledWaste), 2);
    }

    #[test]
    fn a_late_work_completion_cannot_collect_payment() {
        let mut world = world();
        let agent = AgentId::Atlas;
        act(
            &mut world,
            RateId::Claim {
                agent,
                job: JobId(1),
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::Depot,
                to: Location::North,
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Begin {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
            },
        );
        until(&mut world, 24);
        rejected_unchanged(
            &mut world,
            proposal(RateId::Finish {
                agent,
                job: JobId(1),
                mode: WorkMode::Rush,
                outcome: Outcome::Success,
            }),
        );
        act(
            &mut world,
            RateId::Expire {
                job: JobId(1),
                agent: Some(agent),
                mode: Some(WorkMode::Rush),
            },
        );
        assert_eq!(balance(&world, agent, Asset::Credit), 5);
        assert_eq!(balance(&world, agent, Asset::Busy), 0);
        assert_eq!(
            world.balance(&AccountId::World, &Asset::Activities).get(),
            0
        );
    }

    #[test]
    fn shared_scenarios_resolve_honestly_across_lineups_and_seeds() {
        let mut has_handoff = false;
        let mut has_recharge = false;
        let mut has_expiry = false;
        let mut has_overlap = false;
        for profile in [Profile::Showcase, Profile::Stress] {
            for lineup in [mixed_lineup(), throughput_lineup(), sustainable_lineup()] {
                let league = build(profile, lineup);
                for seed in [0, 1, 7, 11, 17, 29] {
                    let outcome = run(&league, seed).unwrap();
                    assert!(outcome.final_world.matches(&league.goal));
                    for spec in &league.jobs {
                        assert_eq!(
                            job_balance(&outcome.final_world, spec.id, Asset::Completed)
                                + job_balance(&outcome.final_world, spec.id, Asset::Expired),
                            1
                        );
                    }
                    let mut replay = league.initial.fork();
                    for action in outcome.trace.exchanges() {
                        replay.apply(action.clone()).unwrap();
                        has_overlap |=
                            replay.balance(&AccountId::World, &Asset::Activities).get() >= 2;
                        has_handoff |= matches!(action.rate(), RateId::Handoff { .. });
                        has_recharge |= matches!(
                            action.rate(),
                            RateId::BeginService {
                                facility: Facility::Charger,
                                ..
                            }
                        );
                        has_expiry |= matches!(action.rate(), RateId::Expire { .. });
                    }
                    assert_eq!(replay.state_key(), outcome.final_world.state_key());
                }
            }
        }
        assert!(has_handoff && has_recharge && has_expiry && has_overlap);
    }
}
