//! The Living Market: endogenous price discovery in a closed economy.
//!
//! Energy has value because actors need it to satisfy production and household
//! goals. Credit has value because it settles obligations and buys scarce
//! access to that energy. No external price or oracle exists: the AMM reserve
//! ratio is the economy's only public marginal price.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison, QuantityExpression as Q, Rate, Trace, basket,
};
use std::collections::{BTreeMap, BTreeSet};

pub const FEE_DENOMINATOR: u64 = 1_000;
pub const FEE_NUMERATOR: u64 = 997;
pub const MAX_LP_SHARES: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Actor {
    Founder,
    Generator,
    Factory,
    Household,
    Speculator,
    AdaptiveLp,
    Arbitrageur,
    Whale,
}

pub const ACTORS: [Actor; 8] = [
    Actor::Founder,
    Actor::Generator,
    Actor::Factory,
    Actor::Household,
    Actor::Speculator,
    Actor::AdaptiveLp,
    Actor::Arbitrageur,
    Actor::Whale,
];

/// Actors whose decisions can change the Market Day price. The founding LP's
/// initial reserve ratio is the counterfactual baseline rather than a later
/// intervention, so it is deliberately excluded from Shapley allocation.
pub const PRICE_ACTORS: [Actor; 7] = [
    Actor::Generator,
    Actor::Factory,
    Actor::Household,
    Actor::Speculator,
    Actor::AdaptiveLp,
    Actor::Arbitrageur,
    Actor::Whale,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Pool,
    Treasury,
    Information,
    Actor(Actor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Energy,
    Credit,
    SolarPotential,
    SpentEnergy,
    LpShare,
    UnissuedLpShare,
    Need(Actor),
    SatisfiedNeed(Actor),
    Obligation(Actor),
    SettledObligation(Actor),
    Uninformed(Actor),
    Informed(Actor),
    ShortageSignal,
    Utility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Pool,
    Trader,
    LiquidityProvider,
    Producer,
    Consumer,
    Treasury,
    Information,
    Observer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    BuyEnergy,
    SellEnergy,
    AddLiquidity,
    RemoveLiquidity,
    ProduceEnergy,
    UseEnergy(Actor),
    SettleObligation(Actor),
    LearnShortage(Actor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    MarketDay,
    NoWhale,
    ThinLiquidity,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolState {
    pub energy: u64,
    pub credit: u64,
    pub issued_lp_shares: u64,
    /// Credit per energy, scaled by 1,000 for exact display and comparison.
    pub price_milli: u64,
    pub product: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapleyContribution {
    pub actor: Actor,
    /// Signed contribution in price-milli units, before division.
    pub numerator: i128,
    pub denominator: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketSize {
    Micro,
    Showcase,
    Stress,
}

pub fn initial() -> World {
    build(MarketSize::Micro)
}

pub fn initial_showcase() -> World {
    build(MarketSize::Showcase)
}

pub fn initial_stress() -> World {
    build(MarketSize::Stress)
}

fn build(size: MarketSize) -> World {
    let (pool_energy, pool_credit, actor_scale) = match size {
        MarketSize::Micro => (4_000, 40_000, 1),
        MarketSize::Showcase => (10_000, 100_000, 1),
        MarketSize::Stress => (40_000, 400_000, 4),
    };
    let initial_lp = pool_energy;

    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Pool,
            Account::from(basket([
                (Asset::Energy, pool_energy),
                (Asset::Credit, pool_credit),
                (Asset::UnissuedLpShare, MAX_LP_SHARES - initial_lp),
            ])),
        )
        .account(
            AccountId::Treasury,
            Account::from(basket([(Asset::Credit, 20_000 * actor_scale)])),
        )
        .account(
            AccountId::Information,
            Account::from(basket([(Asset::ShortageSignal, 1)])),
        )
        .account(
            AccountId::Actor(Actor::Founder),
            Account::from(basket([
                (Asset::LpShare, initial_lp),
                (Asset::Credit, 5_000 * actor_scale),
            ])),
        )
        .account(
            AccountId::Actor(Actor::Generator),
            Account::from(basket([
                (Asset::Energy, 3_000 * actor_scale),
                (Asset::SolarPotential, 6_000 * actor_scale),
                (Asset::Credit, 5_000 * actor_scale),
                (Asset::Obligation(Actor::Generator), 1),
            ])),
        )
        .account(
            AccountId::Actor(Actor::Factory),
            Account::from(basket([
                (Asset::Credit, 40_000 * actor_scale),
                (Asset::Need(Actor::Factory), 1),
                (Asset::Obligation(Actor::Factory), 1),
            ])),
        )
        .account(
            AccountId::Actor(Actor::Household),
            Account::from(basket([
                // The shallow micro pool can move far enough that meeting the
                // household's fixed real-energy need costs more than it does
                // in the deeper showcase pool.
                (Asset::Credit, 20_000 * actor_scale),
                (Asset::Need(Actor::Household), 1),
            ])),
        )
        .account(
            AccountId::Actor(Actor::Speculator),
            Account::from(basket([
                (Asset::Credit, 25_000 * actor_scale),
                (Asset::Uninformed(Actor::Speculator), 1),
            ])),
        )
        .account(
            AccountId::Actor(Actor::AdaptiveLp),
            Account::from(basket([
                (Asset::Energy, 2_000 * actor_scale),
                (Asset::Credit, 20_000 * actor_scale),
            ])),
        )
        .account(
            AccountId::Actor(Actor::Arbitrageur),
            Account::from(basket([
                (Asset::Energy, 2_500 * actor_scale),
                (Asset::Credit, 10_000 * actor_scale),
            ])),
        )
        .account(
            AccountId::Actor(Actor::Whale),
            Account::from(basket([(Asset::Credit, 80_000 * actor_scale)])),
        );

    let buy_output = swap_output_expression(Asset::Credit, Asset::Energy);
    builder = builder.rate(
        RateId::BuyEnergy,
        swap_rate(Asset::Credit, Asset::Energy, buy_output),
    );
    let sell_output = swap_output_expression(Asset::Energy, Asset::Credit);
    builder = builder.rate(
        RateId::SellEnergy,
        swap_rate(Asset::Energy, Asset::Credit, sell_output),
    );
    builder = builder
        .rate(RateId::AddLiquidity, add_liquidity_rate())
        .rate(RateId::RemoveLiquidity, remove_liquidity_rate())
        .rate(
            RateId::ProduceEnergy,
            Rate::new()
                .consume(Role::Producer, basket([(Asset::SolarPotential, 1)]))
                .produce(Role::Producer, basket([(Asset::Energy, 1)])),
        );

    for actor in [Actor::Factory, Actor::Household] {
        let (energy, utility) = energy_use(actor);
        builder = builder.rate(
            RateId::UseEnergy(actor),
            Rate::new()
                .consume(
                    Role::Consumer,
                    basket([(Asset::Energy, energy), (Asset::Need(actor), 1)]),
                )
                .produce(
                    Role::Consumer,
                    basket([
                        (Asset::SpentEnergy, energy),
                        (Asset::SatisfiedNeed(actor), 1),
                        (Asset::Utility, utility),
                    ]),
                ),
        );
    }
    for actor in [Actor::Generator, Actor::Factory] {
        let cost = obligation_cost(actor);
        builder = builder.rate(
            RateId::SettleObligation(actor),
            Rate::new()
                .consume(
                    Role::Consumer,
                    basket([(Asset::Credit, cost), (Asset::Obligation(actor), 1)]),
                )
                .produce(
                    Role::Consumer,
                    basket([(Asset::SettledObligation(actor), 1)]),
                )
                .produce(Role::Treasury, basket([(Asset::Credit, cost)]))
                .distinct(Role::Consumer, Role::Treasury),
        );
    }
    builder = builder.rate(
        RateId::LearnShortage(Actor::Speculator),
        Rate::new()
            .preserve(Role::Information, basket([(Asset::ShortageSignal, 1)]))
            .consume(
                Role::Observer,
                basket([(Asset::Uninformed(Actor::Speculator), 1)]),
            )
            .produce(
                Role::Observer,
                basket([(Asset::Informed(Actor::Speculator), 1)]),
            )
            .distinct(Role::Information, Role::Observer),
    );

    let mut energy_invariant = LinearInvariant::new("closed energy accounting")
        .weight(Asset::Energy, 1)
        .weight(Asset::SolarPotential, 1)
        .weight(Asset::SpentEnergy, 1);
    let mut lifecycle = LinearInvariant::new("actor goal lifecycle");
    for actor in [Actor::Factory, Actor::Household] {
        lifecycle = lifecycle
            .weight(Asset::Need(actor), 1)
            .weight(Asset::SatisfiedNeed(actor), 1);
    }
    for actor in [Actor::Generator, Actor::Factory] {
        lifecycle = lifecycle
            .weight(Asset::Obligation(actor), 1)
            .weight(Asset::SettledObligation(actor), 1);
    }
    energy_invariant = energy_invariant.weight(Asset::Utility, 0);
    builder
        .invariant(energy_invariant)
        .invariant(LinearInvariant::new("closed credit accounting").weight(Asset::Credit, 1))
        .invariant(
            LinearInvariant::new("liquidity share accounting")
                .weight(Asset::LpShare, 1)
                .weight(Asset::UnissuedLpShare, 1),
        )
        .invariant(lifecycle)
        .invariant(
            LinearInvariant::new("shortage knowledge lifecycle")
                .weight(Asset::Uninformed(Actor::Speculator), 1)
                .weight(Asset::Informed(Actor::Speculator), 1),
        )
        .build()
        .expect("the Living Market model is coherent")
}

fn swap_output_expression(input: Asset, output: Asset) -> Q<Role, Asset> {
    let input_after_fee = Q::multiply(Q::constant(FEE_NUMERATOR), Q::units());
    Q::divide_floor(
        Q::multiply(Q::balance(Role::Pool, output), input_after_fee.clone()),
        Q::plus(
            Q::multiply(Q::balance(Role::Pool, input), Q::constant(FEE_DENOMINATOR)),
            input_after_fee,
        ),
    )
}

fn swap_rate(input: Asset, output: Asset, output_quantity: Q<Role, Asset>) -> Rate<Role, Asset> {
    Rate::new()
        .consume(Role::Trader, basket([(input, 1)]))
        .produce(Role::Pool, basket([(input, 1)]))
        .consume_computed(Role::Pool, output, output_quantity.clone())
        .produce_computed(Role::Trader, output, output_quantity.clone())
        .condition(
            "minimum output",
            output_quantity,
            QuantityComparison::GreaterThanOrEqual,
            Q::parameter("minimum_output"),
        )
        .distinct(Role::Pool, Role::Trader)
}

fn issued_lp_expression() -> Q<Role, Asset> {
    Q::subtract(
        Q::constant(MAX_LP_SHARES),
        Q::balance(Role::Pool, Asset::UnissuedLpShare),
    )
}

fn minted_lp_expression() -> Q<Role, Asset> {
    let issued = issued_lp_expression();
    Q::minimum(
        Q::divide_floor(
            Q::multiply(Q::parameter("energy_amount"), issued.clone()),
            Q::balance(Role::Pool, Asset::Energy),
        ),
        Q::divide_floor(
            Q::multiply(Q::parameter("credit_amount"), issued),
            Q::balance(Role::Pool, Asset::Credit),
        ),
    )
}

fn add_liquidity_rate() -> Rate<Role, Asset> {
    let minted = minted_lp_expression();
    Rate::new()
        .consume_computed(
            Role::LiquidityProvider,
            Asset::Energy,
            Q::parameter("energy_amount"),
        )
        .consume_computed(
            Role::LiquidityProvider,
            Asset::Credit,
            Q::parameter("credit_amount"),
        )
        .produce_computed(Role::Pool, Asset::Energy, Q::parameter("energy_amount"))
        .produce_computed(Role::Pool, Asset::Credit, Q::parameter("credit_amount"))
        .consume_computed(Role::Pool, Asset::UnissuedLpShare, minted.clone())
        .produce_computed(Role::LiquidityProvider, Asset::LpShare, minted.clone())
        .condition(
            "liquidity must preserve the discovered price",
            Q::multiply(
                Q::parameter("energy_amount"),
                Q::balance(Role::Pool, Asset::Credit),
            ),
            QuantityComparison::Equal,
            Q::multiply(
                Q::parameter("credit_amount"),
                Q::balance(Role::Pool, Asset::Energy),
            ),
        )
        .condition(
            "minimum liquidity shares",
            minted,
            QuantityComparison::GreaterThanOrEqual,
            Q::parameter("minimum_lp_shares"),
        )
        .distinct(Role::Pool, Role::LiquidityProvider)
}

fn remove_liquidity_rate() -> Rate<Role, Asset> {
    let issued = issued_lp_expression();
    let energy = Q::divide_floor(
        Q::multiply(Q::balance(Role::Pool, Asset::Energy), Q::units()),
        issued.clone(),
    );
    let credit = Q::divide_floor(
        Q::multiply(Q::balance(Role::Pool, Asset::Credit), Q::units()),
        issued,
    );
    Rate::new()
        .consume(Role::LiquidityProvider, basket([(Asset::LpShare, 1)]))
        .produce(Role::Pool, basket([(Asset::UnissuedLpShare, 1)]))
        .consume_computed(Role::Pool, Asset::Energy, energy.clone())
        .consume_computed(Role::Pool, Asset::Credit, credit.clone())
        .produce_computed(Role::LiquidityProvider, Asset::Energy, energy)
        .produce_computed(Role::LiquidityProvider, Asset::Credit, credit)
        .distinct(Role::Pool, Role::LiquidityProvider)
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new()
        .require(
            AccountId::Actor(Actor::Factory),
            basket([
                (Asset::SatisfiedNeed(Actor::Factory), 1),
                (Asset::SettledObligation(Actor::Factory), 1),
            ]),
        )
        .require(
            AccountId::Actor(Actor::Household),
            basket([(Asset::SatisfiedNeed(Actor::Household), 1)]),
        )
        .require(
            AccountId::Actor(Actor::Generator),
            basket([(Asset::SettledObligation(Actor::Generator), 1)]),
        )
        .require(
            AccountId::Actor(Actor::Speculator),
            basket([(Asset::Informed(Actor::Speculator), 1)]),
        )
}

pub fn buy_energy(actor: Actor, credit_input: u64, minimum_energy: u64) -> Action {
    Exchange::new(RateId::BuyEnergy, Quantity::new(credit_input))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::Trader, AccountId::Actor(actor))
        .parameter("minimum_output", Quantity::new(minimum_energy))
}

pub fn sell_energy(actor: Actor, energy_input: u64, minimum_credit: u64) -> Action {
    Exchange::new(RateId::SellEnergy, Quantity::new(energy_input))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::Trader, AccountId::Actor(actor))
        .parameter("minimum_output", Quantity::new(minimum_credit))
}

pub fn add_liquidity(actor: Actor, energy: u64, credit: u64, minimum_lp_shares: u64) -> Action {
    Exchange::new(RateId::AddLiquidity, Quantity::new(1))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::LiquidityProvider, AccountId::Actor(actor))
        .parameter("energy_amount", Quantity::new(energy))
        .parameter("credit_amount", Quantity::new(credit))
        .parameter("minimum_lp_shares", Quantity::new(minimum_lp_shares))
}

pub fn remove_liquidity(actor: Actor, shares: u64) -> Action {
    Exchange::new(RateId::RemoveLiquidity, Quantity::new(shares))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::LiquidityProvider, AccountId::Actor(actor))
}

