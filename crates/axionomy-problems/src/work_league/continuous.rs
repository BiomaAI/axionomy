//! Shared-time work economy. Events and worker decisions are proposed as
//! exchanges; time, deadlines, leases, activity duration and facility
//! ownership are laws. Each law is a rate with variables, serving every
//! worker, `{agent}`, and every contract, `{job}`, each role fixed to the
//! account its identifier names.

use super::*;
use axionomy::path;
use axionomy::{OptionPattern, QuantityComparison as C, Relation};

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

fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
fn now() -> Q<Ontology> {
    q(Role::World, Asset::Tick)
}
/// One event of the season, the world role fixed to the world.
fn law() -> Rate<Ontology> {
    Rate::new()
        .account(Role::World, Pattern::literal(AccountId::World))
        .preserve(Role::World, basket([(Asset::SeasonActive, 1)]))
        .condition(
            path!("one-event-at-a-time"),
            Q::units(),
            C::Equal,
            Q::constant(1),
        )
}
/// `rate` for each worker, `{agent}`.
fn actor(rate: Rate<Ontology>) -> Rate<Ontology> {
    by_worker(rate, &AGENTS)
}
/// The worker `{agent}` spending its decision this tick.
fn turn() -> Rate<Ontology> {
    actor(law())
        .consume_at(
            Role::World,
            one([AssetPattern::Permit(agent().var()).into()]),
        )
        .produce_at(
            Role::World,
            one([AssetPattern::Acted(agent().var()).into()]),
        )
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, q(role, asset))
}
fn occupy(rate: Rate<Ontology>, duration: Q<Ontology>) -> Rate<Ontology> {
    rate.consume(Role::Agent, basket([(Asset::Idle, 1)]))
        .produce(Role::Agent, basket([(Asset::Busy, 1)]))
        .produce(Role::World, basket([(Asset::Activities, 1)]))
        .produce_computed(Role::Agent, Asset::ReadyAt, now() + duration.clone())
        .condition(
            path!("activity-fits-the-season"),
            now() + duration,
            C::LessThanOrEqual,
            q(Role::World, Asset::Horizon),
        )
}
fn finish_activity(rate: Rate<Ontology>) -> Rate<Ontology> {
    clear(rate, Role::Agent, Asset::ReadyAt)
        .condition(
            path!("activity-is-due"),
            now(),
            C::GreaterThanOrEqual,
            q(Role::Agent, Asset::ReadyAt),
        )
        .consume(Role::Agent, basket([(Asset::Busy, 1)]))
        .produce(Role::Agent, basket([(Asset::Idle, 1)]))
        .consume(Role::World, basket([(Asset::Activities, 1)]))
}
/// The contract that closes each wave of four, the fourth, eighth and so
/// on, needs the wave's first delivered: the world holds its completion as
/// many times as `{job}` closes a wave, once or not at all. `{first}` is
/// the first contract of `{job}`'s wave.
fn prerequisite(rate: Rate<Ontology>) -> Rate<Ontology> {
    let first = Param::<u8>::new(path!("first"));
    let wave = |number: Q<Ontology>| Q::divide(number, Q::constant(4), Rounding::Floor);
    let earlier = || job().quantity() - Q::constant(1);
    rate.derive(
        first.name().clone(),
        wave(earlier()) * Q::constant(4) + Q::constant(1),
    )
    .preserve_computed_at(
        Role::World,
        AssetPattern::CompletedJob(Slot::Of(JobIdPattern(first.var()))).into(),
        wave(job().quantity()) - wave(earlier()),
    )
}
/// What the contract `{job}` names was claimed, and is still pending or in
/// progress, by the worker `{agent}` names.
fn assigned() -> Pattern<Asset> {
    AssetPattern::Assigned(agent().var()).into()
}
fn claimed() -> Pattern<Asset> {
    AssetPattern::Claimed(contract()).into()
}
fn awaiting() -> Pattern<Asset> {
    AssetPattern::Awaiting(contract(), mode().var()).into()
}

