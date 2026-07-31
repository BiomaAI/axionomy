//! Partially observed, stochastic two-agent reconnaissance mission.

use axionomy::{
    Account, EconomicView, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    Rate, Trace, basket,
};
use axionomy_search::{
    monte_carlo::{BernoulliStatistics, MonteCarloConfig, evaluate},
    rollout::{RolloutConfig, RolloutDecision, RolloutStop, TraceRetention, run_to_goal},
    sampling::{WeightedExchange, choose_by_ticket, systematic_ticket, total_weight},
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
    Scan {
        truth: Location,
        seed: u8,
        report: Location,
        next_seed: u8,
    },
    Share(Location),
    MoveTogether(Location),
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
pub struct MonteCarloEstimate {
    chosen: Policy,
    coordinated_successes: usize,
    direct_successes: usize,
    samples: usize,
}

impl MonteCarloEstimate {
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
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::Sensor, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::Medic),
            Account::from(basket([
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::MedicalKit, 1),
            ])),
        )
        .account(AccountId::Nature, Account::from(nature))
        .account(
            AccountId::Mission,
            Account::from(basket([(Asset::TimeRemaining, 10), (Asset::Planning, 1)])),
        )
        .account(AccountId::Success, Account::default());

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
                RateId::Scan {
                    truth,
                    seed,
                    report,
                    next_seed: (seed + 1) % 4,
                },
                Rate::new()
                    .preserve(Role::Scout, basket([(Asset::At(Location::Base), 1)]))
                    .consume(Role::Scout, basket([(Asset::Sensor, 1)]))
                    .produce(
                        Role::Scout,
                        basket([(Asset::UsedSensor, 1), (Asset::Intel(report), 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::Truth(truth), 1)]))
                    .consume(Role::Nature, basket([(Asset::Seed(seed), 1)]))
                    .produce(Role::Nature, basket([(Asset::Seed((seed + 1) % 4), 1)]))
                    .preserve(Role::Mission, basket([(Asset::Planning, 1)]))
                    .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
                    .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)]))
                    .distinct(Role::Scout, Role::Nature)
                    .distinct(Role::Scout, Role::Mission)
                    .distinct(Role::Nature, Role::Mission),
            );
        }
    }

    for location in [Location::North, Location::South] {
        builder = builder
            .rate(
                RateId::Share(location),
                Rate::new()
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
                Rate::new()
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
                    .distinct(Role::Medic, Role::Mission),
            );

        for hazard in [Hazard::Safe, Hazard::Injury] {
            let rate = Rate::new()
                .preserve(Role::Scout, basket([(Asset::At(location), 1)]))
                .preserve(Role::Medic, basket([(Asset::At(location), 1)]))
                .preserve(Role::Nature, basket([(Asset::Truth(location), 1)]))
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
                .preserve(Role::Scout, basket([(Asset::At(location), 1)]))
                .preserve(Role::Medic, basket([(Asset::At(location), 1)]))
                .preserve(Role::Nature, basket([(Asset::Truth(location), 1)]))
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

pub fn run_policy(model: &World, policy: Policy, sample_index: usize) -> MissionRollout {
    let goal = goal();
    let scenarios = scenarios(model);
    let total = total_weight(&scenarios).expect("mission has encoded scenarios");
    let scenario = choose_by_ticket(&scenarios, systematic_ticket(sample_index, total))
        .expect("systematic ticket is in range")
        .clone();
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

pub fn monte_carlo(model: &World, samples: usize) -> Option<MonteCarloEstimate> {
    let estimates = evaluate(
        [Policy::ShareAndCoordinate, Policy::NorthTogether],
        MonteCarloConfig::new(samples),
        |policy, sample| {
            Ok::<_, std::convert::Infallible>(run_policy(model, *policy, sample).succeeded())
        },
        BernoulliStatistics::new,
    )
    .ok()?;
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
    Some(MonteCarloEstimate {
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
        Policy::NorthTogether => Some(action(RateId::MoveTogether(Location::North))),
        Policy::ShareAndCoordinate => {
            let scout = agent_view(world, AgentId::Scout);
            if scout
                .balance(&AccountId::Agent(AgentId::Scout), &Asset::Sensor)
                .is_some_and(|quantity| !quantity.is_zero())
            {
                return candidates(world)
                    .into_iter()
                    .find(|exchange| matches!(exchange.rate(), RateId::Scan { .. }));
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
        RateId::Scan { .. } => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission),
        RateId::Share(_) | RateId::MoveTogether(_) | RateId::Treat => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Medic, AccountId::Agent(AgentId::Medic))
            .bind(Role::Mission, AccountId::Mission),
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
    }

    #[test]
    fn monte_carlo_prefers_observe_share_and_coordinate() {
        let estimate = monte_carlo(&initial(), 16).expect("mission prior is encoded");

        assert_eq!(estimate.coordinated_successes(), 12);
        assert_eq!(estimate.direct_successes(), 8);
        assert_eq!(estimate.chosen(), Policy::ShareAndCoordinate);
    }
}