pub fn produce_energy(actor: Actor, units: u64) -> Action {
    Exchange::new(RateId::ProduceEnergy, Quantity::new(units))
        .bind(Role::Producer, AccountId::Actor(actor))
}

pub fn use_energy(actor: Actor) -> Action {
    Exchange::new(RateId::UseEnergy(actor), Quantity::new(1))
        .bind(Role::Consumer, AccountId::Actor(actor))
}

pub fn settle_obligation(actor: Actor) -> Action {
    Exchange::new(RateId::SettleObligation(actor), Quantity::new(1))
        .bind(Role::Consumer, AccountId::Actor(actor))
        .bind(Role::Treasury, AccountId::Treasury)
}

pub fn learn_shortage(actor: Actor) -> Action {
    Exchange::new(RateId::LearnShortage(actor), Quantity::new(1))
        .bind(Role::Information, AccountId::Information)
        .bind(Role::Observer, AccountId::Actor(actor))
}

fn energy_use(actor: Actor) -> (u64, u64) {
    match actor {
        Actor::Factory => (1_500, 150),
        Actor::Household => (350, 100),
        _ => (1, 1),
    }
}

fn obligation_cost(actor: Actor) -> u64 {
    match actor {
        Actor::Generator => 4_000,
        Actor::Factory => 6_000,
        _ => 1,
    }
}