pub(super) fn build(profile: Profile, lineup: [Policy; 4]) -> League {
    let agents = AGENTS.to_vec();
    let jobs = (1..=profile.job_count())
        .map(|i| job_spec(JobId(i as u8)))
        .collect::<Vec<_>>();
    let horizon = if profile == Profile::Stress { 78 } else { 48 };
    let mut world_assets = basket([
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
        let mut assets = basket([(Asset::Vacant(facility), 1)]);
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
        let mut invariant =
            LinearInvariant::new(path!("one-service-slot").join(&path::write(&facility)))
                .weight(Asset::Vacant(facility), 1);
        for agent in AGENTS {
            invariant = invariant.weight(Asset::Occupied(facility, agent), 1);
        }
        builder = builder.invariant(invariant);
    }

    for spec in &jobs {
        let release = u64::from((spec.id.0 - 1) / 4) * 6;
        let deadline = release + 20 + u64::from(spec.id.0 % 3) * 3;
        let mut contract = terms(spec);
        for (asset, quantity) in [
            (Asset::Available, 1),
            (Asset::Transferable, 1),
            (Asset::Credit, spec.value),
            (Asset::ReleaseAt, release),
            (Asset::Deadline, deadline),
        ] {
            contract.insert(asset, Quantity::new(quantity));
        }
        builder = builder.account(AccountId::Job(spec.id), Account::from(contract));
    }

    let count = jobs.len() as u8;
    let job = |rate| on_contract(rate, count);
    let expire = |agent, mode| {
        RateIdPattern::Expire {
            job: contract(),
            agent: Slot::Of(agent),
            mode: Slot::Of(mode),
        }
        .into()
    };
    builder = builder
        .rate_at(
            expire(OptionPattern::None, OptionPattern::None),
            expiry(job(law())).consume(Role::Job, basket([(Asset::Available, 1)])),
        )
        .rate_at(
            expire(OptionPattern::Some(agent().var()), OptionPattern::None),
            clear(abandoned(actor(job(law()))), Role::Job, Asset::LeaseUntil)
                .consume(Role::Job, basket([(Asset::Pending, 1)])),
        )
        .rate_at(
            expire(
                OptionPattern::Some(agent().var()),
                OptionPattern::Some(mode().var()),
            ),
            finish_activity(abandoned(actor(job(law()))).variable(mode().every()))
                .consume_at(Role::Agent, one([awaiting()]))
                .consume(Role::Job, basket([(Asset::InProgress, 1)])),
        );
    builder = builder.rate_at(
        RateIdPattern::Claim {
            agent: agent().var(),
            job: contract(),
        }
        .into(),
        prerequisite(job(turn()))
            .preserve(
                Role::Agent,
                basket([(Asset::Idle, 1), (Asset::Operational, 1)]),
            )
            .consume(Role::Agent, basket([(Asset::FreeContract, 1)]))
            .produce_at(Role::Agent, one([claimed()]))
            .consume(Role::Job, basket([(Asset::Available, 1)]))
            .produce_at(
                Role::Job,
                one([assigned(), Pattern::literal(Asset::Pending)]),
            )
            .produce_computed(Role::Job, Asset::LeaseUntil, now() + Q::constant(LEASE))
            .condition(
                path!("contract-has-arrived"),
                now(),
                C::GreaterThanOrEqual,
                q(Role::Job, Asset::ReleaseAt),
            )
            .condition(
                path!("contract-remains-open"),
                now() + Q::constant(1),
                C::LessThanOrEqual,
                q(Role::Job, Asset::Deadline),
            ),
    );
    for expired in [false, true] {
        // A claim released by its worker spends its turn; one whose lease
        // lapsed is released as an event.
        let base = if expired { actor(law()) } else { turn() };
        let release = clear(job(base), Role::Job, Asset::LeaseUntil)
            .consume_at(Role::Agent, one([claimed()]))
            .produce(Role::Agent, basket([(Asset::FreeContract, 1)]))
            .consume_at(
                Role::Job,
                one([assigned(), Pattern::literal(Asset::Pending)]),
            )
            .produce(Role::Job, basket([(Asset::Available, 1)]));
        let release = if expired {
            release.condition(
                path!("claim-lease-has-elapsed"),
                now(),
                C::GreaterThanOrEqual,
                q(Role::Job, Asset::LeaseUntil) + Q::constant(1),
            )
        } else {
            release.preserve(Role::Agent, basket([(Asset::Idle, 1)]))
        };
        builder = builder.rate_at(
            RateIdPattern::ReleaseClaim {
                agent: agent().var(),
                job: contract(),
                expired: Slot::Is(expired),
            }
            .into(),
            release,
        );
    }
    for mode in MODES {
        let cost = cost(mode);
        let begin = occupy(
            clear(prerequisite(job(turn())), Role::Job, Asset::LeaseUntil),
            cost.time.clone(),
        );
        let begin = at_the_contract(begin)
            .preserve(Role::Agent, basket([(Asset::Operational, 1)]))
            .preserve_at(Role::Agent, one([claimed()]))
            .consume_computed(Role::Agent, Asset::Energy, cost.energy.clone())
            .consume_computed(Role::Agent, Asset::Material, cost.material.clone())
            .produce_computed(Role::Agent, Asset::SpentEnergy, cost.energy)
            .produce_computed(Role::Agent, Asset::MaterialSpent, cost.material.clone())
            .produce_computed(Role::Agent, Asset::Waste, cost.waste.clone())
            .produce_computed(
                Role::Agent,
                Asset::EmbodiedMaterial,
                cost.material - cost.waste,
            )
            .produce(
                Role::Agent,
                basket([
                    (Asset::Attempts, 1),
                    (Asset::Wear, if mode == WorkMode::Rush { 2 } else { 1 }),
                ]),
            )
            .produce_at(
                Role::Agent,
                one([AssetPattern::Awaiting(contract(), Slot::Is(mode)).into()]),
            )
            .preserve_at(Role::Job, one([assigned()]))
            .consume(Role::Job, basket([(Asset::Pending, 1)]))
            .produce(
                Role::Job,
                basket([(Asset::InProgress, 1), (Asset::Attempts, 1)]),
            )
            .condition(
                path!("claim-lease-remains-valid"),
                now(),
                C::LessThanOrEqual,
                q(Role::Job, Asset::LeaseUntil),
            )
            .condition(
                path!("work-finishes-by-deadline"),
                now() + cost.time,
                C::LessThanOrEqual,
                q(Role::Job, Asset::Deadline),
            )
            .condition(
                path!("equipment-needs-maintenance-below-wear-six"),
                q(Role::Agent, Asset::Wear),
                C::LessThanOrEqual,
                Q::constant(5),
            );
        builder = builder.rate_at(
            RateIdPattern::Begin {
                agent: agent().var(),
                job: contract(),
                mode: Slot::Is(mode),
            }
            .into(),
            begin,
        );
    }
    for outcome in [Outcome::Success, Outcome::Failure] {
        let finish = finish_activity(job(actor(law())).variable(mode().every()))
            .account(Role::Nature, Pattern::literal(AccountId::Nature))
            .preserve_at(
                Role::Nature,
                one([
                    AssetPattern::OutcomeWeight(contract(), mode().var(), Slot::Is(outcome)).into(),
                ]),
            )
            .consume_at(Role::Agent, one([awaiting()]))
            .consume(Role::Job, basket([(Asset::InProgress, 1)]))
            .condition(
                path!("outcome-settles-before-expiry"),
                now(),
                C::LessThanOrEqual,
                q(Role::Job, Asset::Deadline),
            );
        let reward = || q(Role::Job, Asset::Reward);
        let finish = match outcome {
            Outcome::Success => finish
                .consume_at(Role::Agent, one([claimed()]))
                .produce(
                    Role::Agent,
                    basket([
                        (Asset::FreeContract, 1),
                        (Asset::Completed, 1),
                        (Asset::Successes, 1),
                    ]),
                )
                .produce_computed(Role::Agent, Asset::Value, reward())
                .produce_computed(Role::Agent, Asset::Credit, reward())
                .consume_at(Role::Job, one([assigned()]))
                .consume_computed(Role::Job, Asset::Credit, reward())
                .produce(Role::Job, basket([(Asset::Completed, 1)]))
                .consume(Role::World, basket([(Asset::ContractsOpen, 1)]))
                .produce(Role::World, basket([(Asset::ContractsClosed, 1)]))
                .produce_at(
                    Role::World,
                    one([AssetPattern::CompletedJob(contract()).into()]),
                ),
            Outcome::Failure => finish
                .preserve_at(Role::Agent, one([claimed()]))
                .consume(Role::Agent, basket([(Asset::Operational, 1)]))
                .produce(
                    Role::Agent,
                    basket([(Asset::Damage, 1), (Asset::Failures, 1)]),
                )
                .preserve_at(Role::Job, one([assigned()]))
                .produce(Role::Job, basket([(Asset::Pending, 1)]))
                .produce_computed(Role::Job, Asset::LeaseUntil, now() + Q::constant(LEASE)),
        };
        builder = builder.rate_at(
            RateIdPattern::Finish {
                agent: agent().var(),
                job: contract(),
                mode: mode().var(),
                outcome: Slot::Is(outcome),
            }
            .into(),
            finish,
        );
    }
    // A joint proposal spends both actors' decision permissions. A
    // contract can be handed off once, preventing referral farming.
    let recipient = Param::<AgentId>::new(path!("recipient"));
    builder = builder.rate_at(
        RateIdPattern::Handoff {
            agent: agent().var(),
            recipient: recipient.var(),
            job: contract(),
        }
        .into(),
        clear(
            job(turn()
                .variable(recipient.every())
                .distinct_variables(agent().name().clone(), recipient.name().clone())),
            Role::Job,
            Asset::LeaseUntil,
        )
        .account(
            Role::Partner,
            AccountIdPattern::Agent(recipient.var()).into(),
        )
        .consume_at(
            Role::World,
            one([AssetPattern::Permit(recipient.var()).into()]),
        )
        .produce_at(
            Role::World,
            one([AssetPattern::Acted(recipient.var()).into()]),
        )
        .preserve(Role::Agent, basket([(Asset::Idle, 1)]))
        .consume_at(Role::Agent, one([claimed()]))
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
            basket([(Asset::Idle, 1), (Asset::Operational, 1)]),
        )
        .consume(
            Role::Partner,
            basket([(Asset::FreeContract, 1), (Asset::Credit, 2)]),
        )
        .produce_at(Role::Partner, one([claimed()]))
        .preserve(Role::Job, basket([(Asset::Pending, 1)]))
        .consume_at(
            Role::Job,
            one([assigned(), Pattern::literal(Asset::Transferable)]),
        )
        .produce_at(
            Role::Job,
            one([
                AssetPattern::Assigned(recipient.var()).into(),
                Pattern::literal(Asset::HandedOff),
            ]),
        )
        .produce_computed(Role::Job, Asset::LeaseUntil, now() + Q::constant(LEASE))
        .condition(
            path!("handoff-before-deadline"),
            now() + Q::constant(1),
            C::LessThanOrEqual,
            q(Role::Job, Asset::Deadline),
        ),
    );
    builder = builder.rate_at(
        RateIdPattern::Wait {
            agent: agent().var(),
        }
        .into(),
        turn(),
    );
    // The roads never change: a worker moves along one, either way, a hop a
    // turn.
    let (from, to) = (
        Param::<Location>::new(path!("from")),
        Param::new(path!("to")),
    );
    let roads = LOCATIONS.into_iter().flat_map(|from| {
        LOCATIONS
            .into_iter()
            .filter(move |to| ROUTES.contains(&(from, *to)) || ROUTES.contains(&(*to, from)))
            .map(move |to| (from, to))
    });
    builder = builder.rate_at(
        RateIdPattern::Move {
            agent: agent().var(),
            from: from.var(),
            to: to.var(),
        }
        .into(),
        turn()
            .variable(from.every())
            .variable(to.every())
            .relation(Relation::pairs(&from, &to, roads))
            .preserve(Role::Agent, basket([(Asset::Idle, 1)]))
            .consume_at(
                Role::Agent,
                [
                    (AssetPattern::At(from.var()).into(), Quantity::new(1)),
                    (Pattern::literal(Asset::Energy), Quantity::new(1)),
                ],
            )
            .produce_at(
                Role::Agent,
                [
                    (AssetPattern::At(to.var()).into(), Quantity::new(1)),
                    (Pattern::literal(Asset::SpentEnergy), Quantity::new(1)),
                ],
            ),
    );
    for facility in FACILITIES {
        for maintenance in [false, true] {
            if maintenance && facility != Facility::Workshop {
                continue;
            }
            builder = builder.rate_at(
                RateIdPattern::BeginService {
                    agent: agent().var(),
                    facility: Slot::Is(facility),
                    maintenance: Slot::Is(maintenance),
                }
                .into(),
                service_rate(facility, maintenance),
            );
        }
        let end = finish_activity(at_facility(actor(law()), facility))
            .consume(Role::Agent, basket([(Asset::Servicing(facility), 1)]))
            .consume_at(
                Role::Facility,
                one([AssetPattern::Occupied(Slot::Is(facility), agent().var()).into()]),
            )
            .produce(Role::Facility, basket([(Asset::Vacant(facility), 1)]));
        let end = match facility {
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
        builder = builder.rate_at(
            RateIdPattern::EndService {
                agent: agent().var(),
                facility: Slot::Is(facility),
            }
            .into(),
            end,
        );
    }
    for agent in AGENTS {
        builder = builder.invariant(
            LinearInvariant::new(path!("decision-permission").join(&path::write(&agent)))
                .weight(Asset::Permit(agent), 1)
                .weight(Asset::Acted(agent), 1),
        );
    }
    let mut tick = law()
        .condition(
            path!("season-has-time-remaining"),
            now() + Q::constant(1),
            C::LessThanOrEqual,
            q(Role::World, Asset::Horizon),
        )
        .produce(Role::World, basket([(Asset::Tick, 1)]));
    for agent in AGENTS {
        tick = tick
            .consume(Role::World, basket([(Asset::Acted(agent), 1)]))
            .produce(Role::World, basket([(Asset::Permit(agent), 1)]))
            .account(
                Role::Worker(agent),
                Pattern::literal(AccountId::Agent(agent)),
            )
            .consume(Role::Worker(agent), basket([(Asset::TimeRemaining, 1)]))
            .produce(Role::Worker(agent), basket([(Asset::ElapsedTime, 1)]));
    }
    builder = builder
        .rate(RateId::Tick, tick)
        .rate(
            RateId::EndSeason,
            Rate::new()
                .account(Role::World, Pattern::literal(AccountId::World))
                .consume(Role::World, basket([(Asset::SeasonActive, 1)]))
                .produce(Role::World, basket([(Asset::SeasonEnded, 1)]))
                .condition(
                    path!("all-contracts-resolved"),
                    q(Role::World, Asset::ContractsOpen),
                    C::Equal,
                    Q::constant(0),
                )
                .condition(
                    path!("all-activities-settled"),
                    q(Role::World, Asset::Activities),
                    C::Equal,
                    Q::constant(0),
                ),
        )
        .invariant(
            LinearInvariant::new(path!("season-lifecycle"))
                .weight(Asset::SeasonActive, 1)
                .weight(Asset::SeasonEnded, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-contracts"))
                .weight(Asset::ContractsOpen, 1)
                .weight(Asset::ContractsClosed, 1),
        )
        .invariant(
            LinearInvariant::new(path!("worker-availability"))
                .weight(Asset::Idle, 1)
                .weight(Asset::Busy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("worker-condition"))
                .weight(Asset::Operational, 1)
                .weight(Asset::Damage, 1)
                .weight(Asset::Repairing, 1),
        )
        .invariant(
            LinearInvariant::new(path!("energy-lifecycle"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1)
                .weight(Asset::ChargeSupply, 1)
                .weight(Asset::ChargingEnergy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("material-lifecycle"))
                .weight(Asset::Material, 1)
                .weight(Asset::Waste, 1)
                .weight(Asset::EmbodiedMaterial, 1)
                .weight(Asset::ReclaimingMaterial, 1)
                .weight(Asset::RecyclingLoss, 1),
        )
        .invariant(LinearInvariant::new(path!("closed-settlement-credit")).weight(Asset::Credit, 1))
        .invariant(
            LinearInvariant::new(path!("repair-supplies"))
                .weight(Asset::RepairSupply, 1)
                .weight(Asset::SpentRepairSupply, 1),
        )
        .invariant(
            LinearInvariant::new(path!("worker-time"))
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        );
    let mut claims =
        LinearInvariant::new(path!("one-contract-per-worker")).weight(Asset::FreeContract, 1);
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

/// A contract missing its deadline: it closes unpaid.
fn expiry(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.condition(
        path!("contract-deadline-has-elapsed"),
        now(),
        C::GreaterThanOrEqual,
        q(Role::Job, Asset::Deadline) + Q::constant(1),
    )
    .produce(Role::Job, basket([(Asset::Expired, 1)]))
    .consume(Role::World, basket([(Asset::ContractsOpen, 1)]))
    .produce(Role::World, basket([(Asset::ContractsClosed, 1)]))
}

/// A contract expiring while the worker `{agent}` holds it: the worker is
/// free of it.
fn abandoned(rate: Rate<Ontology>) -> Rate<Ontology> {
    expiry(rate)
        .consume_at(Role::Agent, one([claimed()]))
        .produce(Role::Agent, basket([(Asset::FreeContract, 1)]))
        .consume_at(Role::Job, one([assigned()]))
}

/// `rate` at `facility`, the facility role fixed to it.
fn at_facility(rate: Rate<Ontology>, facility: Facility) -> Rate<Ontology> {
    rate.account(
        Role::Facility,
        Pattern::literal(AccountId::Facility(facility)),
    )
}

fn service_rate(facility: Facility, maintenance: bool) -> Rate<Ontology> {
    let duration = match facility {
        Facility::Workshop => 4,
        Facility::Charger => 3,
        Facility::Recycler => 2,
    };
    let rate = occupy(at_facility(turn(), facility), Q::constant(duration))
        .preserve(
            Role::Agent,
            basket([(Asset::At(service_location(facility)), 1)]),
        )
        .produce(Role::Agent, basket([(Asset::Servicing(facility), 1)]))
        .consume(Role::Facility, basket([(Asset::Vacant(facility), 1)]))
        .produce_at(
            Role::Facility,
            one([AssetPattern::Occupied(Slot::Is(facility), agent().var()).into()]),
        );
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

/// The season's clock: the ticks that have passed.
pub fn clock(world: &World) -> u64 {
    world.balance(&AccountId::World, &Asset::Tick).to_u64()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
        world.balance(&AccountId::Agent(agent), &asset).to_u64()
    }
    fn job_balance(world: &World, id: JobId, asset: Asset) -> u64 {
        world.balance(&AccountId::Job(id), &asset).to_u64()
    }
    fn world() -> World {
        build(Profile::Showcase, mixed_lineup()).initial
    }
    fn act(world: &mut World, rate: RateId) {
        world.apply(action(world, rate)).unwrap();
    }
    fn tick(world: &mut World) {
        for agent in AGENTS {
            let wait = action(world, RateId::Wait { agent });
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
    fn rejected_unchanged(world: &mut World, exchange: Action) {
        let before = world.state_key();
        assert!(world.apply(exchange).is_err());
        assert_eq!(world.state_key(), before);
    }
    fn refused(world: &mut World, rate: RateId) {
        let exchange = action(world, rate);
        rejected_unchanged(world, exchange);
    }

    #[test]
    fn workers_overlap_but_cannot_act_or_settle_early() {
        let mut world = world();
        for (agent, id) in [(AgentId::Atlas, JobId(1)), (AgentId::Bolt, JobId(2))] {
            act(&mut world, RateId::Claim { agent, job: id });
        }
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Coda,
                job: JobId(1),
            },
        );
        refused(&mut world, RateId::Tick);
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
            world
                .balance(&AccountId::World, &Asset::Activities)
                .to_u64(),
            2
        );
        let finish = RateId::Finish {
            agent: AgentId::Atlas,
            job: JobId(1),
            mode: WorkMode::Rush,
            outcome: Outcome::Success,
        };
        refused(&mut world, finish);
        tick(&mut world);
        refused(
            &mut world,
            RateId::Move {
                agent: AgentId::Atlas,
                from: Location::North,
                to: Location::Depot,
            },
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
        refused(&mut world, start(AgentId::Bolt));
        let end = RateId::EndService {
            agent: AgentId::Atlas,
            facility: Facility::Workshop,
        };
        refused(&mut world, end);
        until(&mut world, 6);
        act(&mut world, end);
        act(&mut world, start(AgentId::Bolt));
        assert_eq!(
            world
                .balance(
                    &AccountId::Facility(Facility::Workshop),
                    &Asset::Occupied(Facility::Workshop, AgentId::Bolt)
                )
                .to_u64(),
            1
        );
        let elsewhere = action(&world, start(AgentId::Atlas))
            .bind(Role::Facility, AccountId::Facility(Facility::Charger));
        rejected_unchanged(&mut world, elsewhere);
    }

    #[test]
    fn travel_uses_the_declared_road_network_one_hop_a_turn() {
        let mut world = world();
        let agent = AgentId::Atlas;
        assert!(
            world
                .instance(&RateId::Move {
                    agent,
                    from: Location::Depot,
                    to: Location::Workshop
                })
                .is_err()
        );
        assert_eq!(
            world
                .rate_ids()
                .flat_map(|key| world.instances_of(key))
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
        // The workshop is two roads from the depot, one hop a turn.
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::Depot,
                to: Location::West,
            },
        );
        refused(
            &mut world,
            RateId::Move {
                agent,
                from: Location::West,
                to: Location::Workshop,
            },
        );
        tick(&mut world);
        act(
            &mut world,
            RateId::Move {
                agent,
                from: Location::West,
                to: Location::Workshop,
            },
        );
        assert_eq!(balance(&world, agent, Asset::At(Location::Workshop)), 1);
    }

    #[test]
    fn future_jobs_dependencies_leases_and_deadlines_are_core_enforced() {
        let mut world = world();
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(5),
            },
        );
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(4),
            },
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
        refused(&mut world, release);
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
        refused(&mut world, expiry);
        until(&mut world, 24);
        act(&mut world, expiry);
        assert_eq!(job_balance(&world, JobId(1), Asset::Expired), 1);
        assert_eq!(balance(&world, AgentId::Bolt, Asset::FreeContract), 1);
        refused(&mut world, expiry);
        refused(
            &mut world,
            RateId::Claim {
                agent: AgentId::Coda,
                job: JobId(1),
            },
        );
        refused(&mut world, RateId::EndSeason);
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
        let wrong = action(&world, handoff).bind(Role::Partner, AccountId::Agent(AgentId::Coda));
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
        // The handoff spent Bolt's turn too.
        refused(
            &mut world,
            RateId::Wait {
                agent: AgentId::Bolt,
            },
        );
        tick(&mut world);
        refused(
            &mut world,
            RateId::Handoff {
                agent: AgentId::Bolt,
                recipient: AgentId::Atlas,
                job: JobId(1),
            },
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
        refused(
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
            RateId::Expire {
                job: JobId(1),
                agent: Some(agent),
                mode: Some(WorkMode::Rush),
            },
        );
        assert_eq!(balance(&world, agent, Asset::Credit), 5);
        assert_eq!(balance(&world, agent, Asset::Busy), 0);
        assert_eq!(
            world
                .balance(&AccountId::World, &Asset::Activities)
                .to_u64(),
            0
        );
    }
}
