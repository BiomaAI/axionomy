#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Goal, Quantity, basket};

pub mod cold_chain;
#[cfg(feature = "packages")]
pub mod package;

pub const DEFAULT_WAREHOUSE_CLAIMS: u64 = 7_000;
pub const DEFAULT_FRIDGE_CLAIMS: u64 = 3_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Warehouse,
    Fridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Cohort {
    Ambient,
    Refrigerated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Exposure {
    Ambient,
    Cold,
    WarmedAfterOutage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Moment {
    Harvest,
    AmbientExpiry,
    WarmedExpiry,
    ColdExpiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Storage(Location),
    Cohort(Cohort),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    WorldIdentity,
    LocationIdentity(Location),
    CohortIdentity(Cohort),
    Claim(Cohort),
    Consumed(Cohort),
    Fresh(Cohort, Exposure, Moment),
    Rotten(Cohort),
    Ambient,
    Cold,
    Powered,
    Unpowered,
    CoolingEnergy,
    SpentCoolingEnergy,
    Now(Moment),
    Before(Moment),
    Reached(Moment),
    Active,
    Solved,
    Planning,
    PlanSolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Holder,
    Storage,
    Cohort,
    SourceStorage,
    DestinationStorage,
    SourceCohort,
    DestinationCohort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    MoveToFridge,
    Advance { from: Moment, to: Moment },
    LosePower,
    Eat { cohort: Cohort, exposure: Exposure },
    Spoil { cohort: Cohort, exposure: Exposure },
    Finish,
    SealStoragePlan,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `perishables` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "perishables";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial() -> World {
    initial_with_inventory(DEFAULT_WAREHOUSE_CLAIMS, DEFAULT_FRIDGE_CLAIMS)
}

pub fn initial_with_inventory(warehouse_claims: u64, fridge_claims: u64) -> World {
    crate::fixtures::variant(axionomy::path!("perishables/micro"), |instance| {
        crate::fixtures::set(
            instance,
            storage_account(Location::Warehouse),
            [(Asset::Claim(Cohort::Ambient), warehouse_claims)],
        );
        crate::fixtures::set(
            instance,
            storage_account(Location::Fridge),
            [
                (Asset::Claim(Cohort::Refrigerated), fridge_claims),
                (Asset::CoolingEnergy, warehouse_claims),
            ],
        );
    })
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::World, basket([(Asset::Solved, 1)]))
}

pub fn action(rate: RateId, units: u64) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(units));
    match rate {
        RateId::MoveToFridge => exchange
            .bind(Role::SourceStorage, storage_account(Location::Warehouse))
            .bind(Role::DestinationStorage, storage_account(Location::Fridge))
            .bind(Role::SourceCohort, cohort_account(Cohort::Ambient))
            .bind(
                Role::DestinationCohort,
                cohort_account(Cohort::Refrigerated),
            )
            .bind(Role::World, AccountId::World),
        RateId::Advance { .. } => exchange.bind(Role::World, AccountId::World),
        RateId::LosePower => exchange
            .bind(Role::Storage, storage_account(Location::Fridge))
            .bind(Role::Cohort, cohort_account(Cohort::Refrigerated))
            .bind(Role::World, AccountId::World),
        RateId::Eat { cohort, .. } => exchange
            .bind(Role::Holder, storage_account(stored_in(cohort)))
            .bind(Role::Cohort, cohort_account(cohort))
            .bind(Role::World, AccountId::World),
        RateId::Spoil { cohort, .. } => exchange
            .bind(Role::Storage, storage_account(stored_in(cohort)))
            .bind(Role::Cohort, cohort_account(cohort))
            .bind(Role::World, AccountId::World),
        RateId::Finish | RateId::SealStoragePlan => exchange
            .bind(Role::SourceStorage, storage_account(Location::Warehouse))
            .bind(Role::DestinationStorage, storage_account(Location::Fridge))
            .bind(Role::SourceCohort, cohort_account(Cohort::Ambient))
            .bind(
                Role::DestinationCohort,
                cohort_account(Cohort::Refrigerated),
            )
            .bind(Role::World, AccountId::World),
    }
}

