//! Repeated production, consumption and investment over the same exact AMM.
//! A period closes only after every actor ends trading. Future needs, defaults,
//! construction and production limits are balances, not runner-owned state.

use super::*;
use axionomy::QuantityComparison as C;

const DECISIONS: u64 = 6;

fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, q(role, asset))
}
fn actor_law(rate: Rate<Ontology>, role: Role) -> Rate<Ontology> {
    rate.preserve_computed(role, Asset::MarketOpen, Q::constant(1))
        .preserve_computed(role, Asset::Trading, Q::constant(1))
        .consume_computed(role, Asset::Decisions, Q::constant(1))
}
fn world_law() -> Rate<Ontology> {
    Rate::new()
        .preserve(Role::World, basket([(Asset::WorldIdentity, 1)]))
        .condition(
            "market remains active",
            q(Role::World, Asset::MarketActive),
            C::Equal,
            Q::constant(1),
        )
        .condition("one period event", Q::units(), C::Equal, Q::constant(1))
}
fn weather(seed: u64, day: u8) -> u64 {
    let mut ticket = seed.wrapping_add(u64::from(day).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    ticket = (ticket ^ (ticket >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    ticket = (ticket ^ (ticket >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    if (ticket ^ (ticket >> 31)) % 3 == 0 {
        40
    } else {
        100
    }
}

pub(super) fn build(source: World, days: u8, scale: u64, seed: u64) -> World {
    let mut builder = EconomyBuilder::new();
    for (id, account) in source.accounts() {
        let mut account = account.clone();
        let mut additions = axionomy::Basket::new();
        match id {
            AccountId::Pool => {
                additions = basket([(Asset::PoolIdentity, 1), (Asset::MarketOpen, 1)]);
            }
            AccountId::Treasury => {
                additions = basket([
                    (Asset::TreasuryIdentity, 1),
                    (Asset::Credit, 80_000 * scale),
                ]);
            }
            AccountId::Information => {
                account
                    .withdraw(&basket([(Asset::ShortageSignal, 1)]))
                    .unwrap();
                if weather(seed, 1) < 100 {
                    additions = basket([(Asset::ShortageSignal, 1)]);
                }
            }
            AccountId::Actor(actor) => {
                let storage = match actor {
                    Actor::Founder => 20_000,
                    Actor::Generator => 3_500,
                    Actor::Factory => 2_500,
                    Actor::Household => 1_200,
                    Actor::Whale => 8_000,
                    _ => 3_000,
                } * scale;
                additions = basket([
                    (Asset::ActorIdentity(*actor), 1),
                    (Asset::MarketOpen, 1),
                    (Asset::Trading, 1),
                    (Asset::Decisions, DECISIONS),
                    (Asset::LiquidityDecision, 1),
                    (Asset::StorageCapacity, storage),
                    (Asset::ReferencePrice, 10_000),
                ]);
                if matches!(actor, Actor::Factory | Actor::Household) {
                    additions.insert(
                        Asset::FutureNeed(*actor),
                        Quantity::new(u64::from(days - 1)),
                    );
                }
                if matches!(actor, Actor::Generator | Actor::Factory) {
                    additions.insert(
                        Asset::FutureObligation(*actor),
                        Quantity::new(u64::from(days - 1)),
                    );
                }
                match actor {
                    Actor::Generator => {
                        additions.insert(Asset::ProductionCapacity, Quantity::new(800 * scale));
                        additions.insert(
                            Asset::ProductionAllowance,
                            Quantity::new(800 * scale * weather(seed, 0) / 100),
                        );
                        additions.insert(Asset::FutureCapacity, Quantity::new(800 * scale));
                        additions.insert(Asset::SolarPotential, Quantity::new(18_000 * scale));
                    }
                    Actor::Factory => {
                        additions.insert(
                            Asset::UnmadeGoods,
                            Quantity::new(u64::from(days) * 100 * scale),
                        );
                    }
                    Actor::Household => {
                        additions.insert(Asset::IncomeDue, Quantity::new(1));
                        additions.insert(Asset::FutureIncome, Quantity::new(u64::from(days - 1)));
                    }
                    _ => {}
                }
            }
            AccountId::World => unreachable!("base market has no clock"),
        }
        account.deposit(&additions).unwrap();
        builder = builder.account(*id, account);
    }
    let mut world = basket([
        (Asset::WorldIdentity, 1),
        (Asset::MarketActive, 1),
        (Asset::AtDay(0), 1),
        (Asset::Days, u64::from(days)),
        (Asset::Scale, scale),
        (Asset::CurrentSunlight, weather(seed, 0)),
    ]);
    for day in 0..days {
        world.insert(Asset::Sunlight(day), Quantity::new(weather(seed, day)));
    }
    builder = builder.account(AccountId::World, Account::from(world));

    for id in source.rate_ids() {
        let mut rate = source.rate(id).unwrap().clone();
        let role = match id {
            RateId::BuyEnergy | RateId::SellEnergy => Role::Trader,
            RateId::AddLiquidity | RateId::RemoveLiquidity => Role::LiquidityProvider,
            RateId::ProduceEnergy => Role::Producer,
            RateId::UseEnergy(_) | RateId::SettleObligation(_) => Role::Consumer,
            RateId::LearnShortage(_) => Role::Observer,
            _ => unreachable!("base market rate"),
        };
        match id {
            RateId::BuyEnergy => {
                rate = rate.condition(
                    "purchased energy fits storage",
                    q(role, Asset::Energy) + swap_output_expression(Asset::Credit, Asset::Energy),
                    C::LessThanOrEqual,
                    q(role, Asset::StorageCapacity),
                );
            }
            RateId::ProduceEnergy => {
                rate = rate
                    .preserve_computed(role, Asset::ActorIdentity(Actor::Generator), Q::constant(1))
                    .consume_computed(role, Asset::ProductionAllowance, Q::units())
                    .condition(
                        "production fits storage",
                        q(role, Asset::Energy) + Q::units(),
                        C::LessThanOrEqual,
                        q(role, Asset::StorageCapacity),
                    );
            }
            RateId::UseEnergy(actor) => {
                let (energy, utility) = energy_use(*actor);
                rate = Rate::new()
                    .preserve(role, basket([(Asset::ActorIdentity(*actor), 1)]))
                    .consume(
                        role,
                        basket([(Asset::Energy, energy * scale), (Asset::Need(*actor), 1)]),
                    )
                    .produce(
                        role,
                        basket([
                            (Asset::SpentEnergy, energy * scale),
                            (Asset::SatisfiedNeed(*actor), 1),
                            (Asset::Utility, utility),
                        ]),
                    );
                if *actor == Actor::Factory {
                    rate = rate
                        .consume(role, basket([(Asset::UnmadeGoods, 100 * scale)]))
                        .produce(role, basket([(Asset::Goods, 100 * scale)]));
                }
            }
            RateId::SettleObligation(actor) => {
                let cost = obligation_cost(*actor) * scale;
                rate = Rate::new()
                    .preserve(role, basket([(Asset::ActorIdentity(*actor), 1)]))
                    .consume(
                        role,
                        basket([(Asset::Credit, cost), (Asset::Obligation(*actor), 1)]),
                    )
                    .produce(role, basket([(Asset::SettledObligation(*actor), 1)]))
                    .preserve(Role::Treasury, basket([(Asset::TreasuryIdentity, 1)]))
                    .produce(Role::Treasury, basket([(Asset::Credit, cost)]))
                    .distinct(role, Role::Treasury);
            }
            RateId::AddLiquidity => {
                rate = rate.condition(
                    "one liquidity deposit",
                    Q::units(),
                    C::Equal,
                    Q::constant(1),
                );
            }
            RateId::RemoveLiquidity => {
                rate = rate.condition(
                    "withdrawn energy fits storage",
                    q(role, Asset::Energy) + withdrawn_reserve(Asset::Energy),
                    C::LessThanOrEqual,
                    q(role, Asset::StorageCapacity),
                );
            }
            _ => {}
        }
        if matches!(
            id,
            RateId::BuyEnergy | RateId::SellEnergy | RateId::AddLiquidity | RateId::RemoveLiquidity
        ) {
            rate = rate
                .preserve_computed(Role::Pool, Asset::PoolIdentity, Q::constant(1))
                .preserve_computed(Role::Pool, Asset::MarketOpen, Q::constant(1));
        }
        if matches!(id, RateId::AddLiquidity | RateId::RemoveLiquidity) {
            rate = rate.consume_computed(role, Asset::LiquidityDecision, Q::constant(1));
        }
        builder = builder.rate(*id, actor_law(rate, role));
    }

    for actor in ACTORS {
        builder = builder.rate(
            RateId::EndTrading(actor),
            clear(Rate::new(), Role::Consumer, Asset::Decisions)
                .preserve(
                    Role::Consumer,
                    basket([(Asset::ActorIdentity(actor), 1), (Asset::MarketOpen, 1)]),
                )
                .consume(Role::Consumer, basket([(Asset::Trading, 1)]))
                .produce(Role::Consumer, basket([(Asset::Done, 1)])),
        );
    }
    for day in 0..days {
        let last = day + 1 == days;
        let mut next = world_law()
            .consume(Role::World, basket([(Asset::AtDay(day), 1)]))
            .preserve(Role::Pool, basket([(Asset::PoolIdentity, 1)]));
        for actor in ACTORS {
            let role = Role::Participant(actor);
            next = next
                .preserve(role, basket([(Asset::ActorIdentity(actor), 1)]))
                .consume(role, basket([(Asset::Done, 1)]));
            if matches!(actor, Actor::Factory | Actor::Household) {
                next = clear(next, role, Asset::Need(actor)).produce_computed(
                    role,
                    Asset::MissedNeed(actor),
                    q(role, Asset::Need(actor)),
                );
                if !last {
                    next = next
                        .consume(role, basket([(Asset::FutureNeed(actor), 1)]))
                        .produce(role, basket([(Asset::Need(actor), 1)]));
                }
            }
            if matches!(actor, Actor::Generator | Actor::Factory) {
                next = clear(next, role, Asset::Obligation(actor)).produce_computed(
                    role,
                    Asset::DefaultedObligation(actor),
                    q(role, Asset::Obligation(actor)),
                );
                if !last {
                    next = next
                        .consume(role, basket([(Asset::FutureObligation(actor), 1)]))
                        .produce(role, basket([(Asset::Obligation(actor), 1)]));
                }
            }
            if actor == Actor::Household {
                next = clear(next, role, Asset::IncomeDue).produce_computed(
                    role,
                    Asset::MissedIncome,
                    q(role, Asset::IncomeDue),
                );
                if !last {
                    next = next
                        .consume(role, basket([(Asset::FutureIncome, 1)]))
                        .produce(role, basket([(Asset::IncomeDue, 1)]));
                }
            }
            if actor == Actor::Generator {
                next = clear(next, role, Asset::ProductionAllowance);
                if !last {
                    next = next.produce_computed(
                        role,
                        Asset::ProductionAllowance,
                        // The producer receives the allowance: round down.
                        Q::mul_div(
                            q(role, Asset::ProductionCapacity),
                            q(Role::World, Asset::Sunlight(day + 1)),
                            Q::constant(100),
                            Rounding::Floor,
                        ),
                    );
                }
            }
            if !last {
                // A price signal, not a payment; floor keeps it a pure observation.
                let closing_price = Q::mul_div(
                    q(Role::Pool, Asset::Credit),
                    Q::constant(1_000),
                    q(Role::Pool, Asset::Energy),
                    Rounding::Floor,
                );
                next = clear(
                    clear(next, role, Asset::ReferencePrice),
                    role,
                    Asset::LiquidityDecision,
                )
                .produce_computed(
                    role,
                    Asset::ReferencePrice,
                    Q::divide(
                        q(role, Asset::ReferencePrice) * Q::constant(3) + closing_price,
                        Q::constant(4),
                        Rounding::Floor,
                    ),
                )
                .produce(
                    role,
                    basket([
                        (Asset::Trading, 1),
                        (Asset::Decisions, DECISIONS),
                        (Asset::LiquidityDecision, 1),
                    ]),
                );
                if actor == Actor::Speculator {
                    next = clear(
                        clear(next, role, Asset::Informed(actor)),
                        role,
                        Asset::Uninformed(actor),
                    )
                    .produce(role, basket([(Asset::Uninformed(actor), 1)]));
                }
            } else {
                next = next.consume(role, basket([(Asset::MarketOpen, 1)]));
            }
        }
        if last {
            next = next
                .consume(Role::Pool, basket([(Asset::MarketOpen, 1)]))
                .consume(Role::World, basket([(Asset::MarketActive, 1)]))
                .produce(Role::World, basket([(Asset::MarketEnded, 1)]));
            builder = builder.rate(RateId::CloseMarket, next);
        } else {
            next = clear(
                clear(next, Role::Information, Asset::ShortageSignal),
                Role::World,
                Asset::CurrentSunlight,
            )
            .produce_computed(
                Role::World,
                Asset::CurrentSunlight,
                q(Role::World, Asset::Sunlight(day + 1)),
            )
            .produce(
                Role::Information,
                basket([(
                    Asset::ShortageSignal,
                    u64::from(day + 2 < days && weather(seed, day + 2) < 100),
                )]),
            )
            .produce(
                Role::World,
                basket([(Asset::AtDay(day + 1), 1), (Asset::Day, 1)]),
            );
            builder = builder.rate(RateId::AdvanceDay(day), next);
        }
    }

    let producer = Role::Producer;
    let invest = actor_law(world_law(), producer)
        .preserve(
            producer,
            basket([(Asset::ActorIdentity(Actor::Generator), 1)]),
        )
        .preserve(Role::Treasury, basket([(Asset::TreasuryIdentity, 1)]))
        .consume(
            producer,
            basket([
                (Asset::Credit, 6_000 * scale),
                (Asset::FutureCapacity, 400 * scale),
            ]),
        )
        .produce(producer, basket([(Asset::PendingCapacity, 400 * scale)]))
        .produce(Role::Treasury, basket([(Asset::Credit, 6_000 * scale)]))
        .produce_computed(
            producer,
            Asset::ConstructionDue,
            q(Role::World, Asset::Day) + Q::constant(2),
        )
        .condition(
            "one construction project",
            q(producer, Asset::PendingCapacity),
            C::Equal,
            Q::constant(0),
        )
        .condition(
            "capacity arrives before close",
            q(Role::World, Asset::Day) + Q::constant(3),
            C::LessThanOrEqual,
            q(Role::World, Asset::Days),
        )
        .distinct(producer, Role::Treasury);
    builder = builder.rate(RateId::InvestProduction, invest);
    // Commissioning is a due event, independent of the actor's trading budget.
    // Day-specific sunlight keeps the extra daily production entitlement honest.
    let current_sun = q(Role::World, Asset::CurrentSunlight);
    let commission = clear(world_law(), producer, Asset::ConstructionDue)
        .preserve(
            producer,
            basket([(Asset::ActorIdentity(Actor::Generator), 1)]),
        )
        .consume(producer, basket([(Asset::PendingCapacity, 400 * scale)]))
        .produce(producer, basket([(Asset::ProductionCapacity, 400 * scale)]))
        .produce_computed(
            producer,
            Asset::ProductionAllowance,
            Q::mul_div(
                Q::constant(400 * scale),
                current_sun,
                Q::constant(100),
                Rounding::Floor,
            ),
        )
        .condition(
            "construction has finished",
            q(Role::World, Asset::Day),
            C::GreaterThanOrEqual,
            q(producer, Asset::ConstructionDue),
        );
    builder = builder.rate(RateId::CommissionProduction, commission);
    builder = builder.rate(
        RateId::DeliverGoods,
        actor_law(Rate::new(), Role::Producer)
            .preserve(
                Role::Producer,
                basket([(Asset::ActorIdentity(Actor::Factory), 1)]),
            )
            .preserve(Role::Treasury, basket([(Asset::TreasuryIdentity, 1)]))
            .consume(Role::Producer, basket([(Asset::Goods, 100 * scale)]))
            .produce(
                Role::Treasury,
                basket([(Asset::DeliveredGoods, 100 * scale)]),
            )
            .consume(Role::Treasury, basket([(Asset::Credit, 15_000 * scale)]))
            .produce(Role::Producer, basket([(Asset::Credit, 15_000 * scale)]))
            .distinct(Role::Producer, Role::Treasury),
    );
    builder = builder.rate(
        RateId::CollectIncome,
        actor_law(Rate::new(), Role::Consumer)
            .preserve(
                Role::Consumer,
                basket([(Asset::ActorIdentity(Actor::Household), 1)]),
            )
            .preserve(Role::Treasury, basket([(Asset::TreasuryIdentity, 1)]))
            .consume(Role::Consumer, basket([(Asset::IncomeDue, 1)]))
            .produce(
                Role::Consumer,
                basket([(Asset::ReceivedIncome, 1), (Asset::Credit, 4_000 * scale)]),
            )
            .consume(Role::Treasury, basket([(Asset::Credit, 4_000 * scale)]))
            .distinct(Role::Consumer, Role::Treasury),
    );
    // The provider pays reserves to mint shares, so each side rounds up.
    let deposit = |reserve| {
        Q::mul_div(
            q(Role::Pool, reserve),
            Q::units(),
            issued_lp_expression(),
            Rounding::Ceil,
        )
    };
    let energy = deposit(Asset::Energy);
    let credit = deposit(Asset::Credit);
    builder = builder.rate(
        RateId::FundLiquidity,
        actor_law(Rate::new(), Role::LiquidityProvider)
            .preserve_computed(Role::Pool, Asset::PoolIdentity, Q::constant(1))
            .preserve_computed(Role::Pool, Asset::MarketOpen, Q::constant(1))
            .consume_computed(
                Role::LiquidityProvider,
                Asset::LiquidityDecision,
                Q::constant(1),
            )
            .consume_computed(Role::LiquidityProvider, Asset::Energy, energy.clone())
            .consume_computed(Role::LiquidityProvider, Asset::Credit, credit.clone())
            .produce_computed(Role::Pool, Asset::Energy, energy)
            .produce_computed(Role::Pool, Asset::Credit, credit)
            .consume(Role::Pool, basket([(Asset::UnissuedLpShare, 1)]))
            .produce(Role::LiquidityProvider, basket([(Asset::LpShare, 1)]))
            .distinct(Role::Pool, Role::LiquidityProvider),
    );

    for invariant in source.invariants() {
        let mut invariant = invariant.clone();
        if invariant.name() == "actor goal lifecycle" {
            for actor in [Actor::Factory, Actor::Household] {
                invariant = invariant
                    .weight(Asset::FutureNeed(actor), 1)
                    .weight(Asset::MissedNeed(actor), 1);
            }
            for actor in [Actor::Factory, Actor::Generator] {
                invariant = invariant
                    .weight(Asset::FutureObligation(actor), 1)
                    .weight(Asset::DefaultedObligation(actor), 1);
            }
        }
        builder = builder.invariant(invariant);
    }
    builder
        .invariant(
            LinearInvariant::new("market lifecycle")
                .weight(Asset::MarketActive, 1)
                .weight(Asset::MarketEnded, 1),
        )
        .invariant(
            LinearInvariant::new("production capacity lifecycle")
                .weight(Asset::ProductionCapacity, 1)
                .weight(Asset::FutureCapacity, 1)
                .weight(Asset::PendingCapacity, 1),
        )
        .invariant(
            LinearInvariant::new("manufactured goods lifecycle")
                .weight(Asset::UnmadeGoods, 1)
                .weight(Asset::Goods, 1)
                .weight(Asset::DeliveredGoods, 1),
        )
        .invariant(
            LinearInvariant::new("income entitlement lifecycle")
                .weight(Asset::FutureIncome, 1)
                .weight(Asset::IncomeDue, 1)
                .weight(Asset::ReceivedIncome, 1)
                .weight(Asset::MissedIncome, 1),
        )
        .build()
        .expect("seasonal market is coherent")
}

pub fn period(world: &World) -> u64 {
    world.balance(&AccountId::World, &Asset::Day).to_u64()
}
pub fn market_goal(world: &World) -> Goal<Ontology> {
    if world.account(&AccountId::World).is_some() {
        Goal::new().require(AccountId::World, basket([(Asset::MarketEnded, 1)]))
    } else {
        goal()
    }
}
fn balance(world: &World, actor: Actor, asset: Asset) -> u64 {
    world.balance(&AccountId::Actor(actor), &asset).to_u64()
}
fn basic(rate: RateId, role: Role, actor: Actor) -> Action {
    Exchange::new(rate, Quantity::new(1)).bind(role, AccountId::Actor(actor))
}
fn event(day: u8, last: bool) -> Action {
    let mut action = Exchange::new(
        if last {
            RateId::CloseMarket
        } else {
            RateId::AdvanceDay(day)
        },
        Quantity::new(1),
    )
    .bind(Role::World, AccountId::World)
    .bind(Role::Pool, AccountId::Pool);
    for actor in ACTORS {
        action = action.bind(Role::Participant(actor), AccountId::Actor(actor));
    }
    if !last {
        action = action.bind(Role::Information, AccountId::Information);
    }
    action
}
fn commission() -> Action {
    basic(
        RateId::CommissionProduction,
        Role::Producer,
        Actor::Generator,
    )
    .bind(Role::World, AccountId::World)
}
fn invest() -> Action {
    basic(RateId::InvestProduction, Role::Producer, Actor::Generator)
        .bind(Role::World, AccountId::World)
        .bind(Role::Treasury, AccountId::Treasury)
}
fn fund(actor: Actor, shares: u64) -> Action {
    Exchange::new(RateId::FundLiquidity, Quantity::new(shares))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::LiquidityProvider, AccountId::Actor(actor))
}
fn buy_to(world: &World, actor: Actor, target: u64, budget: u64) -> Option<Action> {
    let missing = target.saturating_sub(balance(world, actor, Asset::Energy));
    if missing == 0 || budget == 0 || quote_output(world, Asset::Credit, budget)? < missing {
        return None;
    }
    let (mut low, mut high) = (1, budget);
    while low < high {
        let middle = low + (high - low) / 2;
        if quote_output(world, Asset::Credit, middle)? >= missing {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let action = buy_energy(actor, low, missing);
    world.is_applicable(&action).then_some(action)
}
fn sell(world: &World, actor: Actor, units: u64) -> Option<Action> {
    let amount = units.min(balance(world, actor, Asset::Energy));
    if amount == 0 {
        return None;
    }
    let action = sell_energy(actor, amount, quote_output(world, Asset::Energy, amount)?);
    world.is_applicable(&action).then_some(action)
}

/// Every proposal responds to the latest reserves, inventory, entitlements,
/// capacity, credit and encoded closing-price memory. It installs no outcome.
pub fn seasonal_decision(world: &World, actor: Actor) -> Option<Action> {
    if balance(world, actor, Asset::Trading) == 0 || balance(world, actor, Asset::Decisions) == 0 {
        return None;
    }
    let scale = world.balance(&AccountId::World, &Asset::Scale).to_u64();
    let price = pool_state(world).price_milli;
    let reference = balance(world, actor, Asset::ReferencePrice);
    let energy = balance(world, actor, Asset::Energy);
    let cash = balance(world, actor, Asset::Credit);
    let remaining = balance(world, actor, Asset::Decisions);
    let applicable = |action: Action| world.is_applicable(&action).then_some(action);
    if matches!(actor, Actor::Factory | Actor::Generator)
        && balance(world, actor, Asset::Obligation(actor)) > 0
        && (remaining <= 2
            || (actor == Actor::Factory && balance(world, actor, Asset::Need(actor)) == 0))
        && let Some(action) = applicable(settle_obligation(actor))
    {
        return Some(action);
    }
    match actor {
        Actor::Founder => None,
        Actor::Generator => {
            if price > 12_500
                && cash > 10_000 * scale
                && let Some(action) = applicable(invest())
            {
                return Some(action);
            }
            if (energy > 800 * scale
                || (energy > 0 && (price > 9_000 || cash < obligation_cost(actor) * scale)))
                && let Some(action) = sell(world, actor, 1_000 * scale)
            {
                return Some(action);
            }
            let allowance = balance(world, actor, Asset::ProductionAllowance);
            let amount = allowance
                .min(balance(world, actor, Asset::SolarPotential))
                .min(balance(world, actor, Asset::StorageCapacity).saturating_sub(energy));
            if amount > 0 && price >= 7_000 {
                applicable(produce_energy(actor, amount))
            } else {
                applicable(settle_obligation(actor))
            }
        }
        Actor::Factory => {
            let deliver = basic(RateId::DeliverGoods, Role::Producer, actor)
                .bind(Role::Treasury, AccountId::Treasury);
            if let Some(action) = applicable(deliver) {
                return Some(action);
            }
            if balance(world, actor, Asset::Need(actor)) > 0 {
                if let Some(action) = applicable(use_energy(actor)) {
                    return Some(action);
                }
                // Production can be declined when input prices exceed the
                // procurement revenue; defaults and missed output remain visible.
                if price <= 19_000 {
                    return buy_to(
                        world,
                        actor,
                        1_500 * scale,
                        cash.saturating_sub(2_000 * scale),
                    );
                }
            }
            applicable(settle_obligation(actor))
        }
        Actor::Household => {
            let income = basic(RateId::CollectIncome, Role::Consumer, actor)
                .bind(Role::Treasury, AccountId::Treasury);
            if let Some(action) = applicable(income) {
                return Some(action);
            }
            if balance(world, actor, Asset::Need(actor)) > 0 {
                if let Some(action) = applicable(use_energy(actor)) {
                    return Some(action);
                }
                return buy_to(world, actor, 350 * scale, cash);
            }
            if price < 8_000 {
                buy_to(world, actor, 700 * scale, cash / 2)
            } else {
                None
            }
        }
        Actor::Speculator => {
            if let Some(action) = applicable(learn_shortage(actor)) {
                return Some(action);
            }
            if energy > 0 && price > reference * 11 / 10 {
                return sell(world, actor, 500 * scale);
            }
            if balance(world, actor, Asset::Informed(actor)) > 0 && price < reference * 11 / 10 {
                buy_to(
                    world,
                    actor,
                    (energy + 500 * scale).min(2_500 * scale),
                    cash / 3,
                )
            } else if price < reference * 9 / 10 {
                buy_to(world, actor, 1_000 * scale, cash / 3)
            } else {
                None
            }
        }
        Actor::Arbitrageur => {
            if price > reference * 11 / 10 {
                sell(world, actor, 500 * scale)
            } else if price < reference * 9 / 10 {
                buy_to(world, actor, 2_000 * scale, cash / 3)
            } else {
                None
            }
        }
        Actor::Whale => {
            if period(world) % 3 == 2 && price > reference {
                return sell(world, actor, 1_000 * scale);
            }
            if period(world) % 3 != 2 && price < 18_000 {
                buy_to(
                    world,
                    actor,
                    (energy + 500 * scale).min(5_000 * scale),
                    cash / 4,
                )
            } else {
                None
            }
        }
        Actor::AdaptiveLp => {
            let shares = balance(world, actor, Asset::LpShare);
            if shares > 0 && (price > reference * 12 / 10 || price < reference * 8 / 10) {
                return applicable(remove_liquidity(actor, shares.div_ceil(2)));
            }
            if price >= reference * 8 / 10 && price <= reference * 12 / 10 {
                let shares = (pool_state(world).issued_lp_shares / 20).max(1);
                applicable(fund(actor, shares))
            } else {
                None
            }
        }
    }
}

pub(super) fn run(
    initial: &World,
    scenario: Scenario,
    included: &BTreeSet<Actor>,
) -> Trace<Ontology> {
    let mut world = initial.clone();
    let mut trace = Trace::new();
    if scenario == Scenario::ThinLiquidity {
        let shares = pool_state(&world).issued_lp_shares / 2;
        push(
            &mut world,
            &mut trace,
            remove_liquidity(Actor::Founder, shares),
        );
    }
    let days = world.balance(&AccountId::World, &Asset::Days).to_u64() as u8;
    for day in 0..days {
        let due = commission();
        if world.is_applicable(&due) {
            push(&mut world, &mut trace, due);
        }
        for round in 0..=DECISIONS {
            let first = (usize::from(day) + round as usize) % ACTORS.len();
            for offset in 0..ACTORS.len() {
                let actor = ACTORS[(first + offset) % ACTORS.len()];
                if balance(&world, actor, Asset::Trading) == 0 {
                    continue;
                }
                let participating = included.contains(&actor)
                    && !(scenario == Scenario::NoWhale && actor == Actor::Whale);
                let next = participating
                    .then(|| seasonal_decision(&world, actor))
                    .flatten();
                let action =
                    next.unwrap_or_else(|| basic(RateId::EndTrading(actor), Role::Consumer, actor));
                push(&mut world, &mut trace, action);
            }
        }
        push(&mut world, &mut trace, event(day, day + 1 == days));
    }
    assert!(world.matches(&market_goal(&world)));
    trace
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end_day(world: &mut World) {
        for actor in ACTORS {
            if balance(world, actor, Asset::Trading) > 0 {
                world
                    .apply(basic(RateId::EndTrading(actor), Role::Consumer, actor))
                    .unwrap();
            }
        }
        let day = period(world) as u8;
        let last = u64::from(day + 1) == world.balance(&AccountId::World, &Asset::Days).to_u64();
        world.apply(event(day, last)).unwrap();
    }
    fn rejects_without_change(world: &mut World, action: Action) {
        let before = world.state_key();
        assert!(world.apply(action).is_err());
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn daily_entitlements_expire_and_refresh_only_at_an_authorized_boundary() {
        let mut world = initial_showcase();
        rejects_without_change(&mut world, event(0, false));
        let allowance = balance(&world, Actor::Factory, Asset::Decisions);
        for _ in 0..allowance {
            world.apply(buy_energy(Actor::Factory, 100, 1)).unwrap();
        }
        rejects_without_change(&mut world, buy_energy(Actor::Factory, 100, 1));
        end_day(&mut world);
        assert_eq!(period(&world), 1);
        assert_eq!(
            balance(&world, Actor::Factory, Asset::MissedNeed(Actor::Factory)),
            1
        );
        assert_eq!(
            balance(
                &world,
                Actor::Generator,
                Asset::DefaultedObligation(Actor::Generator)
            ),
            1
        );
        assert_eq!(
            balance(&world, Actor::Factory, Asset::Need(Actor::Factory)),
            1
        );
        assert_eq!(
            balance(&world, Actor::Factory, Asset::FutureNeed(Actor::Factory)),
            6
        );
        world.apply(buy_energy(Actor::Factory, 100, 1)).unwrap();
        rejects_without_change(&mut world, event(0, false));
        while !world.matches(&market_goal(&world)) {
            end_day(&mut world);
        }
        assert_eq!(
            balance(&world, Actor::Factory, Asset::MissedNeed(Actor::Factory)),
            8
        );
        rejects_without_change(&mut world, buy_energy(Actor::Factory, 100, 1));
        rejects_without_change(&mut world, event(7, true));
    }

    #[test]
    fn storage_and_daily_production_limits_are_enforced_by_the_economy() {
        let mut world = initial_showcase();
        rejects_without_change(&mut world, buy_energy(Actor::Household, 20_000, 1));
        world
            .apply(sell_energy(Actor::Generator, 1_000, 1))
            .unwrap();
        let allowance = balance(&world, Actor::Generator, Asset::ProductionAllowance);
        rejects_without_change(&mut world, produce_energy(Actor::Generator, allowance + 1));
        world
            .apply(produce_energy(Actor::Generator, allowance))
            .unwrap();
        rejects_without_change(&mut world, produce_energy(Actor::Generator, 1));
        rejects_without_change(
            &mut world,
            basic(RateId::DeliverGoods, Role::Producer, Actor::Generator)
                .bind(Role::Treasury, AccountId::Treasury),
        );
    }

    #[test]
    fn investment_spends_credit_now_and_adds_capacity_after_two_days() {
        let mut world = initial_showcase();
        rejects_without_change(&mut world, invest());
        world
            .apply(sell_energy(Actor::Generator, 1_000, 1))
            .unwrap();
        let before = balance(&world, Actor::Generator, Asset::Credit);
        world.apply(invest()).unwrap();
        assert_eq!(
            balance(&world, Actor::Generator, Asset::Credit),
            before - 6_000
        );
        assert_eq!(
            balance(&world, Actor::Generator, Asset::ProductionCapacity),
            800
        );
        rejects_without_change(&mut world, commission());
        end_day(&mut world);
        rejects_without_change(&mut world, commission());
        end_day(&mut world);
        world.apply(commission()).unwrap();
        assert_eq!(
            balance(&world, Actor::Generator, Asset::ProductionCapacity),
            1_200
        );
        assert_eq!(
            balance(&world, Actor::Generator, Asset::ProductionAllowance),
            1_200
                * world
                    .balance(&AccountId::World, &Asset::CurrentSunlight)
                    .to_u64()
                / 100
        );
        rejects_without_change(&mut world, commission());
    }

    #[test]
    fn liquidity_can_follow_changed_prices_with_at_most_one_atom_rounding_per_reserve() {
        let mut world = initial_showcase();
        world.apply(buy_energy(Actor::Factory, 10_000, 1)).unwrap();
        let before = pool_state(&world);
        world.apply(fund(Actor::AdaptiveLp, 50)).unwrap();
        let after = pool_state(&world);
        assert_eq!(after.issued_lp_shares - before.issued_lp_shares, 50);
        assert_eq!(
            after.energy - before.energy,
            (before.energy * 50).div_ceil(before.issued_lp_shares)
        );
        assert_eq!(
            after.credit - before.credit,
            (before.credit * 50).div_ceil(before.issued_lp_shares)
        );
        rejects_without_change(&mut world, fund(Actor::AdaptiveLp, 50));
        rejects_without_change(&mut world, remove_liquidity(Actor::AdaptiveLp, 50));
    }

    #[test]
    fn repeated_markets_preserve_every_need_and_produce_adaptive_investment_and_liquidity_paths() {
        let mut investments = 0;
        let mut commissions = 0;
        let mut withdrawals = 0;
        let mut deposits = 0;
        let mut goods_sold = 0;
        let mut missed = 0;
        for seed in [0, 7, 17, 29] {
            for initial in [
                initial_showcase_with_seed(seed),
                initial_stress_with_seed(seed),
            ] {
                let days = initial.balance(&AccountId::World, &Asset::Days).to_u64();
                for scenario in [
                    Scenario::MarketDay,
                    Scenario::NoWhale,
                    Scenario::ThinLiquidity,
                ] {
                    let trace = super::super::trace(&initial, scenario);
                    let final_world = initial.replayed(&trace).unwrap();
                    assert!(final_world.matches(&market_goal(&initial)));
                    for actor in [Actor::Factory, Actor::Household] {
                        assert_eq!(
                            balance(&final_world, actor, Asset::SatisfiedNeed(actor))
                                + balance(&final_world, actor, Asset::MissedNeed(actor)),
                            days
                        );
                        assert_eq!(balance(&final_world, actor, Asset::FutureNeed(actor)), 0);
                        missed += balance(&final_world, actor, Asset::MissedNeed(actor));
                    }
                    for actor in [Actor::Factory, Actor::Generator] {
                        assert_eq!(
                            balance(&final_world, actor, Asset::SettledObligation(actor))
                                + balance(&final_world, actor, Asset::DefaultedObligation(actor)),
                            days
                        );
                    }
                    for action in trace.exchanges() {
                        match action.rate() {
                            RateId::InvestProduction => investments += 1,
                            RateId::CommissionProduction => commissions += 1,
                            RateId::FundLiquidity => deposits += 1,
                            RateId::RemoveLiquidity
                                if action.bindings().get(&Role::LiquidityProvider)
                                    == Some(&AccountId::Actor(Actor::AdaptiveLp)) =>
                            {
                                withdrawals += 1
                            }
                            RateId::DeliverGoods => goods_sold += 1,
                            _ => {}
                        }
                    }
                    let contributions = direct_price_contributions(&initial, &trace);
                    assert_eq!(
                        contributions.values().sum::<i128>(),
                        i128::from(pool_state(&final_world).price_milli)
                            - i128::from(pool_state(&initial).price_milli)
                    );
                }
            }
        }
        assert!(
            investments > 0
                && commissions > 0
                && withdrawals > 0
                && deposits > 0
                && goods_sold > 0
                && missed > 0,
            "investment={investments}, commission={commissions}, withdrawal={withdrawals}, deposit={deposits}, goods={goods_sold}, missed={missed}"
        );
    }
}