pub fn pool_state(world: &World) -> PoolState {
    let energy = world.balance(&AccountId::Pool, &Asset::Energy).get();
    let credit = world.balance(&AccountId::Pool, &Asset::Credit).get();
    let unissued = world
        .balance(&AccountId::Pool, &Asset::UnissuedLpShare)
        .get();
    PoolState {
        energy,
        credit,
        issued_lp_shares: MAX_LP_SHARES - unissued,
        price_milli: credit
            .saturating_mul(1_000)
            .checked_div(energy)
            .unwrap_or_default(),
        product: u128::from(energy) * u128::from(credit),
    }
}

pub fn quote_output(world: &World, input: Asset, input_amount: u64) -> Option<u64> {
    let output = match input {
        Asset::Credit => Asset::Energy,
        Asset::Energy => Asset::Credit,
        _ => return None,
    };
    let reserve_in = u128::from(world.balance(&AccountId::Pool, &input).get());
    let reserve_out = u128::from(world.balance(&AccountId::Pool, &output).get());
    let after_fee = u128::from(input_amount) * u128::from(FEE_NUMERATOR);
    let denominator = reserve_in * u128::from(FEE_DENOMINATOR) + after_fee;
    if denominator == 0 {
        return None;
    }
    u64::try_from(reserve_out * after_fee / denominator).ok()
}