pub fn move_to_fridge(units: u64) -> Action {
    action(RateId::MoveToFridge, units)
}

pub fn advance(from: Moment, to: Moment) -> Action {
    action(RateId::Advance { from, to }, 1)
}

pub fn lose_power() -> Action {
    action(RateId::LosePower, 1)
}

pub fn eat(cohort: Cohort, exposure: Exposure, units: u64) -> Action {
    action(RateId::Eat { cohort, exposure }, units)
}

pub fn spoil(cohort: Cohort, exposure: Exposure) -> Action {
    action(RateId::Spoil { cohort, exposure }, 1)
}

pub fn finish() -> Action {
    action(RateId::Finish, 1)
}

pub fn seal_storage_plan() -> Action {
    action(RateId::SealStoragePlan, 1)
}

pub fn storage_plan_goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::World, basket([(Asset::PlanSolved, 1)]))
}

fn storage_account(location: Location) -> AccountId {
    AccountId::Storage(location)
}

fn cohort_account(cohort: Cohort) -> AccountId {
    AccountId::Cohort(cohort)
}

/// Where `cohort` is stored.
const fn stored_in(cohort: Cohort) -> Location {
    match cohort {
        Cohort::Ambient => Location::Warehouse,
        Cohort::Refrigerated => Location::Fridge,
    }
}

#[cfg(test)]
fn due(exposure: Exposure) -> Moment {
    match exposure {
        Exposure::Ambient => Moment::AmbientExpiry,
        Exposure::Cold => Moment::ColdExpiry,
        Exposure::WarmedAfterOutage => Moment::WarmedExpiry,
    }
}

