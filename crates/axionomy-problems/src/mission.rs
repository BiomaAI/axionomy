//! Partially observed, stochastic two-agent reconnaissance mission.

use axionomy::{
    Account, EconomicView, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant,
    ObservationKey, Quantity, Rate, Trace, basket,
};
use axionomy_search::{
    action_source::{ActionSource, lazy_actions},
    ismcts::{InformationState, IsmctsResult, search as information_set_search},
    mcts::MctsConfig,
    monte_carlo::{BernoulliStatistics, MonteCarloConfig, evaluate},
    rollout::{RolloutConfig, RolloutDecision, RolloutStop, TraceRetention, run_to_goal},
    sampling::{
        SeededSampler, TicketSource, WeightedExchange, choose_by_ticket, sample, systematic_ticket,
        total_weight,
    },
};

const HORIZON: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentId {
    Scout,
    Medic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    Base,
    North,
    South,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hazard {
    Safe,
    Injury,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Agent(AgentId),
    Nature,
    Mission,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    AgentIdentity(AgentId),
    NatureIdentity,
    MissionIdentity,
    SuccessIdentity,
    At(Location),
    Energy,
    SpentEnergy,
    Sensor,
    UsedSensor,
    MedicalKit,
    UsedMedicalKit,
    Intel(Location),
    SharedIntel(Location),
    Injured,
    Unresolved,
    ScenarioWeight(Location, u8, Hazard),
    Truth(Location),
    Seed(u8),
    Hazard(Hazard),
    HazardResolved,
    TimeRemaining,
    ElapsedTime,
    Planning,
    AwaitingScan,
    AwaitingEncounter,
    NeedsTreatment,
    AreaSafe,
    Rescued,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Scout,
    Medic,
    Nature,
    Mission,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Instantiate {
        truth: Location,
        seed: u8,
        hazard: Hazard,
    },
    BeginScan,
    ResolveScan {
        truth: Location,
        seed: u8,
        report: Location,
        next_seed: u8,
    },
    Share(Location),
    MoveTogether(Location),
    MoveDirect(Location),
    Encounter {
        location: Location,
        hazard: Hazard,
    },
    Treat,
    Rescue(Location),
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Policy {
    ShareAndCoordinate,
    NorthTogether,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type AgentView<'a> = EconomicView<'a, AccountId, Asset, RateId, Role>;
pub type MissionObservation = ObservationKey<AccountId, Asset>;
pub type MissionInformation = InformationState<MissionObservation>;

#[derive(Debug, Clone)]
pub struct MissionRollout {
    trace: Trace<RateId, Role, AccountId>,
    succeeded: bool,
    elapsed_time: u64,
    used_medical_kit: bool,
}

impl MissionRollout {
    pub const fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub const fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub const fn elapsed_time(&self) -> u64 {
        self.elapsed_time
    }

    pub const fn used_medical_kit(&self) -> bool {
        self.used_medical_kit
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyComparison {
    chosen: Policy,
    coordinated_successes: usize,
    direct_successes: usize,
    samples: usize,
}

impl PolicyComparison {
    pub const fn chosen(self) -> Policy {
        self.chosen
    }

    pub const fn coordinated_successes(self) -> usize {
        self.coordinated_successes
    }

    pub const fn direct_successes(self) -> usize {
        self.direct_successes
    }

    pub const fn samples(self) -> usize {
        self.samples
    }
}

pub fn initial() -> World {
    let mut nature = basket([(Asset::Unresolved, 1)]);
    for truth in [Location::North, Location::South] {
        for seed in 0..4 {
            for hazard in [Hazard::Safe, Hazard::Injury] {
                nature.insert(Asset::ScenarioWeight(truth, seed, hazard), Quantity::new(1));
            }
        }
    }

    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent(AgentId::Scout),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::Scout), 1),
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::Sensor, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::Medic),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::Medic), 1),
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::MedicalKit, 1),
            ])),
        )
        .account(AccountId::Nature, {
            nature.insert(Asset::NatureIdentity, Quantity::new(1));
            Account::from(nature)
        })
        .account(
            AccountId::Mission,
            Account::from(basket([
                (Asset::MissionIdentity, 1),
                (Asset::TimeRemaining, 10),
                (Asset::Planning, 1),
            ])),
        )
        .account(
            AccountId::Success,
            Account::from(basket([(Asset::SuccessIdentity, 1)])),
        );

    for truth in [Location::North, Location::South] {
        for seed in 0..4 {
            for hazard in [Hazard::Safe, Hazard::Injury] {
                builder = builder.rate(
                    RateId::Instantiate {
                        truth,
                        seed,
                        hazard,
                    },
                    Rate::new()
                        .preserve(Role::Nature, basket([(Asset::NatureIdentity, 1)]))
                        .consume(Role::Nature, basket([(Asset::Unresolved, 1)]))
                        .preserve(
                            Role::Nature,
                            basket([(Asset::ScenarioWeight(truth, seed, hazard), 1)]),
                        )
                        .produce(
                            Role::Nature,
                            basket([
                                (Asset::Truth(truth), 1),
                                (Asset::Seed(seed), 1),
                                (Asset::Hazard(hazard), 1),
                            ]),
                        ),
                );
            }

            let report = signal(truth, seed);
            builder = builder.rate(
                RateId::ResolveScan {
                    truth,
                    seed,
                    report,
                    next_seed: (seed + 1) % 4,
                },
                Rate::new()
                    .preserve(
                        Role::Scout,
                        basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::NatureIdentity, 1)]))
                    .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                    .produce(Role::Scout, basket([(Asset::Intel(report), 1)]))
                    .preserve(Role::Nature, basket([(Asset::Truth(truth), 1)]))
                    .consume(Role::Nature, basket([(Asset::Seed(seed), 1)]))
                    .produce(Role::Nature, basket([(Asset::Seed((seed + 1) % 4), 1)]))
                    .consume(Role::Mission, basket([(Asset::AwaitingScan, 1)]))
                    .produce(Role::Mission, basket([(Asset::Planning, 1)]))
                    .distinct(Role::Scout, Role::Nature)
                    .distinct(Role::Scout, Role::Mission)
                    .distinct(Role::Nature, Role::Mission),
            );
        }
    }

    builder = builder.rate(
        RateId::BeginScan,
        Rate::new()
            .preserve(
                Role::Scout,
                basket([
                    (Asset::AgentIdentity(AgentId::Scout), 1),
                    (Asset::At(Location::Base), 1),
                ]),
            )
            .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
            .consume(Role::Scout, basket([(Asset::Sensor, 1)]))
            .produce(Role::Scout, basket([(Asset::UsedSensor, 1)]))
            .consume(
                Role::Mission,
                basket([(Asset::Planning, 1), (Asset::TimeRemaining, 1)]),
            )
            .produce(
                Role::Mission,
                basket([(Asset::AwaitingScan, 1), (Asset::ElapsedTime, 1)]),
            )
            .distinct(Role::Scout, Role::Mission),
    );

    for location in [Location::North, Location::South] {
        builder = builder
            .rate(
                RateId::Share(location),
                Rate::new()
                    .preserve(
                        Role::Scout,
                        basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
                    )
                    .preserve(
                        Role::Medic,
                        basket([(Asset::AgentIdentity(AgentId::Medic), 1)]),
                    )
                    .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                    .consume(Role::Scout, basket([(Asset::Intel(location), 1)]))
                    .produce(Role::Scout, basket([(Asset::SharedIntel(location), 1)]))
                    .produce(Role::Medic, basket([(Asset::Intel(location), 1)]))
                    .preserve(Role::Mission, basket([(Asset::Planning, 1)]))
                    .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
                    .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)]))
                    .distinct(Role::Scout, Role::Medic)
                    .distinct(Role::Scout, Role::Mission)
                    .distinct(Role::Medic, Role::Mission),
            )
            .rate(
                RateId::MoveTogether(location),
                movement_rate(location, true),
            )
            .rate(RateId::MoveDirect(location), movement_rate(location, false));

        for hazard in [Hazard::Safe, Hazard::Injury] {
            let rate = Rate::new()
                .preserve(
                    Role::Scout,
                    basket([
                        (Asset::AgentIdentity(AgentId::Scout), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Medic,
                    basket([
                        (Asset::AgentIdentity(AgentId::Medic), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Nature,
                    basket([(Asset::NatureIdentity, 1), (Asset::Truth(location), 1)]),
                )
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .consume(Role::Nature, basket([(Asset::Hazard(hazard), 1)]))
                .produce(Role::Nature, basket([(Asset::HazardResolved, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::AwaitingEncounter, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)]))
                .distinct(Role::Scout, Role::Medic)
                .distinct(Role::Scout, Role::Nature)
                .distinct(Role::Scout, Role::Mission)
                .distinct(Role::Medic, Role::Nature)
                .distinct(Role::Medic, Role::Mission)
                .distinct(Role::Nature, Role::Mission);
            builder = builder.rate(
                RateId::Encounter { location, hazard },
                match hazard {
                    Hazard::Safe => rate.produce(Role::Mission, basket([(Asset::AreaSafe, 1)])),
                    Hazard::Injury => rate
                        .produce(Role::Scout, basket([(Asset::Injured, 1)]))
                        .produce(Role::Mission, basket([(Asset::NeedsTreatment, 1)])),
                },
            );
        }

        builder = builder.rate(
            RateId::Rescue(location),
            Rate::new()
                .preserve(
                    Role::Scout,
                    basket([
                        (Asset::AgentIdentity(AgentId::Scout), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Medic,
                    basket([
                        (Asset::AgentIdentity(AgentId::Medic), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Nature,
                    basket([(Asset::NatureIdentity, 1), (Asset::Truth(location), 1)]),
                )
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::AreaSafe, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::Mission,
                    basket([(Asset::Rescued, 1), (Asset::ElapsedTime, 1)]),
                )
                .distinct(Role::Scout, Role::Medic)
                .distinct(Role::Scout, Role::Nature)
                .distinct(Role::Scout, Role::Mission)
                .distinct(Role::Medic, Role::Nature)
                .distinct(Role::Medic, Role::Mission)
                .distinct(Role::Nature, Role::Mission),
        );
    }

    builder
        .rate(
            RateId::Treat,
            Rate::new()
                .preserve(
                    Role::Scout,
                    basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
                )
                .preserve(
                    Role::Medic,
                    basket([(Asset::AgentIdentity(AgentId::Medic), 1)]),
                )
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .consume(Role::Scout, basket([(Asset::Injured, 1)]))
                .consume(Role::Medic, basket([(Asset::MedicalKit, 1)]))
                .produce(Role::Medic, basket([(Asset::UsedMedicalKit, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::NeedsTreatment, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::Mission,
                    basket([(Asset::AreaSafe, 1), (Asset::ElapsedTime, 1)]),
                )
                .distinct(Role::Scout, Role::Medic)
                .distinct(Role::Scout, Role::Mission)
                .distinct(Role::Medic, Role::Mission),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .preserve(Role::Goal, basket([(Asset::SuccessIdentity, 1)]))
                .consume(Role::Mission, basket([(Asset::Rescued, 1)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::Mission, Role::Goal),
        )
        .invariant(
            [Location::Base, Location::North, Location::South]
                .into_iter()
                .fold(
                    LinearInvariant::new("two agent positions"),
                    |invariant, location| invariant.weight(Asset::At(location), 1),
                ),
        )
        .invariant(
            LinearInvariant::new("energy accounting")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("time accounting")
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        )
        .invariant(
            LinearInvariant::new("sensor accounting")
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant(
            LinearInvariant::new("medical-kit accounting")
                .weight(Asset::MedicalKit, 1)
                .weight(Asset::UsedMedicalKit, 1),
        )
        .invariant(
            LinearInvariant::new("mission lifecycle")
                .weight(Asset::Planning, 1)
                .weight(Asset::AwaitingScan, 1)
                .weight(Asset::AwaitingEncounter, 1)
                .weight(Asset::NeedsTreatment, 1)
                .weight(Asset::AreaSafe, 1)
                .weight(Asset::Rescued, 1)
                .weight(Asset::Solved, 1),
        )
        .invariant([Location::North, Location::South].into_iter().fold(
            LinearInvariant::new("one hidden truth").weight(Asset::Unresolved, 1),
            |invariant, location| invariant.weight(Asset::Truth(location), 1),
        ))
        .invariant((0..4).fold(
            LinearInvariant::new("one hidden seed").weight(Asset::Unresolved, 1),
            |invariant, seed| invariant.weight(Asset::Seed(seed), 1),
        ))
        .invariant(
            [Hazard::Safe, Hazard::Injury].into_iter().fold(
                LinearInvariant::new("one hidden hazard")
                    .weight(Asset::Unresolved, 1)
                    .weight(Asset::HazardResolved, 1),
                |invariant, hazard| invariant.weight(Asset::Hazard(hazard), 1),
            ),
        )
        .build()
        .expect("mission model is valid")
}

fn movement_rate(location: Location, coordinated: bool) -> Rate<Role, Asset> {
    let rate = Rate::new()
        .preserve(
            Role::Scout,
            basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
        )
        .preserve(
            Role::Medic,
            basket([(Asset::AgentIdentity(AgentId::Medic), 1)]),
        )
        .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
        .consume(
            Role::Scout,
            basket([(Asset::At(Location::Base), 1), (Asset::Energy, 1)]),
        )
        .produce(
            Role::Scout,
            basket([(Asset::At(location), 1), (Asset::SpentEnergy, 1)]),
        )
        .consume(
            Role::Medic,
            basket([(Asset::At(Location::Base), 1), (Asset::Energy, 1)]),
        )
        .produce(
            Role::Medic,
            basket([(Asset::At(location), 1), (Asset::SpentEnergy, 1)]),
        )
        .consume(Role::Mission, basket([(Asset::Planning, 1)]))
        .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
        .produce(
            Role::Mission,
            basket([(Asset::AwaitingEncounter, 1), (Asset::ElapsedTime, 1)]),
        )
        .distinct(Role::Scout, Role::Medic)
        .distinct(Role::Scout, Role::Mission)
        .distinct(Role::Medic, Role::Mission);
    if coordinated {
        rate.preserve(Role::Scout, basket([(Asset::SharedIntel(location), 1)]))
            .preserve(Role::Medic, basket([(Asset::Intel(location), 1)]))
    } else {
        // The unused sensor proves this direct commitment was made before a
        // scan. Once the Scout observes, coordination requires explicit share.
        rate.preserve(Role::Scout, basket([(Asset::Sensor, 1)]))
    }
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

pub fn agent_view(world: &World, agent: AgentId) -> AgentView<'_> {
    world.view([
        AccountId::Agent(agent),
        AccountId::Mission,
        AccountId::Success,
    ])
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut rates = world.rate_ids().copied().collect::<Vec<_>>();
    rates.sort();
    world.applicable(rates.into_iter().map(action))
}

/// Produces the acting Scout's information identity without exposing Nature or
/// the Medic's private account.
pub fn scout_information(world: &World) -> MissionInformation {
    InformationState::new(0, agent_view(world, AgentId::Scout).observation_key())
}

/// Lazily derives public mission decisions from the Scout's observation.
///
/// The emitted values are ordinary concrete exchanges. ISMCTS still assesses
/// them against each sampled economy and revalidates the selected exchange
/// against the live mission.
pub fn decision_source() -> impl ActionSource<MissionInformation, Action> {
    lazy_actions(
        |information: &MissionInformation, emit: &mut dyn FnMut(Action)| {
            if observed(information, AccountId::Mission, Asset::NeedsTreatment) {
                emit(action(RateId::Treat));
                return;
            }
            if observed(information, AccountId::Mission, Asset::AreaSafe) {
                for location in [Location::North, Location::South] {
                    if observed(
                        information,
                        AccountId::Agent(AgentId::Scout),
                        Asset::At(location),
                    ) {
                        emit(action(RateId::Rescue(location)));
                    }
                }
                return;
            }
            if observed(information, AccountId::Mission, Asset::Rescued) {
                emit(action(RateId::Finish));
                return;
            }
            if !observed(information, AccountId::Mission, Asset::Planning) {
                return;
            }

            let has_sensor = observed(information, AccountId::Agent(AgentId::Scout), Asset::Sensor);
            if has_sensor {
                emit(action(RateId::BeginScan));
            }
            for location in [Location::North, Location::South] {
                if observed(
                    information,
                    AccountId::Agent(AgentId::Scout),
                    Asset::Intel(location),
                ) {
                    emit(action(RateId::Share(location)));
                }
                if observed(
                    information,
                    AccountId::Agent(AgentId::Scout),
                    Asset::SharedIntel(location),
                ) {
                    emit(action(RateId::MoveTogether(location)));
                }
                if has_sensor {
                    emit(action(RateId::MoveDirect(location)));
                }
            }
        },
    )
}

/// Plans a public mission decision from caller-supplied encoded belief worlds.
///
/// Belief management stays outside the authoritative economy. Every supplied
/// determinization is still a complete economy, inconsistent worlds are
/// discarded by observation identity, and the selected live exchange is
/// revalidated against `actual`.
pub fn plan(
    actual: &World,
    beliefs: &[World],
    config: MctsConfig,
) -> IsmctsResult<RateId, Role, AccountId, Asset> {
    let root = scout_information(actual);
    let compatible = beliefs
        .iter()
        .filter(|world| scout_information(world) == root)
        .cloned()
        .collect::<Vec<_>>();
    information_set_search(
        actual,
        config,
        1,
        scout_information,
        move |_, random| {
            (!compatible.is_empty()).then(|| {
                let index = random.ticket(compatible.len() as u64) as usize;
                compatible[index].fork()
            })
        },
        decision_source(),
        nature_outcomes,
        |world| world.matches(&goal()).then_some(vec![1.0]),
        |_| vec![0.0],
        informed_rollout_action,
    )
}

/// Expands Nature's encoded integer-weighted prior into initial belief worlds.
///
/// This is fixture orchestration, not authoritative state: callers may retain,
/// filter, replace, or generate their own complete encoded determinizations.
pub fn initial_beliefs(model: &World) -> Vec<World> {
    scenarios(model)
        .into_iter()
        .flat_map(|weighted| {
            let mut world = model.fork();
            world
                .apply(weighted.exchange().clone())
                .expect("scenario support contains applicable exchanges");
            std::iter::repeat_n(
                world,
                usize::try_from(weighted.weight())
                    .expect("this concrete fixture uses platform-sized integer weights"),
            )
        })
        .collect()
}

/// Advances caller-owned beliefs through one public action and any immediately
/// required encoded Nature response, then conditions on the live observation.
pub fn update_beliefs(
    beliefs: &[World],
    public_action: &Action,
    live_observation: &MissionInformation,
) -> Vec<World> {
    beliefs
        .iter()
        .filter_map(|belief| {
            let mut next = belief.fork();
            next.apply(public_action.clone()).ok()?;
            if let Some(response) = required_nature_response(&next) {
                next.apply(response).ok()?;
            }
            (scout_information(&next) == *live_observation).then_some(next)
        })
        .collect()
}

/// Returns the unique immediately required Nature exchange in this fixture.
pub fn required_nature_response(world: &World) -> Option<Action> {
    candidates(world).into_iter().find(|exchange| {
        matches!(
            exchange.rate(),
            RateId::ResolveScan { .. } | RateId::Encounter { .. }
        )
    })
}

/// Instantiates one reproducible hidden scenario on an isolated branch.
pub fn instantiate(model: &World, sample_index: usize) -> Option<World> {
    let support = scenarios(model);
    let total = total_weight(&support).ok()?;
    let exchange = choose_by_ticket(&support, systematic_ticket(sample_index, total))
        .ok()?
        .clone();
    let mut world = model.fork();
    world.apply(exchange).ok()?;
    Some(world)
}

pub fn run_policy(model: &World, policy: Policy, sample_index: usize) -> MissionRollout {
    let scenarios = scenarios(model);
    let total = total_weight(&scenarios).expect("mission has encoded scenarios");
    let scenario = choose_by_ticket(&scenarios, systematic_ticket(sample_index, total))
        .expect("systematic ticket is in range")
        .clone();
    run_policy_with_scenario(model, policy, scenario)
}

fn run_policy_with_scenario(model: &World, policy: Policy, scenario: Action) -> MissionRollout {
    let goal = goal();
    let mut scenario = Some(scenario);
    let result = run_to_goal(
        model,
        &goal,
        RolloutConfig::new(HORIZON).with_retention(TraceRetention::Trace),
        |world, _| {
            let proposal = if !world
                .balance(&AccountId::Nature, &Asset::Unresolved)
                .is_zero()
            {
                scenario.take()
            } else {
                policy_action(world, policy)
            };
            match proposal {
                Some(exchange) => RolloutDecision::Propose(exchange),
                None => RolloutDecision::Stop(RolloutStop::NoProposal),
            }
        },
    );
    MissionRollout {
        trace: result
            .trace()
            .cloned()
            .expect("mission rollouts retain traces"),
        succeeded: result.world().matches(&goal),
        elapsed_time: result
            .world()
            .balance(&AccountId::Mission, &Asset::ElapsedTime)
            .get(),
        used_medical_kit: !result
            .world()
            .balance(&AccountId::Agent(AgentId::Medic), &Asset::UsedMedicalKit)
            .is_zero(),
    }
}

/// Exhaustively compares policies across every integer-weighted scenario.
pub fn evaluate_scenarios(model: &World) -> Option<PolicyComparison> {
    let support = scenarios(model);
    let samples = usize::try_from(total_weight(&support).ok()?).ok()?;
    let estimates = evaluate(
        [Policy::ShareAndCoordinate, Policy::NorthTogether],
        MonteCarloConfig::new(samples),
        |policy, sample| {
            Ok::<_, std::convert::Infallible>(run_policy(model, *policy, sample).succeeded())
        },
        BernoulliStatistics::new,
    )
    .ok()?;
    comparison(estimates, samples)
}

/// Estimates policies from reproducible random draws over the encoded prior.
pub fn monte_carlo(model: &World, samples: usize, seed: u64) -> Option<PolicyComparison> {
    let support = scenarios(model);
    let estimates = evaluate(
        [Policy::ShareAndCoordinate, Policy::NorthTogether],
        MonteCarloConfig::new(samples),
        |policy, sample_index| {
            let offset = u64::try_from(sample_index)
                .unwrap_or(u64::MAX)
                .wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let mut sampler = SeededSampler::new(seed.wrapping_add(offset));
            let scenario = sample(&support, &mut sampler).map_err(|_| ())?.clone();
            Ok::<_, ()>(run_policy_with_scenario(model, *policy, scenario).succeeded())
        },
        BernoulliStatistics::new,
    )
    .ok()?;
    comparison(estimates, samples)
}

fn comparison(
    estimates: Vec<
        axionomy_search::monte_carlo::PolicyEstimate<
            Policy,
            axionomy_search::monte_carlo::BernoulliSummary,
        >,
    >,
    samples: usize,
) -> Option<PolicyComparison> {
    let coordinated_successes = estimates
        .iter()
        .find(|estimate| estimate.policy() == &Policy::ShareAndCoordinate)?
        .summary()
        .successes();
    let direct_successes = estimates
        .iter()
        .find(|estimate| estimate.policy() == &Policy::NorthTogether)?
        .summary()
        .successes();
    Some(PolicyComparison {
        chosen: if coordinated_successes >= direct_successes {
            Policy::ShareAndCoordinate
        } else {
            Policy::NorthTogether
        },
        coordinated_successes,
        direct_successes,
        samples,
    })
}

fn policy_action(world: &World, policy: Policy) -> Option<Action> {
    if !world
        .balance(&AccountId::Mission, &Asset::AwaitingScan)
        .is_zero()
    {
        return candidates(world)
            .into_iter()
            .find(|exchange| matches!(exchange.rate(), RateId::ResolveScan { .. }));
    }
    if !world
        .balance(&AccountId::Mission, &Asset::Rescued)
        .is_zero()
    {
        return Some(action(RateId::Finish));
    }
    if !world
        .balance(&AccountId::Mission, &Asset::NeedsTreatment)
        .is_zero()
    {
        return Some(action(RateId::Treat));
    }
    if !world
        .balance(&AccountId::Mission, &Asset::AreaSafe)
        .is_zero()
    {
        let location = agent_location(world, AgentId::Scout)?;
        return Some(action(RateId::Rescue(location)));
    }
    if !world
        .balance(&AccountId::Mission, &Asset::AwaitingEncounter)
        .is_zero()
    {
        return candidates(world)
            .into_iter()
            .find(|exchange| matches!(exchange.rate(), RateId::Encounter { .. }));
    }

    match policy {
        Policy::NorthTogether => Some(action(RateId::MoveDirect(Location::North))),
        Policy::ShareAndCoordinate => {
            let scout = agent_view(world, AgentId::Scout);
            if scout
                .balance(&AccountId::Agent(AgentId::Scout), &Asset::Sensor)
                .is_some_and(|quantity| !quantity.is_zero())
            {
                return Some(action(RateId::BeginScan));
            }
            if let Some(location) = believed_location(&scout, AgentId::Scout, false) {
                return Some(action(RateId::Share(location)));
            }
            let medic = agent_view(world, AgentId::Medic);
            believed_location(&medic, AgentId::Medic, false)
                .map(|location| action(RateId::MoveTogether(location)))
        }
    }
}

fn observed(information: &MissionInformation, account: AccountId, asset: Asset) -> bool {
    information
        .key()
        .balances()
        .iter()
        .any(|(present_account, present_asset, quantity)| {
            present_account == &account && present_asset == &asset && !quantity.is_zero()
        })
}

fn nature_outcomes(world: &World) -> Vec<WeightedExchange<Action>> {
    candidates(world)
        .into_iter()
        .filter(|exchange| {
            matches!(
                exchange.rate(),
                RateId::ResolveScan { .. } | RateId::Encounter { .. }
            )
        })
        .map(|exchange| WeightedExchange::new(exchange, 1))
        .collect()
}

fn informed_rollout_action(
    information: &MissionInformation,
    actions: &[Action],
    _: &mut SeededSampler,
) -> Option<Action> {
    let preferred_location = [Location::North, Location::South]
        .into_iter()
        .find(|location| {
            observed(
                information,
                AccountId::Agent(AgentId::Scout),
                Asset::Intel(*location),
            ) || observed(
                information,
                AccountId::Agent(AgentId::Scout),
                Asset::SharedIntel(*location),
            )
        });
    for simple in [RateId::Finish, RateId::Treat] {
        if let Some(action) = actions.iter().find(|exchange| exchange.rate() == &simple) {
            return Some(action.clone());
        }
    }
    if let Some(action) = actions
        .iter()
        .find(|exchange| matches!(exchange.rate(), RateId::Rescue(_)))
    {
        return Some(action.clone());
    }
    if let Some(location) = preferred_location {
        for preferred in [RateId::Share(location), RateId::MoveTogether(location)] {
            if let Some(action) = actions
                .iter()
                .find(|exchange| exchange.rate() == &preferred)
            {
                return Some(action.clone());
            }
        }
    }
    for fallback in [
        RateId::BeginScan,
        RateId::MoveDirect(Location::North),
        RateId::MoveDirect(Location::South),
    ] {
        if let Some(action) = actions.iter().find(|exchange| exchange.rate() == &fallback) {
            return Some(action.clone());
        }
    }
    None
}

fn believed_location(view: &AgentView<'_>, agent: AgentId, shared: bool) -> Option<Location> {
    [Location::North, Location::South]
        .into_iter()
        .find(|location| {
            let asset = if shared {
                Asset::SharedIntel(*location)
            } else {
                Asset::Intel(*location)
            };
            view.balance(&AccountId::Agent(agent), &asset)
                .is_some_and(|quantity| !quantity.is_zero())
        })
}

fn scenarios(model: &World) -> Vec<WeightedExchange<Action>> {
    candidates(model)
        .into_iter()
        .filter_map(|exchange| match *exchange.rate() {
            RateId::Instantiate {
                truth,
                seed,
                hazard,
            } => Some(WeightedExchange::new(
                exchange,
                model
                    .balance(
                        &AccountId::Nature,
                        &Asset::ScenarioWeight(truth, seed, hazard),
                    )
                    .get(),
            )),
            _ => None,
        })
        .collect()
}

fn agent_location(world: &World, agent: AgentId) -> Option<Location> {
    [Location::Base, Location::North, Location::South]
        .into_iter()
        .find(|location| {
            !world
                .balance(&AccountId::Agent(agent), &Asset::At(*location))
                .is_zero()
        })
}

fn signal(truth: Location, seed: u8) -> Location {
    if seed == 0 {
        match truth {
            Location::North => Location::South,
            Location::South => Location::North,
            Location::Base => unreachable!("base is not a hidden mission site"),
        }
    } else {
        truth
    }
}

fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    match rate {
        RateId::Instantiate { .. } => exchange.bind(Role::Nature, AccountId::Nature),
        RateId::BeginScan => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Mission, AccountId::Mission),
        RateId::ResolveScan { .. } => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission),
        RateId::Share(_) | RateId::MoveTogether(_) | RateId::MoveDirect(_) | RateId::Treat => {
            exchange
                .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
                .bind(Role::Medic, AccountId::Agent(AgentId::Medic))
                .bind(Role::Mission, AccountId::Mission)
        }
        RateId::Encounter { .. } | RateId::Rescue(_) => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Medic, AccountId::Agent(AgentId::Medic))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission),
        RateId::Finish => exchange
            .bind(Role::Mission, AccountId::Mission)
            .bind(Role::Goal, AccountId::Success),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy_search::{action_source::collect_actions, rl::replay_transitions};

    #[test]
    fn agent_views_hide_nature_and_each_other() {
        let world = initial();
        let scout = agent_view(&world, AgentId::Scout);
        let medic = agent_view(&world, AgentId::Medic);

        assert!(scout.account(&AccountId::Nature).is_none());
        assert!(scout.account(&AccountId::Agent(AgentId::Medic)).is_none());
        assert!(medic.account(&AccountId::Nature).is_none());
        assert!(medic.account(&AccountId::Agent(AgentId::Scout)).is_none());
    }

    #[test]
    fn equal_scout_observations_have_equal_public_decisions() {
        let model = initial();
        let north = (0..16)
            .filter_map(|sample| instantiate(&model, sample))
            .find(|world| {
                !world
                    .balance(&AccountId::Nature, &Asset::Truth(Location::North))
                    .is_zero()
            })
            .expect("the prior includes a north world");
        let south = (0..16)
            .filter_map(|sample| instantiate(&model, sample))
            .find(|world| {
                !world
                    .balance(&AccountId::Nature, &Asset::Truth(Location::South))
                    .is_zero()
            })
            .expect("the prior includes a south world");

        assert_ne!(north.state_key(), south.state_key());
        assert_eq!(scout_information(&north), scout_information(&south));
        let information = scout_information(&north);
        let mut source = decision_source();
        let proposals = collect_actions(&mut source, &information);
        assert_eq!(
            proposals,
            vec![
                action(RateId::BeginScan),
                action(RateId::MoveDirect(Location::North)),
                action(RateId::MoveDirect(Location::South)),
            ]
        );
        assert!(proposals.iter().all(|proposal| {
            !matches!(
                proposal.rate(),
                RateId::Instantiate { .. } | RateId::ResolveScan { .. } | RateId::Encounter { .. }
            )
        }));

        let config = MctsConfig::new(1_024, HORIZON).with_seed(29);
        let beliefs = initial_beliefs(&model);
        let north_decision = plan(&north, &beliefs, config).expect("north can be planned");
        let south_decision = plan(&south, &beliefs, config).expect("south can be planned");

        assert_eq!(north_decision.action().rate(), &RateId::BeginScan);
        assert_eq!(north_decision.action(), south_decision.action());
        assert!(north.is_applicable(north_decision.action()));
        assert!(south.is_applicable(south_decision.action()));
    }

    #[test]
    fn caller_filters_beliefs_and_replans_after_the_scan() {
        let model = initial();
        let beliefs = initial_beliefs(&model);
        assert_eq!(beliefs.len(), 16);
        let mut actual = instantiate(&model, 3).expect("scenario exists");

        let first = plan(
            &actual,
            &beliefs,
            MctsConfig::new(1_024, HORIZON).with_seed(29),
        )
        .expect("initial information set can be planned");
        assert_eq!(first.action().rate(), &RateId::BeginScan);
        actual
            .apply(first.action().clone())
            .expect("public scan intent applies");
        let response = required_nature_response(&actual).expect("Nature resolves the scan");
        actual.apply(response).expect("encoded response applies");

        let observation = scout_information(&actual);
        let posterior = update_beliefs(&beliefs, first.action(), &observation);
        assert!(!posterior.is_empty());
        assert!(posterior.len() < beliefs.len());
        assert!(
            posterior
                .iter()
                .all(|belief| scout_information(belief) == observation)
        );

        let second = plan(
            &actual,
            &posterior,
            MctsConfig::new(1_024, HORIZON).with_seed(37),
        )
        .expect("posterior information set can be replanned");
        assert!(matches!(second.action().rate(), RateId::Share(_)));
        assert!(actual.is_applicable(second.action()));
    }

    #[test]
    fn coordinated_sample_replays_with_intelligence_transfer() {
        let model = initial();
        let rollout = run_policy(&model, Policy::ShareAndCoordinate, 3);

        assert!(
            rollout.succeeded(),
            "trace: {:?}",
            rollout
                .trace()
                .exchanges()
                .iter()
                .map(Action::rate)
                .collect::<Vec<_>>()
        );
        assert!(
            rollout
                .trace()
                .exchanges()
                .iter()
                .any(|exchange| exchange.rate() == &RateId::BeginScan)
        );
        assert!(
            rollout
                .trace()
                .exchanges()
                .iter()
                .any(|exchange| matches!(exchange.rate(), RateId::ResolveScan { .. }))
        );
        assert!(
            rollout
                .trace()
                .exchanges()
                .iter()
                .any(|exchange| matches!(exchange.rate(), RateId::Share(_)))
        );
        assert!(
            rollout
                .trace()
                .exchanges()
                .iter()
                .any(|exchange| matches!(exchange.rate(), RateId::Encounter { .. }))
        );
        let replayed = model
            .replayed(rollout.trace())
            .expect("mission trace must replay");
        assert!(replayed.matches(&goal()));
        assert!(candidates(&replayed).is_empty());
    }

    #[test]
    fn exact_scenario_evaluation_prefers_observe_share_and_coordinate() {
        let estimate = evaluate_scenarios(&initial()).expect("mission prior is encoded");

        assert_eq!(estimate.coordinated_successes(), 12);
        assert_eq!(estimate.direct_successes(), 8);
        assert_eq!(estimate.chosen(), Policy::ShareAndCoordinate);
    }

    #[test]
    fn seeded_monte_carlo_is_reproducible() {
        let model = initial();
        assert_eq!(monte_carlo(&model, 64, 23), monte_carlo(&model, 64, 23));
    }

    #[test]
    fn replayed_mission_produces_observation_and_outcome_transitions() {
        let model = initial();
        let rollout = run_policy(&model, Policy::ShareAndCoordinate, 3);
        let transitions = replay_transitions(
            &model,
            rollout.trace(),
            |world| {
                [
                    world.balance(&AccountId::Mission, &Asset::Planning).get(),
                    world
                        .balance(&AccountId::Mission, &Asset::AwaitingEncounter)
                        .get(),
                    world.balance(&AccountId::Success, &Asset::Solved).get(),
                ]
            },
            |world| {
                [
                    world.balance(&AccountId::Success, &Asset::Solved).get(),
                    world
                        .balance(&AccountId::Mission, &Asset::ElapsedTime)
                        .get(),
                ]
            },
            |world| world.matches(&goal()),
        )
        .expect("valid mission trace becomes learning data");

        assert_eq!(transitions.len(), rollout.trace().exchanges().len());
        assert!(transitions.last().expect("finish transition").terminal());
        assert_eq!(
            transitions.last().expect("finish transition").outcome()[0],
            1
        );
    }

    #[test]
    fn encoded_roles_reject_resolving_a_scan_into_the_medics_account() {
        let model = initial();
        let mut world = instantiate(&model, 0).expect("scenario exists");
        world
            .apply(action(RateId::BeginScan))
            .expect("scout starts scan");
        let resolution = candidates(&world)
            .into_iter()
            .find(|exchange| matches!(exchange.rate(), RateId::ResolveScan { .. }))
            .expect("nature can resolve the scan");
        let rebound = Exchange::new(*resolution.rate(), Quantity::new(1))
            .bind(Role::Scout, AccountId::Agent(AgentId::Medic))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission);

        assert!(!world.is_applicable(&rebound));
    }
}