fn guarded_buy(world: &World, actor: Actor, input: u64) -> Action {
    let output = quote_output(world, Asset::Credit, input).unwrap_or_default();
    buy_energy(actor, input, output.saturating_mul(98) / 100)
}

fn guarded_buy_at_least(
    world: &World,
    actor: Actor,
    preferred_input: u64,
    required_output: u64,
) -> Action {
    let mut input = preferred_input;
    while quote_output(world, Asset::Credit, input).unwrap_or_default() < required_output {
        input = input
            .checked_add(100)
            .expect("canonical market input remains bounded");
    }
    let output = quote_output(world, Asset::Credit, input).unwrap_or_default();
    buy_energy(actor, input, output.saturating_mul(98) / 100)
}

fn guarded_sell(world: &World, actor: Actor, input: u64) -> Action {
    let output = quote_output(world, Asset::Energy, input).unwrap_or_default();
    sell_energy(actor, input, output.saturating_mul(98) / 100)
}

fn push(world: &mut World, trace: &mut Trace<RateId, Role, AccountId>, action: Action) {
    world
        .apply(action.clone())
        .expect("canonical Living Market action is applicable");
    trace.push(action);
}

pub fn trace(initial: &World, scenario: Scenario) -> Trace<RateId, Role, AccountId> {
    let mut included = ACTORS.into_iter().collect::<BTreeSet<_>>();
    if matches!(scenario, Scenario::NoWhale) {
        included.remove(&Actor::Whale);
    }
    trace_with_actors(initial, scenario, &included)
}