#[cfg(test)]
fn fresh_asset(cohort: Cohort, exposure: Exposure) -> Asset {
    Asset::Fresh(cohort, exposure, due(exposure))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{AssessmentStatus, Trace};
    use proptest::prelude::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Oracle {
        warehouse_claims: u64,
        fridge_claims: u64,
        ambient_rotten: bool,
        refrigerated_rotten: bool,
    }

    impl Oracle {
        fn run(warehouse_claims: u64, fridge_claims: u64, moved: u64) -> Self {
            Self {
                warehouse_claims: warehouse_claims - moved,
                fridge_claims: fridge_claims + moved,
                ambient_rotten: true,
                refrigerated_rotten: true,
            }
        }
    }

    /// The claims of `cohort`, wherever they are held.
    fn claims(world: &World, cohort: Cohort) -> u64 {
        world
            .accounts()
            .map(|(_, account)| account.balance(&Asset::Claim(cohort)).to_u64())
            .sum()
    }

    /// The outage, `moved` claims first moved to the fridge: each timed
    /// condition spoils its cohort once its moment comes, except the cold
    /// one, which the power loss has already replaced, and the outage ends.
    fn outage(moved: u64) -> Trace<Ontology> {
        (moved > 0)
            .then(|| move_to_fridge(moved))
            .into_iter()
            .chain([
                advance(Moment::Harvest, Moment::AmbientExpiry),
                spoil(Cohort::Ambient, Exposure::Ambient),
                lose_power(),
                advance(Moment::AmbientExpiry, Moment::WarmedExpiry),
                spoil(Cohort::Refrigerated, Exposure::WarmedAfterOutage),
                advance(Moment::WarmedExpiry, Moment::ColdExpiry),
                finish(),
            ])
            .collect()
    }

    #[test]
    fn ten_thousand_units_share_two_balance_entries_and_two_condition_facts() {
        let small = initial_with_inventory(7, 3);
        let large = initial();
        let claim_entries = large
            .accounts()
            .flat_map(|(_, account)| account.balances().iter())
            .filter(|(asset, _)| matches!(asset, Asset::Claim(_)))
            .count();

        assert_eq!(
            large.state_key().balances().len(),
            small.state_key().balances().len()
        );
        assert_eq!(
            claims(&large, Cohort::Ambient) + claims(&large, Cohort::Refrigerated),
            10_000
        );
        assert_eq!(claim_entries, 2);
        assert_eq!(
            large.balance(
                &cohort_account(Cohort::Ambient),
                &fresh_asset(Cohort::Ambient, Exposure::Ambient),
            ),
            Quantity::new(1)
        );
        assert_eq!(
            large.balance(
                &cohort_account(Cohort::Refrigerated),
                &fresh_asset(Cohort::Refrigerated, Exposure::Cold),
            ),
            Quantity::new(1)
        );
    }

    #[test]
    fn a_sealed_storage_plan_keeps_what_it_moved_cold_for_its_energy() {
        let plan: Trace<Ontology> = [
            move_to_fridge(1_000),
            advance(Moment::Harvest, Moment::AmbientExpiry),
            spoil(Cohort::Ambient, Exposure::Ambient),
            seal_storage_plan(),
        ]
        .into_iter()
        .collect();
        let sealed = initial().replayed(&plan).expect("the storage plan seals");

        assert!(sealed.matches(&storage_plan_goal()));
        assert_eq!(
            sealed.balance(
                &storage_account(Location::Fridge),
                &Asset::Claim(Cohort::Refrigerated),
            ),
            Quantity::new(4_000)
        );
        assert_eq!(
            sealed.balance(
                &storage_account(Location::Fridge),
                &Asset::SpentCoolingEnergy,
            ),
            Quantity::new(1_000)
        );
    }

    #[test]
    fn fungible_transfer_scales_one_exchange_without_scaling_shared_facts() {
        let mut world = initial();
        let receipt = world
            .apply(move_to_fridge(1_000))
            .expect("the batched transfer applies");

        assert_eq!(claims(&world, Cohort::Ambient), 6_000);
        assert_eq!(claims(&world, Cohort::Refrigerated), 4_000);
        assert_eq!(receipt.exchange().units(), Quantity::new(1_000));
        assert_eq!(
            world.balance(
                &cohort_account(Cohort::Ambient),
                &fresh_asset(Cohort::Ambient, Exposure::Ambient),
            ),
            Quantity::new(1)
        );
    }

    #[test]
    fn unique_condition_fact_rejects_multi_unit_and_repeated_spoilage() {
        let mut world = initial();
        world
            .apply(advance(Moment::Harvest, Moment::AmbientExpiry))
            .unwrap();

        let multi = action(
            RateId::Spoil {
                cohort: Cohort::Ambient,
                exposure: Exposure::Ambient,
            },
            2,
        );
        assert_eq!(world.assess(&multi).status(), AssessmentStatus::Infeasible);
        world
            .apply(spoil(Cohort::Ambient, Exposure::Ambient))
            .unwrap();
        assert!(!world.is_applicable(&spoil(Cohort::Ambient, Exposure::Ambient)));
    }

    #[test]
    fn deadlines_block_early_decay_and_late_use_even_before_materialization() {
        let mut world = initial();
        let early = world.assess(&spoil(Cohort::Ambient, Exposure::Ambient));
        assert_eq!(early.status(), AssessmentStatus::Infeasible);
        assert_eq!(
            early
                .shortfall(&AccountId::World)
                .expect("the clock guard is missing")
                .quantity(&Asset::Reached(Moment::AmbientExpiry)),
            Quantity::new(1)
        );

        world
            .apply(advance(Moment::Harvest, Moment::AmbientExpiry))
            .unwrap();
        assert!(!world.is_applicable(&move_to_fridge(1)));
        assert!(!world.is_applicable(&eat(Cohort::Ambient, Exposure::Ambient, 1)));
        assert!(world.is_applicable(&spoil(Cohort::Ambient, Exposure::Ambient)));
    }

    #[test]
    fn power_loss_reclassifies_shared_fate_and_invalidates_old_event() {
        let mut world = initial();
        let stale = spoil(Cohort::Refrigerated, Exposure::Cold);
        world
            .apply(advance(Moment::Harvest, Moment::AmbientExpiry))
            .unwrap();
        let receipt = world.apply(lose_power()).unwrap();

        assert!(!world.is_applicable(&stale));
        assert_eq!(
            world.balance(
                &cohort_account(Cohort::Refrigerated),
                &fresh_asset(Cohort::Refrigerated, Exposure::WarmedAfterOutage,),
            ),
            Quantity::new(1)
        );
        assert!(receipt.deltas().iter().all(|delta| {
            delta
                .consumed()
                .iter()
                .chain(delta.produced().iter())
                .all(|(asset, _)| !matches!(asset, Asset::Claim(_)))
        }));
    }

    #[test]
    fn fresh_claim_use_is_fungible_and_condition_gated() {
        let mut fresh = initial();
        fresh
            .apply(eat(Cohort::Ambient, Exposure::Ambient, 500))
            .expect("five hundred equivalent claims are consumed together");
        assert_eq!(
            fresh.balance(
                &storage_account(Location::Warehouse),
                &Asset::Consumed(Cohort::Ambient),
            ),
            Quantity::new(500)
        );

        let mut expired = initial();
        expired
            .apply(advance(Moment::Harvest, Moment::AmbientExpiry))
            .unwrap();
        expired
            .apply(spoil(Cohort::Ambient, Exposure::Ambient))
            .unwrap();
        assert!(!expired.is_applicable(&eat(Cohort::Ambient, Exposure::Ambient, 1)));
    }

    #[test]
    fn adversarial_role_rebinding_cannot_change_location_cohort_or_world() {
        let mut world = initial();
        world
            .apply(advance(Moment::Harvest, Moment::AmbientExpiry))
            .unwrap();

        let wrong_storage = spoil(Cohort::Ambient, Exposure::Ambient)
            .bind(Role::Storage, storage_account(Location::Fridge));
        let wrong_cohort = lose_power().bind(Role::Cohort, cohort_account(Cohort::Ambient));
        let wrong_world = spoil(Cohort::Ambient, Exposure::Ambient)
            .bind(Role::World, storage_account(Location::Fridge));

        assert!(!world.is_applicable(&wrong_storage));
        assert!(!world.is_applicable(&wrong_cohort));
        assert!(!world.is_applicable(&wrong_world));
    }

    #[test]
    fn the_outage_replays_and_the_replaced_cold_condition_never_spoils() {
        let source = initial();
        let trace = outage(1_000);
        let (last, before_finish) = trace.exchanges().split_last().unwrap();
        let mut world = source
            .replayed(&before_finish.iter().cloned().collect())
            .unwrap();

        assert!(!world.is_applicable(&spoil(Cohort::Refrigerated, Exposure::Cold)));
        world.apply(last.clone()).unwrap();
        assert!(world.matches(&goal()));
        assert_eq!(claims(&world, Cohort::Ambient), 6_000);
        assert_eq!(claims(&world, Cohort::Refrigerated), 4_000);
        assert!(
            world
                .rate_ids()
                .flat_map(|key| world.instances_of(key))
                .all(|rate| !world.is_applicable(&action(rate, 1)))
        );
        assert_eq!(
            source.replayed(&trace).unwrap().state_key(),
            world.state_key()
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn cohort_model_agrees_with_independent_inventory_oracle(
            warehouse_claims in 0_u64..20_000,
            fridge_claims in 0_u64..20_000,
            transfer_seed in any::<u64>(),
        ) {
            let moved = if warehouse_claims == 0 {
                0
            } else {
                transfer_seed % (warehouse_claims + 1)
            };
            let oracle = Oracle::run(warehouse_claims, fridge_claims, moved);
            let source = initial_with_inventory(warehouse_claims, fridge_claims);
            let world = source.replayed(&outage(moved)).unwrap();

            prop_assert_eq!(claims(&world, Cohort::Ambient), oracle.warehouse_claims);
            prop_assert_eq!(claims(&world, Cohort::Refrigerated), oracle.fridge_claims);
            prop_assert_eq!(
                world.balance(
                    &cohort_account(Cohort::Ambient),
                    &Asset::Rotten(Cohort::Ambient),
                ) == Quantity::new(1),
                oracle.ambient_rotten
            );
            prop_assert_eq!(
                world.balance(
                    &cohort_account(Cohort::Refrigerated),
                    &Asset::Rotten(Cohort::Refrigerated),
                ) == Quantity::new(1),
                oracle.refrigerated_rotten
            );
            prop_assert!(world.matches(&goal()));
        }
    }
}
