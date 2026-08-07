//! Cohort-indexed perishable inventory with explicit time and storage effects.
//!
//! Ten thousand physical units are represented as fungible claims over two
//! cohorts. Each cohort has one unique condition fact, so spoilage changes a
//! shared fact rather than rewriting every claim holder. A disposable event
//! agenda proposes due exchanges, but the economy remains the only authority
//! on timing, environment, identity, and effects.

use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    Receipt, Trace, basket,
};
use axionomy_search::pareto::{self, Objective, ObjectiveVector, ParetoError, ParetoSearchResult};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

pub const DEFAULT_WAREHOUSE_CLAIMS: u64 = 7_000;
pub const DEFAULT_FRIDGE_CLAIMS: u64 = 3_000;
pub const DEFAULT_TRANSFER: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    Warehouse,
    Fridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cohort {
    Ambient,
    Refrigerated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Exposure {
    Ambient,
    Cold,
    WarmedAfterOutage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Moment {
    Harvest,
    AmbientExpiry,
    WarmedExpiry,
    ColdExpiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    World,
    Storage(Location),
    Cohort(Cohort),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    MoveToFridge,
    Advance { from: Moment, to: Moment },
    LosePower,
    Eat { cohort: Cohort, exposure: Exposure },
    Spoil { cohort: Cohort, exposure: Exposure },
    Finish,
    SealStoragePlan,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type EventTrace = Trace<RateId, Role, AccountId>;
pub type EventReceipt = Receipt<RateId, Role, AccountId, Asset>;
pub type Failure = ApplyError<RateId, Role, AccountId, Asset>;
pub type ParetoResult = ParetoSearchResult<RateId, Role, AccountId, u64, ObjectiveKey, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveKey {
    UsableInventory,
    CoolingEnergy,
}

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
            LinearInvariant::new("storage environment")
                .weight(Asset::Ambient, 1)
                .weight(Asset::Cold, 1),
        )
        .invariant(
            LinearInvariant::new("fridge power")
                .weight(Asset::Powered, 1)
                .weight(Asset::Unpowered, 1),
        )
        .invariant(
            LinearInvariant::new("cooling energy")
                .weight(Asset::CoolingEnergy, 1)
                .weight(Asset::SpentCoolingEnergy, 1),
        )
        .invariant(clock_invariant())
        .invariant(deadline_invariant(Moment::AmbientExpiry))
        .invariant(deadline_invariant(Moment::WarmedExpiry))
        .invariant(deadline_invariant(Moment::ColdExpiry))
        .invariant(
            LinearInvariant::new("scenario lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .invariant(
            LinearInvariant::new("storage planning lifecycle")
                .weight(Asset::Planning, 1)
                .weight(Asset::PlanSolved, 1),
        )
        .build()
        .expect("perishables model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
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

pub fn candidates(world: &World) -> Vec<Action> {
    world.applicable(world.rate_ids().copied().map(|rate| action(rate, 1)))
}

pub fn storage_plan_goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::World, basket([(Asset::PlanSolved, 1)]))
}

/// A deliberately bounded policy set: transfer another 1,000 claims, advance
/// to ambient expiry, apply the due cohort effect, or seal the storage plan.
pub fn storage_plan_candidates(world: &World) -> Vec<Action> {
    storage_plan_candidates_with_transfer(world, DEFAULT_TRANSFER)
}

pub fn storage_plan_candidates_with_transfer(world: &World, transfer: u64) -> Vec<Action> {
    world.applicable([
        move_to_fridge(transfer),
        advance(Moment::Harvest, Moment::AmbientExpiry),
        spoil(Cohort::Ambient, Exposure::Ambient),
        seal_storage_plan(),
    ])
}

/// Exhaustively compares the bounded storage commitments while the underlying
/// transfer rate remains fungible and accepts arbitrary caller-selected units.
pub fn storage_plan_front(world: &World) -> Result<ParetoResult, ParetoError> {
    storage_plan_front_with_transfer(world, DEFAULT_TRANSFER)
}

pub fn storage_plan_front_with_transfer(
    world: &World,
    transfer: u64,
) -> Result<ParetoResult, ParetoError> {
    pareto::search(
        world,
        &storage_plan_goal(),
        |world| storage_plan_candidates_with_transfer(world, transfer),
        storage_objectives,
    )
}

pub fn storage_objectives(world: &World) -> ObjectiveVector<ObjectiveKey, u64> {
    ObjectiveVector::try_new([
        Objective::maximize(ObjectiveKey::UsableInventory, usable_inventory(world)),
        Objective::minimize(ObjectiveKey::CoolingEnergy, spent_cooling_energy(world)),
    ])
    .expect("storage objective schema is static and unique")
}

pub fn usable_inventory(world: &World) -> u64 {
    world
        .balance(
            &storage_account(Location::Fridge),
            &Asset::Claim(Cohort::Refrigerated),
        )
        .get()
}

pub fn spent_cooling_energy(world: &World) -> u64 {
    world
        .balance(
            &storage_account(Location::Fridge),
            &Asset::SpentCoolingEnergy,
        )
        .get()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimIndex {
    holdings: BTreeMap<Cohort, BTreeMap<AccountId, u64>>,
}

impl ClaimIndex {
    pub fn build(world: &World) -> Self {
        let mut index = Self {
            holdings: BTreeMap::new(),
        };
        for (account, balances) in world.accounts() {
            for (asset, quantity) in balances.balances().iter() {
                if let Asset::Claim(cohort) = asset {
                    index.set(*cohort, *account, quantity.get());
                }
            }
        }
        index
    }

    pub fn total(&self, cohort: Cohort) -> u128 {
        self.holdings
            .get(&cohort)
            .into_iter()
            .flatten()
            .map(|quantity| u128::from(*quantity.1))
            .sum()
    }

    pub fn total_claims(&self) -> u128 {
        COHORTS.into_iter().map(|cohort| self.total(cohort)).sum()
    }

    pub fn balance_entries(&self) -> usize {
        self.holdings.values().map(BTreeMap::len).sum()
    }

    pub fn holders(&self, cohort: Cohort) -> Vec<(AccountId, u64)> {
        self.holdings
            .get(&cohort)
            .into_iter()
            .flat_map(|holdings| holdings.iter())
            .map(|(account, quantity)| (*account, *quantity))
            .collect()
    }

    pub fn usable_total(&self, world: &World) -> u128 {
        TIMED_CONDITIONS
            .into_iter()
            .filter(|(cohort, exposure, due)| {
                world.balance(
                    &cohort_account(*cohort),
                    &Asset::Fresh(*cohort, *exposure, *due),
                ) == Quantity::new(1)
                    && world.balance(&AccountId::World, &Asset::Before(*due)) == Quantity::new(1)
            })
            .map(|(cohort, _, _)| self.total(cohort))
            .sum()
    }

    fn apply(&mut self, receipt: &EventReceipt) {
        for delta in receipt.deltas() {
            for (asset, quantity) in delta.consumed().iter() {
                if let Asset::Claim(cohort) = asset {
                    self.decrease(*cohort, *delta.account(), quantity.get());
                }
            }
            for (asset, quantity) in delta.produced().iter() {
                if let Asset::Claim(cohort) = asset {
                    self.increase(*cohort, *delta.account(), quantity.get());
                }
            }
        }
    }

    fn set(&mut self, cohort: Cohort, account: AccountId, quantity: u64) {
        if quantity == 0 {
            return;
        }
        self.holdings
            .entry(cohort)
            .or_default()
            .insert(account, quantity);
    }

    fn increase(&mut self, cohort: Cohort, account: AccountId, quantity: u64) {
        let balance = self
            .holdings
            .entry(cohort)
            .or_default()
            .entry(account)
            .or_default();
        *balance = balance
            .checked_add(quantity)
            .expect("a core-valid receipt cannot overflow the derived account balance");
    }

    fn decrease(&mut self, cohort: Cohort, account: AccountId, quantity: u64) {
        let holdings = self
            .holdings
            .get_mut(&cohort)
            .expect("a consumed claim exists in a correctly synchronized index");
        let balance = holdings
            .get_mut(&account)
            .expect("a consumed claim holder exists in a correctly synchronized index");
        *balance = balance
            .checked_sub(quantity)
            .expect("a core-valid receipt cannot underflow the derived account balance");
        if *balance == 0 {
            holdings.remove(&account);
        }
        if holdings.is_empty() {
            self.holdings.remove(&cohort);
        }
    }
}

#[derive(Debug, Clone)]
struct AgendaEntry {
    at: Moment,
    sequence: u64,
    action: Action,
}

impl PartialEq for AgendaEntry {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at && self.sequence == other.sequence
    }
}

impl Eq for AgendaEntry {}

impl PartialOrd for AgendaEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AgendaEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .at
            .cmp(&self.at)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

#[derive(Debug, Clone)]
pub struct EffectAgenda {
    pending: BinaryHeap<AgendaEntry>,
    next_sequence: u64,
}

impl EffectAgenda {
    pub fn build(world: &World) -> Self {
        let mut agenda = Self {
            pending: BinaryHeap::new(),
            next_sequence: 0,
        };
        for (cohort, exposure, due) in TIMED_CONDITIONS {
            if world.balance(
                &cohort_account(cohort),
                &Asset::Fresh(cohort, exposure, due),
            ) == Quantity::new(1)
            {
                agenda.schedule(cohort, exposure, due);
            }
        }
        agenda
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn next_due(&self) -> Option<Moment> {
        self.pending.peek().map(|entry| entry.at)
    }

    fn observe(&mut self, receipt: &EventReceipt) {
        for delta in receipt.deltas() {
            for (asset, _) in delta.produced().iter() {
                if let Asset::Fresh(cohort, exposure, due) = asset {
                    self.schedule(*cohort, *exposure, *due);
                }
            }
        }
    }

    fn schedule(&mut self, cohort: Cohort, exposure: Exposure, at: Moment) {
        let entry = AgendaEntry {
            at,
            sequence: self.next_sequence,
            action: spoil(cohort, exposure),
        };
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("the concrete agenda sequence cannot overflow");
        self.pending.push(entry);
    }

    fn pop_due(&mut self, now: Moment) -> Vec<Action> {
        let mut due = Vec::new();
        while self.pending.peek().is_some_and(|entry| entry.at <= now) {
            due.push(
                self.pending
                    .pop()
                    .expect("a peeked agenda entry exists")
                    .action,
            );
        }
        due
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectReport {
    at: Moment,
    applied: Vec<RateId>,
    stale: Vec<RateId>,
}

impl EffectReport {
    pub fn at(&self) -> Moment {
        self.at
    }

    pub fn applied(&self) -> &[RateId] {
        &self.applied
    }

    pub fn stale(&self) -> &[RateId] {
        &self.stale
    }
}

#[derive(Debug, Clone)]
pub struct ScenarioRun {
    world: World,
    trace: EventTrace,
    index: ClaimIndex,
    effects: Vec<EffectReport>,
}

impl ScenarioRun {
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn trace(&self) -> &EventTrace {
        &self.trace
    }

    pub fn claim_index(&self) -> &ClaimIndex {
        &self.index
    }

    pub fn effects(&self) -> &[EffectReport] {
        &self.effects
    }
}

pub fn run_outage_scenario(source: &World, moved: u64) -> Result<ScenarioRun, Failure> {
    let mut world = source.fork();
    let mut trace = EventTrace::new();
    let mut index = ClaimIndex::build(&world);
    let mut agenda = EffectAgenda::build(&world);

    if moved > 0 {
        apply_recorded(
            &mut world,
            &mut trace,
            &mut index,
            &mut agenda,
            move_to_fridge(moved),
        )?;
    }
    apply_recorded(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        advance(Moment::Harvest, Moment::AmbientExpiry),
    )?;
    let ambient = settle_due(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        Moment::AmbientExpiry,
    )?;
    apply_recorded(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        lose_power(),
    )?;
    apply_recorded(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        advance(Moment::AmbientExpiry, Moment::WarmedExpiry),
    )?;
    let warmed = settle_due(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        Moment::WarmedExpiry,
    )?;
    apply_recorded(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        advance(Moment::WarmedExpiry, Moment::ColdExpiry),
    )?;
    let cold = settle_due(
        &mut world,
        &mut trace,
        &mut index,
        &mut agenda,
        Moment::ColdExpiry,
    )?;
    apply_recorded(&mut world, &mut trace, &mut index, &mut agenda, finish())?;

    debug_assert_eq!(index, ClaimIndex::build(&world));
    Ok(ScenarioRun {
        world,
        trace,
        index,
        effects: vec![ambient, warmed, cold],
    })
}

fn apply_recorded(
    world: &mut World,
    trace: &mut EventTrace,
    index: &mut ClaimIndex,
    agenda: &mut EffectAgenda,
    action: Action,
) -> Result<(), Failure> {
    let receipt = world.apply(action)?;
    index.apply(&receipt);
    agenda.observe(&receipt);
    trace.push(receipt.exchange().clone());
    Ok(())
}

fn settle_due(
    world: &mut World,
    trace: &mut EventTrace,
    index: &mut ClaimIndex,
    agenda: &mut EffectAgenda,
    now: Moment,
) -> Result<EffectReport, Failure> {
    let mut report = EffectReport {
        at: now,
        applied: Vec::new(),
        stale: Vec::new(),
    };
    for action in agenda.pop_due(now) {
        let rate = *action.rate();
        match world.apply(action) {
            Ok(receipt) => {
                index.apply(&receipt);
                agenda.observe(&receipt);
                trace.push(receipt.exchange().clone());
                report.applied.push(rate);
            }
            Err(ApplyError::Infeasible { .. }) => report.stale.push(rate),
            Err(error) => return Err(error),
        }
    }
    Ok(report)
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

fn active_world(rate: Rate<Role, Asset>) -> Rate<Role, Asset> {
    rate.preserve(
        Role::World,
        basket([(Asset::WorldIdentity, 1), (Asset::Active, 1)]),
    )
}

fn all_distinct(mut rate: Rate<Role, Asset>, roles: &[Role]) -> Rate<Role, Asset> {
    for (offset, left) in roles.iter().enumerate() {
        for right in &roles[offset + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    rate
}

fn move_to_fridge_rate() -> Rate<Role, Asset> {
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

fn advance_rate(from: Moment, to: Moment) -> Rate<Role, Asset> {
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

fn lose_power_rate() -> Rate<Role, Asset> {
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

fn eat_rate(cohort: Cohort, exposure: Exposure) -> Rate<Role, Asset> {
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

fn spoil_rate(cohort: Cohort, exposure: Exposure) -> Rate<Role, Asset> {
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

fn finish_rate() -> Rate<Role, Asset> {
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

fn seal_storage_plan_rate() -> Rate<Role, Asset> {
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
    let mut invariant = LinearInvariant::new("fruit claims");
    for cohort in COHORTS {
        invariant = invariant
            .weight(Asset::Claim(cohort), 1)
            .weight(Asset::Consumed(cohort), 1);
    }
    invariant
}

fn cohort_condition_invariant(cohort: Cohort) -> LinearInvariant<Asset> {
    let mut invariant = LinearInvariant::new(format!("{cohort:?} cohort condition"))
        .weight(Asset::Rotten(cohort), 1);
    for (candidate, exposure, _) in TIMED_CONDITIONS {
        if candidate == cohort {
            invariant = invariant.weight(fresh_asset(cohort, exposure), 1);
        }
    }
    invariant
}

fn clock_invariant() -> LinearInvariant<Asset> {
    let mut invariant = LinearInvariant::new("one current time");
    for moment in MOMENTS {
        invariant = invariant.weight(Asset::Now(moment), 1);
    }
    invariant
}

fn deadline_invariant(moment: Moment) -> LinearInvariant<Asset> {
    LinearInvariant::new(format!("{moment:?} deadline phase"))
        .weight(Asset::Before(moment), 1)
        .weight(Asset::Reached(moment), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::AssessmentStatus;
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

    #[test]
    fn ten_thousand_units_share_two_balance_entries_and_two_condition_facts() {
        let small = initial_with_inventory(7, 3);
        let large = initial();
        let index = ClaimIndex::build(&large);

        assert_eq!(large.state_key().len(), small.state_key().len());
        assert_eq!(index.total_claims(), 10_000);
        assert_eq!(index.balance_entries(), 2);
        assert_eq!(index.usable_total(&large), 10_000);
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
    fn storage_front_exposes_the_full_preservation_energy_curve() {
        let initial = initial();
        let result = storage_plan_front(&initial).unwrap();
        let mut outcomes = Vec::new();

        for entry in result.front().entries() {
            let replayed = initial.replayed(entry.payload()).unwrap();
            assert!(replayed.matches(&storage_plan_goal()));
            assert_eq!(&storage_objectives(&replayed), entry.objectives());
            outcomes.push((usable_inventory(&replayed), spent_cooling_energy(&replayed)));
        }

        outcomes.sort_unstable();
        assert_eq!(
            outcomes,
            [
                (3_000, 0),
                (4_000, 1_000),
                (5_000, 2_000),
                (6_000, 3_000),
                (7_000, 4_000),
                (8_000, 5_000),
                (9_000, 6_000),
                (10_000, 7_000),
            ]
        );
    }

    #[test]
    fn fungible_transfer_scales_one_exchange_without_scaling_shared_facts() {
        let mut world = initial();
        let receipt = world
            .apply(move_to_fridge(DEFAULT_TRANSFER))
            .expect("the batched transfer applies");
        let index = ClaimIndex::build(&world);

        assert_eq!(index.total(Cohort::Ambient), 6_000);
        assert_eq!(index.total(Cohort::Refrigerated), 4_000);
        assert_eq!(index.total_claims(), 10_000);
        assert_eq!(receipt.exchange().units(), &Quantity::new(1_000));
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
    fn event_driven_outage_run_replays_and_discards_stale_cold_event() {
        let source = initial();
        let before = source.state_key();
        let run = run_outage_scenario(&source, DEFAULT_TRANSFER).unwrap();

        assert_eq!(source.state_key(), before);
        assert!(run.world().matches(&goal()));
        assert_eq!(run.claim_index().total(Cohort::Ambient), 6_000);
        assert_eq!(run.claim_index().total(Cohort::Refrigerated), 4_000);
        assert_eq!(run.claim_index(), &ClaimIndex::build(run.world()));
        assert_eq!(run.effects()[0].applied().len(), 1);
        assert_eq!(run.effects()[1].applied().len(), 1);
        assert!(run.effects()[2].applied().is_empty());
        assert_eq!(run.effects()[2].stale().len(), 1);
        assert_eq!(
            run.effects()[2].stale(),
            &[RateId::Spoil {
                cohort: Cohort::Refrigerated,
                exposure: Exposure::Cold,
            }]
        );
        assert!(candidates(run.world()).is_empty());

        let replayed = source.replayed(run.trace()).unwrap();
        assert_eq!(replayed.state_key(), run.world().state_key());
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
            let run = run_outage_scenario(&source, moved).unwrap();

            prop_assert_eq!(
                run.claim_index().total(Cohort::Ambient),
                u128::from(oracle.warehouse_claims)
            );
            prop_assert_eq!(
                run.claim_index().total(Cohort::Refrigerated),
                u128::from(oracle.fridge_claims)
            );
            prop_assert_eq!(
                run.world().balance(
                    &cohort_account(Cohort::Ambient),
                    &Asset::Rotten(Cohort::Ambient),
                ) == Quantity::new(1),
                oracle.ambient_rotten
            );
            prop_assert_eq!(
                run.world().balance(
                    &cohort_account(Cohort::Refrigerated),
                    &Asset::Rotten(Cohort::Refrigerated),
                ) == Quantity::new(1),
                oracle.refrigerated_rotten
            );
            prop_assert_eq!(
                run.claim_index(),
                &ClaimIndex::build(run.world())
            );
            prop_assert!(source.replayed(run.trace()).is_ok());
        }
    }
}