/// Replays the same deterministic Market Day policies for an arbitrary actor
/// coalition. Missing actors make no proposals; every remaining action is
/// re-quoted against the counterfactual reserve state before Axionomy applies
/// it. This is the causal primitive used by exact Shapley attribution.
pub fn market_day_with_actors(
    initial: &World,
    included: &BTreeSet<Actor>,
) -> Trace<RateId, Role, AccountId> {
    trace_with_actors(initial, Scenario::MarketDay, included)
}

fn trace_with_actors(
    initial: &World,
    scenario: Scenario,
    included: &BTreeSet<Actor>,
) -> Trace<RateId, Role, AccountId> {
    let mut world = initial.fork();
    let mut trace = Trace::new();
    let stress = pool_state(initial).energy >= 40_000;
    let scale = if stress { 4 } else { 1 };

    if matches!(scenario, Scenario::ThinLiquidity) {
        let shares = pool_state(&world).issued_lp_shares / 2;
        push(
            &mut world,
            &mut trace,
            remove_liquidity(Actor::Founder, shares),
        );
    } else if included.contains(&Actor::AdaptiveLp) {
        let state = pool_state(&world);
        let energy = state.energy / 10;
        let credit = state.credit / 10;
        push(
            &mut world,
            &mut trace,
            add_liquidity(Actor::AdaptiveLp, energy, credit, energy),
        );
    }

    if included.contains(&Actor::Generator) {
        push(
            &mut world,
            &mut trace,
            produce_energy(Actor::Generator, 1_500 * scale),
        );
        let action = guarded_sell(&world, Actor::Generator, 2_500 * scale);
        push(&mut world, &mut trace, action);
    }

    if included.contains(&Actor::Factory) {
        let action = guarded_buy_at_least(&world, Actor::Factory, 26_000 * scale, 1_500);
        push(&mut world, &mut trace, action);
        push(&mut world, &mut trace, use_energy(Actor::Factory));
        push(&mut world, &mut trace, settle_obligation(Actor::Factory));
    }

    if included.contains(&Actor::Household) {
        let action = guarded_buy_at_least(&world, Actor::Household, 5_000 * scale, 350);
        push(&mut world, &mut trace, action);
        push(&mut world, &mut trace, use_energy(Actor::Household));
    }

    if included.contains(&Actor::Speculator) {
        push(&mut world, &mut trace, learn_shortage(Actor::Speculator));
        let action = guarded_buy(&world, Actor::Speculator, 16_000 * scale);
        push(&mut world, &mut trace, action);
    }

    if included.contains(&Actor::Whale) && !matches!(scenario, Scenario::NoWhale) {
        let action = guarded_buy(&world, Actor::Whale, 45_000 * scale);
        push(&mut world, &mut trace, action);
    }

    if included.contains(&Actor::Generator) {
        push(
            &mut world,
            &mut trace,
            produce_energy(Actor::Generator, 1_800 * scale),
        );
        let action = guarded_sell(&world, Actor::Generator, 1_800 * scale);
        push(&mut world, &mut trace, action);
    }
    if included.contains(&Actor::Arbitrageur) {
        let action = guarded_sell(&world, Actor::Arbitrageur, 1_200 * scale);
        push(&mut world, &mut trace, action);
    }
    if included.contains(&Actor::Generator) {
        push(&mut world, &mut trace, settle_obligation(Actor::Generator));
    }
    trace
}

