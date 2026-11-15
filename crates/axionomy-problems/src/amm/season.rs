//! Repeated production, consumption and investment over the same exact AMM.
//! A period closes only after every actor ends trading. Future needs, defaults,
//! construction and production limits are balances, not runner-owned state.

use super::*;
use axionomy::QuantityComparison as C;
use axionomy::path;

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
        .account(Role::World, Pattern::literal(AccountId::World))
        .condition(
            path!("market-remains-active"),
            q(Role::World, Asset::MarketActive),
            C::Equal,
            Q::constant(1),
        )
        .condition(
            path!("one-period-event"),
            Q::units(),
            C::Equal,
            Q::constant(1),
        )
}
fn weather(seed: u64, day: u8) -> u64 {
    if axionomy_search::sampling::splitmix(seed, u64::from(day)).is_multiple_of(3) {
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
                additions = basket([(Asset::MarketOpen, 1)]);
            }
            AccountId::Treasury => {
                additions = basket([(Asset::Credit, 80_000 * scale)]);
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
    let world = basket([
        (Asset::MarketActive, 1),
        (Asset::AtDay(0), 1),
        (Asset::Days, u64::from(days)),
        (Asset::Scale, scale),
        (Asset::CurrentSunlight, weather(seed, 0)),
    ]);
    builder = builder.account(AccountId::World, Account::from(world));

    // Meeting a need and settling an obligation, at the season's scale.
    builder = builder
        .rate_at(
            RateIdPattern::UseEnergy(consumer().var()).into(),
            actor_law(use_energy_rate(scale, true), Role::Consumer),
        )
        .rate_at(
            RateIdPattern::SettleObligation(consumer().var()).into(),
            actor_law(settle_obligation_rate(scale), Role::Consumer),
        );
    for (key, rate) in source.rates() {
        // The base market's needs and obligations are rebuilt above.
        let Some(id) = key.as_literal() else {
            continue;
        };
        let mut rate = rate.clone();
        let role = match id {
            RateId::BuyEnergy | RateId::SellEnergy => Role::Trader,
            RateId::AddLiquidity | RateId::RemoveLiquidity => Role::LiquidityProvider,
            RateId::ProduceEnergy => Role::Producer,
            RateId::LearnShortage(_) => Role::Observer,
            _ => unreachable!("base market rate"),
        };
        match id {
            RateId::BuyEnergy => {
                rate = rate.condition(
                    path!("purchased-energy-fits-storage"),
                    q(role, Asset::Energy) + swap_output_expression(Asset::Credit, Asset::Energy),
                    C::LessThanOrEqual,
                    q(role, Asset::StorageCapacity),
                );
            }
            RateId::ProduceEnergy => {
                rate = rate
                    .consume_computed(role, Asset::ProductionAllowance, Q::units())
                    .condition(
                        path!("production-fits-storage"),
                        q(role, Asset::Energy) + Q::units(),
                        C::LessThanOrEqual,
                        q(role, Asset::StorageCapacity),
                    );
            }
            RateId::AddLiquidity => {
                rate = rate.condition(
                    path!("one-liquidity-deposit"),
                    Q::units(),
                    C::Equal,
                    Q::constant(1),
                );
            }
            RateId::RemoveLiquidity => {
                rate = rate.condition(
                    path!("withdrawn-energy-fits-storage"),
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
            rate = rate.preserve_computed(Role::Pool, Asset::MarketOpen, Q::constant(1));
        }
        if matches!(id, RateId::AddLiquidity | RateId::RemoveLiquidity) {
            rate = rate.consume_computed(role, Asset::LiquidityDecision, Q::constant(1));
        }
        builder = builder.rate(*id, actor_law(rate, role));
    }

    // Any actor ending its trading for the day, `end-trading/{actor}`.
    let actor = Param::<Actor>::new(path!("actor"));
    builder = builder.rate_at(
        RateIdPattern::EndTrading(actor.var()).into(),
        clear(Rate::new(), Role::Consumer, Asset::Decisions)
            .variable(actor.every())
            .account(Role::Consumer, AccountIdPattern::Actor(actor.var()).into())
            .preserve(Role::Consumer, basket([(Asset::MarketOpen, 1)]))
            .consume(Role::Consumer, basket([(Asset::Trading, 1)]))
            .produce(Role::Consumer, basket([(Asset::Done, 1)])),
    );
    // Every day but the last closes as one rate, `advance-day/{day}`, into
    // day `{next}`. The season's weather never changes, so it is law
    // structure: a relation's row for each day gives the sunlight of the day
    // it opens, `{sunlight}`, which the world makes current and the
    // generator's allowance takes, and whether the day after falls short,
    // `{shortage}`, which the information account signals. The last day
    // closes the market.
    let day = Param::<u8>::new(path!("day"));
    let next = Param::<u8>::new(path!("next"));
    let (sunlight, shortage) = (
        Param::<u64>::new(path!("sunlight")),
        Param::<u64>::new(path!("shortage")),
    );
    let short = |day: u8| day < days && weather(seed, day) < 100;
    let on = |day: &Param<u8>| -> Vec<(Pattern<Asset>, Quantity)> {
        vec![(AssetPattern::AtDay(day.var()).into(), Quantity::new(1))]
    };
    let advance = close_day(
        world_law()
            .variable(day.range(0, days - 2))
            .relation(Relation::each_giving(
                &day,
                [sunlight.name().clone(), shortage.name().clone()],
                (0..days - 1).map(|day| (day, [weather(seed, day + 1), u64::from(short(day + 2))])),
            ))
            .derive(next.name().clone(), day.quantity() + Q::constant(1))
            .consume_at(Role::World, on(&day)),
        Some(sunlight.quantity()),
    );
    builder = builder
        .rate_at(
            RateIdPattern::AdvanceDay(day.var()).into(),
            clear(
                clear(
                    with_information(advance),
                    Role::Information,
                    Asset::ShortageSignal,
                ),
                Role::World,
                Asset::CurrentSunlight,
            )
            .produce_computed(Role::World, Asset::CurrentSunlight, sunlight.quantity())
            .produce_computed(
                Role::Information,
                Asset::ShortageSignal,
                shortage.quantity(),
            )
            .produce_at(Role::World, on(&next))
            .produce(Role::World, basket([(Asset::Day, 1)])),
        )
        .rate(
            RateId::CloseMarket,
            close_day(
                world_law().consume(Role::World, basket([(Asset::AtDay(days - 1), 1)])),
                None,
            )
            .consume(Role::Pool, basket([(Asset::MarketOpen, 1)]))
            .consume(Role::World, basket([(Asset::MarketActive, 1)]))
            .produce(Role::World, basket([(Asset::MarketEnded, 1)])),
        );

    let producer = Role::Producer;
    let invest = with_treasury(actor_law(world_law(), producer))
        .account(producer, own(Actor::Generator))
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
            path!("one-construction-project"),
            q(producer, Asset::PendingCapacity),
            C::Equal,
            Q::constant(0),
        )
        .condition(
            path!("capacity-arrives-before-close"),
            q(Role::World, Asset::Day) + Q::constant(3),
            C::LessThanOrEqual,
            q(Role::World, Asset::Days),
        );
    builder = builder.rate(RateId::InvestProduction, invest);
    // Commissioning is a due event, independent of the actor's trading budget.
    // Day-specific sunlight keeps the extra daily production entitlement honest.
    let current_sun = q(Role::World, Asset::CurrentSunlight);
    let commission = clear(world_law(), producer, Asset::ConstructionDue)
        .account(producer, own(Actor::Generator))
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
            path!("construction-has-finished"),
            q(Role::World, Asset::Day),
            C::GreaterThanOrEqual,
            q(producer, Asset::ConstructionDue),
        );
    builder = builder.rate(RateId::CommissionProduction, commission);
    builder = builder.rate(
        RateId::DeliverGoods,
        with_treasury(actor_law(Rate::new(), Role::Producer))
            .account(Role::Producer, own(Actor::Factory))
            .consume(Role::Producer, basket([(Asset::Goods, 100 * scale)]))
            .produce(
                Role::Treasury,
                basket([(Asset::DeliveredGoods, 100 * scale)]),
            )
            .consume(Role::Treasury, basket([(Asset::Credit, 15_000 * scale)]))
            .produce(Role::Producer, basket([(Asset::Credit, 15_000 * scale)])),
    );
    builder = builder.rate(
        RateId::CollectIncome,
        with_treasury(actor_law(Rate::new(), Role::Consumer))
            .account(Role::Consumer, own(Actor::Household))
            .consume(Role::Consumer, basket([(Asset::IncomeDue, 1)]))
            .produce(
                Role::Consumer,
                basket([(Asset::ReceivedIncome, 1), (Asset::Credit, 4_000 * scale)]),
            )
            .consume(Role::Treasury, basket([(Asset::Credit, 4_000 * scale)])),
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
        with_pool(actor_law(Rate::new(), Role::LiquidityProvider))
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
        if invariant.name().as_str() == "actor-goal-lifecycle" {
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
            LinearInvariant::new(path!("market-lifecycle"))
                .weight(Asset::MarketActive, 1)
                .weight(Asset::MarketEnded, 1),
        )
        .invariant(
            LinearInvariant::new(path!("production-capacity-lifecycle"))
                .weight(Asset::ProductionCapacity, 1)
                .weight(Asset::FutureCapacity, 1)
                .weight(Asset::PendingCapacity, 1),
        )
        .invariant(
            LinearInvariant::new(path!("manufactured-goods-lifecycle"))
                .weight(Asset::UnmadeGoods, 1)
                .weight(Asset::Goods, 1)
                .weight(Asset::DeliveredGoods, 1),
        )
        .invariant(
            LinearInvariant::new(path!("income-entitlement-lifecycle"))
                .weight(Asset::FutureIncome, 1)
                .weight(Asset::IncomeDue, 1)
                .weight(Asset::ReceivedIncome, 1)
                .weight(Asset::MissedIncome, 1),
        )
        .build()
        .expect("seasonal market is coherent")
}

/// `next` closing a day for every participant, each role fixed to its
/// actor's account, the pool's too: the needs, obligations and income left
/// are recorded missed, and the generator's allowance lapses. Then, where
/// `tomorrow` is the next day's sunlight, come the next day's entitlements,
/// the generator's allowance under that sunlight, each reference price moved
/// toward the closing price and the speculator uninformed again; without
/// one, the last day closes the market to every participant.
fn close_day(mut next: Rate<Ontology>, tomorrow: Option<Q<Ontology>>) -> Rate<Ontology> {
    let last = tomorrow.is_none();
    next = with_pool(next);
    for actor in ACTORS {
        let role = Role::Participant(actor);
        next = next
            .account(role, own(actor))
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
            if let Some(sunlight) = &tomorrow {
                next = next.produce_computed(
                    role,
                    Asset::ProductionAllowance,
                    // The producer receives the allowance: round down.
                    Q::mul_div(
                        q(role, Asset::ProductionCapacity),
                        sunlight.clone(),
                        Q::constant(100),
                        Rounding::Floor,
                    ),
                );
            }
        }
        if last {
            next = next.consume(role, basket([(Asset::MarketOpen, 1)]));
            continue;
        }
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
    }
    next
}

pub fn market_goal(world: &World) -> Goal<Ontology> {
    if world.account(&AccountId::World).is_some() {
        Goal::new().require(AccountId::World, basket([(Asset::MarketEnded, 1)]))
    } else {
        goal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn period(world: &World) -> u64 {
        world.balance(&AccountId::World, &Asset::Day).to_u64()
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
}
