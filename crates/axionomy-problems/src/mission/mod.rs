#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Goal, Quantity, basket};
pub mod expedition;
#[cfg(feature = "packages")]
pub mod package;

/// The most exchanges a Micro run takes, Nature's draw among them.
pub const HORIZON: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AgentId {
    Scout,
    Medic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Base,
    North,
    South,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Hazard {
    Safe,
    Injury,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Agent(AgentId),
    Nature,
    Mission,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Scout,
    Medic,
    Nature,
    Mission,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Instantiate {
        truth: Location,
        seed: u8,
        hazard: Hazard,
    },
    BeginScan,
    /// Nature answers a scan: the sensor at `seed` reports `report` of a
    /// victim at `truth`, and moves on to the seed after.
    ResolveScan {
        truth: Location,
        seed: u8,
        report: Location,
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

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `mission` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "mission";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial() -> World {
    crate::fixtures::initial(axionomy::path!("mission/micro"))
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
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
        .unwrap_or_else(|_| panic!("the mission has no rate {rate:?}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::Trace;

    /// What the Scout's view holds: its own account and the team's.
    const SCOUT_SEES: [AccountId; 3] = [
        AccountId::Agent(AgentId::Scout),
        AccountId::Mission,
        AccountId::Success,
    ];

    #[test]
    fn the_scout_holds_the_same_before_scanning_whatever_the_truth() {
        let instantiated = |truth| {
            let mut world = initial();
            world
                .apply(action(
                    &world,
                    RateId::Instantiate {
                        truth,
                        seed: 0,
                        hazard: Hazard::Injury,
                    },
                ))
                .expect("the prior encodes the scenario");
            world
        };
        let north = instantiated(Location::North);
        let south = instantiated(Location::South);

        assert_ne!(north.state_key(), south.state_key());
        assert_eq!(
            north.view(SCOUT_SEES).observation_key(),
            south.view(SCOUT_SEES).observation_key()
        );
        let scan = action(&north, RateId::BeginScan);
        assert!(north.is_applicable(&scan));
        assert!(south.is_applicable(&scan));
    }

    #[test]
    fn coordinated_sample_replays_with_intelligence_transfer() {
        let model = initial();
        let rates = [
            RateId::Instantiate {
                truth: Location::North,
                seed: 1,
                hazard: Hazard::Safe,
            },
            RateId::BeginScan,
            RateId::ResolveScan {
                truth: Location::North,
                seed: 1,
                report: Location::North,
            },
            RateId::Share(Location::North),
            RateId::MoveTogether(Location::North),
            RateId::Encounter {
                location: Location::North,
                hazard: Hazard::Safe,
            },
            RateId::Rescue(Location::North),
            RateId::Finish,
        ];
        let trace: Trace<Ontology> = rates.into_iter().map(|rate| action(&model, rate)).collect();

        // The Medic follows the Scout's intelligence only once it is shared.
        let scanned = model
            .replayed(&trace.exchanges()[..3].iter().cloned().collect())
            .expect("the scan replays");
        assert!(!scanned.is_applicable(&action(&scanned, RateId::MoveTogether(Location::North))));

        let replayed = model.replayed(&trace).expect("mission trace must replay");
        assert!(replayed.matches(&goal()));
        assert!(
            replayed
                .rate_ids()
                .flat_map(|key| replayed.instances_of(key))
                .all(|rate| !replayed.is_applicable(&action(&replayed, rate)))
        );
    }

    #[test]
    fn encoded_roles_reject_resolving_a_scan_into_the_medics_account() {
        let mut world = initial();
        world
            .apply(action(
                &world,
                RateId::Instantiate {
                    truth: Location::North,
                    seed: 0,
                    hazard: Hazard::Injury,
                },
            ))
            .expect("the prior encodes the scenario");
        world
            .apply(action(&world, RateId::BeginScan))
            .expect("scout starts scan");
        let resolution = RateId::ResolveScan {
            truth: Location::North,
            seed: 0,
            report: Location::South,
        };
        assert!(world.is_applicable(&action(&world, resolution)));
        let rebound = Exchange::new(resolution, Quantity::new(1))
            .bind(Role::Scout, AccountId::Agent(AgentId::Medic))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission);

        assert!(!world.is_applicable(&rebound));
    }
}