pub fn shapley_price_contributions(initial: &World) -> Vec<ShapleyContribution> {
    let actor_count = PRICE_ACTORS.len();
    let coalition_count = 1_usize << actor_count;
    let mut values = vec![0_i128; coalition_count];
    for (mask, value) in values.iter_mut().enumerate() {
        let included = PRICE_ACTORS
            .into_iter()
            .enumerate()
            .filter_map(|(index, actor)| ((mask & (1 << index)) != 0).then_some(actor))
            .collect::<BTreeSet<_>>();
        let trace = market_day_with_actors(initial, &included);
        let final_world = initial
            .replayed(&trace)
            .expect("every canonical actor coalition is replayable");
        *value = i128::from(pool_state(&final_world).price_milli);
    }

    let factorials = (0..=actor_count)
        .scan(1_u64, |factorial, value| {
            if value > 0 {
                *factorial *= value as u64;
            }
            Some(*factorial)
        })
        .collect::<Vec<_>>();
    let denominator = factorials[actor_count];
    PRICE_ACTORS
        .into_iter()
        .enumerate()
        .map(|(actor_index, actor)| {
            let mut numerator = 0_i128;
            for mask in 0..coalition_count {
                if mask & (1 << actor_index) != 0 {
                    continue;
                }
                let size = mask.count_ones() as usize;
                let weight = factorials[size] * factorials[actor_count - size - 1];
                let marginal = values[mask | (1 << actor_index)] - values[mask];
                numerator += i128::from(weight) * marginal;
            }
            ShapleyContribution {
                actor,
                numerator,
                denominator,
            }
        })
        .collect()
}

