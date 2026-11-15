#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, basket,
};

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

const COHORTS: [Cohort; 2] = [Cohort::Ambient, Cohort::Refrigerated];
const MOMENTS: [Moment; 4] = [
    Moment::Harvest,
    Moment::AmbientExpiry,
    Moment::WarmedExpiry,
    Moment::ColdExpiry,
];
const TIMED_CONDITIONS: [(Cohort, Exposure, Moment); 3] = [
    (Cohort::Ambient, Exposure::Ambient, Moment::AmbientExpiry),
    (Cohort::Refrigerated, Exposure::Cold, Moment::ColdExpiry),
    (
        Cohort::Refrigerated,
        Exposure::WarmedAfterOutage,
        Moment::WarmedExpiry,
    ),
];

pub fn initial() -> World {
    initial_with_inventory(DEFAULT_WAREHOUSE_CLAIMS, DEFAULT_FRIDGE_CLAIMS)
}

pub fn initial_with_inventory(warehouse_claims: u64, fridge_claims: u64) -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::World,
            Account::from(basket([
                (Asset::WorldIdentity, 1),
                (Asset::Now(Moment::Harvest), 1),
                (Asset::Reached(Moment::Harvest), 1),
                (Asset::Before(Moment::AmbientExpiry), 1),
                (Asset::Before(Moment::WarmedExpiry), 1),
                (Asset::Before(Moment::ColdExpiry), 1),
                (Asset::Active, 1),
                (Asset::Planning, 1),
            ])),
        )
        .account(
            storage_account(Location::Warehouse),
            Account::from(basket([
                (Asset::LocationIdentity(Location::Warehouse), 1),
                (Asset::Ambient, 1),
                (Asset::Claim(Cohort::Ambient), warehouse_claims),
            ])),
        )
        .account(
            storage_account(Location::Fridge),
            Account::from(basket([
                (Asset::LocationIdentity(Location::Fridge), 1),
                (Asset::Cold, 1),
                (Asset::Powered, 1),
                (Asset::CoolingEnergy, warehouse_claims),
                (Asset::Claim(Cohort::Refrigerated), fridge_claims),
            ])),
        )
        .account(
            cohort_account(Cohort::Ambient),
            Account::from(basket([
                (Asset::CohortIdentity(Cohort::Ambient), 1),
                (fresh_asset(Cohort::Ambient, Exposure::Ambient), 1),
            ])),
        )
        .account(
            cohort_account(Cohort::Refrigerated),
            Account::from(basket([
                (Asset::CohortIdentity(Cohort::Refrigerated), 1),
                (fresh_asset(Cohort::Refrigerated, Exposure::Cold), 1),
            ])),
        )
        .rate(RateId::MoveToFridge, move_to_fridge_rate())
        .rate(
            RateId::Advance {
                from: Moment::Harvest,
                to: Moment::AmbientExpiry,
            },
            advance_rate(Moment::Harvest, Moment::AmbientExpiry),
        )
        .rate(
            RateId::Advance {
                from: Moment::AmbientExpiry,
                to: Moment::WarmedExpiry,
            },
            advance_rate(Moment::AmbientExpiry, Moment::WarmedExpiry),
        )
        .rate(
            RateId::Advance {
                from: Moment::WarmedExpiry,
                to: Moment::ColdExpiry,
            },
            advance_rate(Moment::WarmedExpiry, Moment::ColdExpiry),
        )
        .rate(RateId::LosePower, lose_power_rate());

    for (cohort, exposure, _) in TIMED_CONDITIONS {
        builder = builder
            .rate(RateId::Eat { cohort, exposure }, eat_rate(cohort, exposure))
            .rate(
                RateId::Spoil { cohort, exposure },
                spoil_rate(cohort, exposure),
            );
    }

    builder
        .rate(RateId::Finish, finish_rate())
        .rate(RateId::SealStoragePlan, seal_storage_plan_rate())
        .invariant(claim_invariant())
        .invariant(cohort_condition_invariant(Cohort::Ambient))
        .invariant(cohort_condition_invariant(Cohort::Refrigerated))
        .invariant(
            LinearInvariant::new(path!("storage-environment"))
                .weight(Asset::Ambient, 1)
                .weight(Asset::Cold, 1),
        )
        .invariant(
            LinearInvariant::new(path!("fridge-power"))
                .weight(Asset::Powered, 1)
                .weight(Asset::Unpowered, 1),
        )
        .invariant(
            LinearInvariant::new(path!("cooling-energy"))
                .weight(Asset::CoolingEnergy, 1)
                .weight(Asset::SpentCoolingEnergy, 1),
        )
        .invariant(clock_invariant())
        .invariant(deadline_invariant(Moment::AmbientExpiry))
        .invariant(deadline_invariant(Moment::WarmedExpiry))
        .invariant(deadline_invariant(Moment::ColdExpiry))
        .invariant(
            LinearInvariant::new(path!("scenario-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .invariant(
            LinearInvariant::new(path!("storage-planning-lifecycle"))
                .weight(Asset::Planning, 1)
                .weight(Asset::PlanSolved, 1),
        )
        .build()
        .expect("perishables model is valid")
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
            .bind(Role::Holder, storage_for(cohort))
            .bind(Role::Cohort, cohort_account(cohort))
            .bind(Role::World, AccountId::World),
        RateId::Spoil { cohort, .. } => exchange
            .bind(Role::Storage, storage_for(cohort))
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

fn storage_for(cohort: Cohort) -> AccountId {
    match cohort {
        Cohort::Ambient => storage_account(Location::Warehouse),
        Cohort::Refrigerated => storage_account(Location::Fridge),
    }
}

fn due(exposure: Exposure) -> Moment {
    match exposure {
        Exposure::Ambient => Moment::AmbientExpiry,
        Exposure::Cold => Moment::ColdExpiry,
        Exposure::WarmedAfterOutage => Moment::WarmedExpiry,
    }
}

fn fresh_asset(cohort: Cohort, exposure: Exposure) -> Asset {
    Asset::Fresh(cohort, exposure, due(exposure))
}

fn active_world(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.preserve(
        Role::World,
        basket([(Asset::WorldIdentity, 1), (Asset::Active, 1)]),
    )
}

fn all_distinct(mut rate: Rate<Ontology>, roles: &[Role]) -> Rate<Ontology> {
    for (offset, left) in roles.iter().enumerate() {
        for right in &roles[offset + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    rate
}

fn move_to_fridge_rate() -> Rate<Ontology> {
    all_distinct(
        active_world(
            Rate::new()
                .preserve(
                    Role::SourceStorage,
                    basket([
                        (Asset::LocationIdentity(Location::Warehouse), 1),
                        (Asset::Ambient, 1),
                    ]),
                )
                .consume(
                    Role::SourceStorage,
                    basket([(Asset::Claim(Cohort::Ambient), 1)]),
                )
                .preserve(
                    Role::DestinationStorage,
                    basket([
                        (Asset::LocationIdentity(Location::Fridge), 1),
                        (Asset::Cold, 1),
                        (Asset::Powered, 1),
                    ]),
                )
                .consume(
                    Role::DestinationStorage,
                    basket([(Asset::CoolingEnergy, 1)]),
                )
                .produce(
                    Role::DestinationStorage,
                    basket([
                        (Asset::Claim(Cohort::Refrigerated), 1),
                        (Asset::SpentCoolingEnergy, 1),
                    ]),
                )
                .preserve(
                    Role::SourceCohort,
                    basket([
                        (Asset::CohortIdentity(Cohort::Ambient), 1),
                        (fresh_asset(Cohort::Ambient, Exposure::Ambient), 1),
                    ]),
                )
                .preserve(
                    Role::DestinationCohort,
                    basket([
                        (Asset::CohortIdentity(Cohort::Refrigerated), 1),
                        (fresh_asset(Cohort::Refrigerated, Exposure::Cold), 1),
                    ]),
                )
                .preserve(
                    Role::World,
                    basket([(Asset::Before(Moment::AmbientExpiry), 1)]),
                ),
        ),
        &[
            Role::SourceStorage,
            Role::DestinationStorage,
            Role::SourceCohort,
            Role::DestinationCohort,
            Role::World,
        ],
    )
}

fn advance_rate(from: Moment, to: Moment) -> Rate<Ontology> {
    active_world(
        Rate::new()
            .consume(
                Role::World,
                basket([(Asset::Now(from), 1), (Asset::Before(to), 1)]),
            )
            .produce(
                Role::World,
                basket([(Asset::Now(to), 1), (Asset::Reached(to), 1)]),
            ),
    )
}

fn lose_power_rate() -> Rate<Ontology> {
    all_distinct(
        active_world(
            Rate::new()
                .preserve(
                    Role::World,
                    basket([
                        (Asset::Reached(Moment::AmbientExpiry), 1),
                        (Asset::Before(Moment::WarmedExpiry), 1),
                    ]),
                )
                .preserve(
                    Role::Storage,
                    basket([(Asset::LocationIdentity(Location::Fridge), 1)]),
                )
                .consume(
                    Role::Storage,
                    basket([(Asset::Cold, 1), (Asset::Powered, 1)]),
                )
                .produce(
                    Role::Storage,
                    basket([(Asset::Ambient, 1), (Asset::Unpowered, 1)]),
                )
                .preserve(
                    Role::Cohort,
                    basket([(Asset::CohortIdentity(Cohort::Refrigerated), 1)]),
                )
                .consume(
                    Role::Cohort,
                    basket([(fresh_asset(Cohort::Refrigerated, Exposure::Cold), 1)]),
                )
                .produce(
                    Role::Cohort,
                    basket([(
                        fresh_asset(Cohort::Refrigerated, Exposure::WarmedAfterOutage),
                        1,
                    )]),
                ),
        ),
        &[Role::Storage, Role::Cohort, Role::World],
    )
}

fn eat_rate(cohort: Cohort, exposure: Exposure) -> Rate<Ontology> {
    all_distinct(
        active_world(
            Rate::new()
                .preserve(Role::Holder, storage_requirements(cohort, exposure))
                .consume(Role::Holder, basket([(Asset::Claim(cohort), 1)]))
                .produce(Role::Holder, basket([(Asset::Consumed(cohort), 1)]))
                .preserve(
                    Role::Cohort,
                    basket([
                        (Asset::CohortIdentity(cohort), 1),
                        (fresh_asset(cohort, exposure), 1),
                    ]),
                )
                .preserve(Role::World, basket([(Asset::Before(due(exposure)), 1)])),
        ),
        &[Role::Holder, Role::Cohort, Role::World],
    )
}

fn spoil_rate(cohort: Cohort, exposure: Exposure) -> Rate<Ontology> {
    all_distinct(
        active_world(
            Rate::new()
                .preserve(Role::Storage, storage_requirements(cohort, exposure))
                .preserve(Role::Cohort, basket([(Asset::CohortIdentity(cohort), 1)]))
                .consume(Role::Cohort, basket([(fresh_asset(cohort, exposure), 1)]))
                .produce(Role::Cohort, basket([(Asset::Rotten(cohort), 1)]))
                .preserve(Role::World, basket([(Asset::Reached(due(exposure)), 1)])),
        ),
        &[Role::Storage, Role::Cohort, Role::World],
    )
}

fn finish_rate() -> Rate<Ontology> {
    all_distinct(
        Rate::new()
            .preserve(
                Role::SourceStorage,
                basket([(Asset::LocationIdentity(Location::Warehouse), 1)]),
            )
            .preserve(
                Role::DestinationStorage,
                basket([(Asset::LocationIdentity(Location::Fridge), 1)]),
            )
            .preserve(
                Role::SourceCohort,
                basket([
                    (Asset::CohortIdentity(Cohort::Ambient), 1),
                    (Asset::Rotten(Cohort::Ambient), 1),
                ]),
            )
            .preserve(
                Role::DestinationCohort,
                basket([
                    (Asset::CohortIdentity(Cohort::Refrigerated), 1),
                    (Asset::Rotten(Cohort::Refrigerated), 1),
                ]),
            )
            .preserve(
                Role::World,
                basket([
                    (Asset::WorldIdentity, 1),
                    (Asset::Now(Moment::ColdExpiry), 1),
                ]),
            )
            .consume(Role::World, basket([(Asset::Active, 1)]))
            .produce(Role::World, basket([(Asset::Solved, 1)])),
        &[
            Role::SourceStorage,
            Role::DestinationStorage,
            Role::SourceCohort,
            Role::DestinationCohort,
            Role::World,
        ],
    )
}

fn seal_storage_plan_rate() -> Rate<Ontology> {
    all_distinct(
        Rate::new()
            .preserve(
                Role::SourceStorage,
                basket([(Asset::LocationIdentity(Location::Warehouse), 1)]),
            )
            .preserve(
                Role::DestinationStorage,
                basket([
                    (Asset::LocationIdentity(Location::Fridge), 1),
                    (Asset::Cold, 1),
                    (Asset::Powered, 1),
                ]),
            )
            .preserve(
                Role::SourceCohort,
                basket([
                    (Asset::CohortIdentity(Cohort::Ambient), 1),
                    (Asset::Rotten(Cohort::Ambient), 1),
                ]),
            )
            .preserve(
                Role::DestinationCohort,
                basket([
                    (Asset::CohortIdentity(Cohort::Refrigerated), 1),
                    (fresh_asset(Cohort::Refrigerated, Exposure::Cold), 1),
                ]),
            )
            .preserve(
                Role::World,
                basket([
                    (Asset::WorldIdentity, 1),
                    (Asset::Now(Moment::AmbientExpiry), 1),
                    (Asset::Active, 1),
                ]),
            )
            .consume(Role::World, basket([(Asset::Planning, 1)]))
            .produce(Role::World, basket([(Asset::PlanSolved, 1)])),
        &[
            Role::SourceStorage,
            Role::DestinationStorage,
            Role::SourceCohort,
            Role::DestinationCohort,
            Role::World,
        ],
    )
}

fn storage_requirements(cohort: Cohort, exposure: Exposure) -> axionomy::Basket<Asset> {
    match (cohort, exposure) {
        (Cohort::Ambient, Exposure::Ambient) => basket([
            (Asset::LocationIdentity(Location::Warehouse), 1),
            (Asset::Ambient, 1),
        ]),
        (Cohort::Refrigerated, Exposure::Cold) => basket([
            (Asset::LocationIdentity(Location::Fridge), 1),
            (Asset::Cold, 1),
            (Asset::Powered, 1),
        ]),
        (Cohort::Refrigerated, Exposure::WarmedAfterOutage) => basket([
            (Asset::LocationIdentity(Location::Fridge), 1),
            (Asset::Ambient, 1),
            (Asset::Unpowered, 1),
        ]),
        _ => unreachable!("only installed cohort/exposure pairs request storage facts"),
    }
}

fn claim_invariant() -> LinearInvariant<Asset> {
    let mut invariant = LinearInvariant::new(path!("fruit-claims"));
    for cohort in COHORTS {
        invariant = invariant
            .weight(Asset::Claim(cohort), 1)
            .weight(Asset::Consumed(cohort), 1);
    }
    invariant
}

fn cohort_condition_invariant(cohort: Cohort) -> LinearInvariant<Asset> {
    let mut invariant = LinearInvariant::new(path!("cohort-condition").join(&path::write(&cohort)))
        .weight(Asset::Rotten(cohort), 1);
    for (candidate, exposure, _) in TIMED_CONDITIONS {
        if candidate == cohort {
            invariant = invariant.weight(fresh_asset(cohort, exposure), 1);
        }
    }
    invariant
}

fn clock_invariant() -> LinearInvariant<Asset> {
    let mut invariant = LinearInvariant::new(path!("one-current-time"));
    for moment in MOMENTS {
        invariant = invariant.weight(Asset::Now(moment), 1);
    }
    invariant
}

fn deadline_invariant(moment: Moment) -> LinearInvariant<Asset> {
    LinearInvariant::new(path!("deadline-phase").join(&path::write(&moment)))
        .weight(Asset::Before(moment), 1)
        .weight(Asset::Reached(moment), 1)
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
                .all(|rate| !world.is_applicable(&action(*rate, 1)))
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