pub fn direct_price_contributions(
    initial: &World,
    trace: &Trace<RateId, Role, AccountId>,
) -> BTreeMap<Actor, i128> {
    let mut world = initial.fork();
    let mut contributions = BTreeMap::new();
    for exchange in trace.exchanges() {
        let before = i128::from(pool_state(&world).price_milli);
        world
            .apply(exchange.clone())
            .expect("trace was already valid");
        let after = i128::from(pool_state(&world).price_milli);
        if matches!(exchange.rate(), RateId::BuyEnergy | RateId::SellEnergy)
            && let Some(AccountId::Actor(actor)) = exchange.bindings().get(&Role::Trader)
        {
            *contributions.entry(*actor).or_default() += after - before;
        }
    }
    contributions
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn swaps_follow_the_independent_constant_product_quote() {
        let mut world = initial_showcase();
        let before = pool_state(&world);
        let expected = quote_output(&world, Asset::Credit, 10_000).unwrap();
        world
            .apply(buy_energy(Actor::Factory, 10_000, expected))
            .unwrap();
        let after = pool_state(&world);

        assert_eq!(before.energy - after.energy, expected);
        assert_eq!(after.credit - before.credit, 10_000);
        assert!(after.product >= before.product);
        assert!(after.price_milli > before.price_milli);
    }

    #[test]
    fn proportional_liquidity_changes_depth_without_changing_price() {
        let mut world = initial_showcase();
        let before = pool_state(&world);
        world
            .apply(add_liquidity(Actor::AdaptiveLp, 1_000, 10_000, 1_000))
            .unwrap();
        let after = pool_state(&world);

        assert_eq!(after.price_milli, before.price_milli);
        assert_eq!(after.issued_lp_shares, before.issued_lp_shares + 1_000);
        assert!(after.product > before.product);
    }

    #[test]
    fn market_day_is_closed_replayable_and_reaches_real_actor_goals() {
        let initial = initial_showcase();
        let trace = trace(&initial, Scenario::MarketDay);
        let final_world = initial.replayed(&trace).unwrap();

        assert!(final_world.matches(&goal()));
        assert!(trace.exchanges().len() >= 14);
        assert!(
            final_world
                .balance(&AccountId::Actor(Actor::Factory), &Asset::Utility)
                .get()
                > 0
        );
    }

    #[test]
    fn removing_the_whale_changes_the_discovered_price() {
        let initial = initial_showcase();
        let full = initial
            .replayed(&trace(&initial, Scenario::MarketDay))
            .unwrap();
        let no_whale = initial
            .replayed(&trace(&initial, Scenario::NoWhale))
            .unwrap();

        assert_ne!(
            pool_state(&full).price_milli,
            pool_state(&no_whale).price_milli
        );
        assert!(pool_state(&full).price_milli > pool_state(&no_whale).price_milli);
    }

    #[test]
    fn direct_actor_contributions_sum_to_swap_price_movement() {
        let initial = initial_showcase();
        let trace = trace(&initial, Scenario::MarketDay);
        let contributions = direct_price_contributions(&initial, &trace);
        let total: i128 = contributions.values().sum();
        let final_world = initial.replayed(&trace).unwrap();
        let actual = i128::from(pool_state(&final_world).price_milli)
            - i128::from(pool_state(&initial).price_milli);

        assert_eq!(total, actual);
        assert!(contributions.len() >= 6);
    }

    #[test]
    fn exact_shapley_contributions_allocate_the_full_counterfactual_price_change() {
        let initial = initial_showcase();
        let contributions = shapley_price_contributions(&initial);
        let denominator = i128::from(contributions[0].denominator);
        let allocated: i128 = contributions.iter().map(|entry| entry.numerator).sum();
        let empty = initial
            .replayed(&market_day_with_actors(&initial, &BTreeSet::new()))
            .unwrap();
        let full = initial
            .replayed(&market_day_with_actors(
                &initial,
                &PRICE_ACTORS.into_iter().collect(),
            ))
            .unwrap();
        let difference =
            i128::from(pool_state(&full).price_milli) - i128::from(pool_state(&empty).price_milli);

        assert_eq!(allocated, difference * denominator);
        assert_eq!(contributions.len(), PRICE_ACTORS.len());
    }

    #[test]
    fn every_market_day_swap_preserves_or_grows_constant_product() {
        let mut world = initial_showcase();
        let trace = trace(&world, Scenario::MarketDay);
        for exchange in trace.exchanges() {
            let before = pool_state(&world);
            world.apply(exchange.clone()).unwrap();
            let after = pool_state(&world);
            if matches!(exchange.rate(), RateId::BuyEnergy | RateId::SellEnergy) {
                assert!(after.product >= before.product);
            }
        }
    }

    #[test]
    fn a_round_trip_cannot_create_credit() {
        let mut world = initial_showcase();
        let input = 10_000;
        let energy = quote_output(&world, Asset::Credit, input).unwrap();
        world
            .apply(buy_energy(Actor::Factory, input, energy))
            .unwrap();
        let credit_back = quote_output(&world, Asset::Energy, energy).unwrap();
        world
            .apply(sell_energy(Actor::Factory, energy, credit_back))
            .unwrap();
        assert!(credit_back < input);
    }

    proptest! {
        #[test]
        fn exact_quotes_are_monotone_and_bounded(first in 1_u64..20_000, extra in 0_u64..20_000) {
            let world = initial_showcase();
            let second = first + extra;
            let first_output = quote_output(&world, Asset::Credit, first).unwrap();
            let second_output = quote_output(&world, Asset::Credit, second).unwrap();
            prop_assert!(second_output >= first_output);
            prop_assert!(second_output < pool_state(&world).energy);
        }
    }
}
