#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern, Quantity,
    QuantityComparison, QuantityExpression as Q, Rate, Relation, Rounding, basket,
};

#[cfg(feature = "packages")]
pub mod package;
mod season;

pub use season::market_goal;

pub const FEE_DENOMINATOR: u64 = 1_000;
pub const FEE_NUMERATOR: u64 = 997;
pub const MAX_LP_SHARES: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Pool,
    Treasury,
    Information,
    Actor(Actor),
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    MarketActive,
    MarketEnded,
    Day,
    AtDay(u8),
    MarketOpen,
    Trading,
    Done,
    Decisions,
    StorageCapacity,
    ProductionCapacity,
    ProductionAllowance,
    FutureCapacity,
    PendingCapacity,
    ConstructionDue,
    ReferencePrice,
    CurrentSunlight,
    LiquidityDecision,
    FutureNeed(Actor),
    MissedNeed(Actor),
    FutureObligation(Actor),
    DefaultedObligation(Actor),
    UnmadeGoods,
    Goods,
    DeliveredGoods,
    IncomeDue,
    FutureIncome,
    ReceivedIncome,
    MissedIncome,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Pool,
    Trader,
    LiquidityProvider,
    Producer,
    Consumer,
    Treasury,
    Information,
    Observer,
    World,
    Participant(Actor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    EndTrading(Actor),
    AdvanceDay(u8),
    CloseMarket,
    InvestProduction,
    CommissionProduction,
    DeliverGoods,
    CollectIncome,
    FundLiquidity,
    BuyEnergy,
    SellEnergy,
    AddLiquidity,
    RemoveLiquidity,
    ProduceEnergy,
    UseEnergy(Actor),
    SettleObligation(Actor),
    LearnShortage(Actor),
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `amm` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "amm";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

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
    initial_showcase_with_seed(0)
}

pub fn initial_stress() -> World {
    initial_stress_with_seed(0)
}

pub fn initial_showcase_with_seed(seed: u64) -> World {
    season::build(build(MarketSize::Showcase), 8, 1, seed)
}

pub fn initial_stress_with_seed(seed: u64) -> World {
    season::build(build(MarketSize::Stress), 12, 4, seed)
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
                .account(Role::Producer, own(Actor::Generator))
                .consume(Role::Producer, basket([(Asset::SolarPotential, 1)]))
                .produce(Role::Producer, basket([(Asset::Energy, 1)])),
        );

    builder = builder
        .rate_at(
            RateIdPattern::UseEnergy(consumer().var()).into(),
            use_energy_rate(1, false),
        )
        .rate_at(
            RateIdPattern::SettleObligation(consumer().var()).into(),
            settle_obligation_rate(1),
        );
    builder = builder.rate(
        RateId::LearnShortage(Actor::Speculator),
        with_information(Rate::new())
            .account(Role::Observer, own(Actor::Speculator))
            .preserve(Role::Information, basket([(Asset::ShortageSignal, 1)]))
            .consume(
                Role::Observer,
                basket([(Asset::Uninformed(Actor::Speculator), 1)]),
            )
            .produce(
                Role::Observer,
                basket([(Asset::Informed(Actor::Speculator), 1)]),
            ),
    );

    let mut energy_invariant = LinearInvariant::new(path!("closed-energy-accounting"))
        .weight(Asset::Energy, 1)
        .weight(Asset::SolarPotential, 1)
        .weight(Asset::SpentEnergy, 1);
    let mut lifecycle = LinearInvariant::new(path!("actor-goal-lifecycle"));
    for (actor, ..) in NEEDS {
        lifecycle = lifecycle
            .weight(Asset::Need(actor), 1)
            .weight(Asset::SatisfiedNeed(actor), 1);
    }
    for (actor, _) in OBLIGATIONS {
        lifecycle = lifecycle
            .weight(Asset::Obligation(actor), 1)
            .weight(Asset::SettledObligation(actor), 1);
    }
    energy_invariant = energy_invariant.weight(Asset::Utility, 0);
    builder
        .invariant(energy_invariant)
        .invariant(LinearInvariant::new(path!("closed-credit-accounting")).weight(Asset::Credit, 1))
        .invariant(
            LinearInvariant::new(path!("liquidity-share-accounting"))
                .weight(Asset::LpShare, 1)
                .weight(Asset::UnissuedLpShare, 1),
        )
        .invariant(lifecycle)
        .invariant(
            LinearInvariant::new(path!("shortage-knowledge-lifecycle"))
                .weight(Asset::Uninformed(Actor::Speculator), 1)
                .weight(Asset::Informed(Actor::Speculator), 1),
        )
        .build()
        .expect("the Living Market model is coherent")
}

/// The account of `actor`, which a rate only it may take fixes to its role.
fn own(actor: Actor) -> Pattern<AccountId> {
    Pattern::literal(AccountId::Actor(actor))
}

/// `rate` against the pool, its pool role fixed to the pool's account.
fn with_pool(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Pool, Pattern::literal(AccountId::Pool))
}

/// `rate` paying or paid by the treasury, its role fixed to the treasury's
/// account.
fn with_treasury(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Treasury, Pattern::literal(AccountId::Treasury))
}

/// `rate` reading the shortage signal, its information role fixed to the
/// information account.
fn with_information(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Information, Pattern::literal(AccountId::Information))
}

// The trader receives the output, so it rounds down; the product is full width.
fn swap_output_expression(input: Asset, output: Asset) -> Q<Ontology> {
    let input_after_fee = Q::constant(FEE_NUMERATOR) * Q::units();
    Q::mul_div(
        Q::balance(Role::Pool, output),
        input_after_fee.clone(),
        (Q::balance(Role::Pool, input) * Q::constant(FEE_DENOMINATOR)) + input_after_fee,
        Rounding::Floor,
    )
}

fn swap_rate(input: Asset, output: Asset, output_quantity: Q<Ontology>) -> Rate<Ontology> {
    with_pool(Rate::new())
        .consume(Role::Trader, basket([(input, 1)]))
        .produce(Role::Pool, basket([(input, 1)]))
        .consume_computed(Role::Pool, output, output_quantity.clone())
        .produce_computed(Role::Trader, output, output_quantity.clone())
        .condition(
            path!("minimum-output"),
            output_quantity,
            QuantityComparison::GreaterThanOrEqual,
            Q::parameter(path!("minimum-output")),
        )
        .distinct(Role::Pool, Role::Trader)
}

fn issued_lp_expression() -> Q<Ontology> {
    Q::constant(MAX_LP_SHARES) - Q::balance(Role::Pool, Asset::UnissuedLpShare)
}

// The provider receives shares, so each side rounds down.
fn minted_lp_expression() -> Q<Ontology> {
    let issued = issued_lp_expression();
    Q::minimum(
        Q::mul_div(
            Q::parameter(path!("energy-amount")),
            issued.clone(),
            Q::balance(Role::Pool, Asset::Energy),
            Rounding::Floor,
        ),
        Q::mul_div(
            Q::parameter(path!("credit-amount")),
            issued,
            Q::balance(Role::Pool, Asset::Credit),
            Rounding::Floor,
        ),
    )
}

/// Reserve paid out for `units` burned shares; the provider receives it, so it
/// rounds down.
pub(crate) fn withdrawn_reserve(reserve: Asset) -> Q<Ontology> {
    Q::mul_div(
        Q::balance(Role::Pool, reserve),
        Q::units(),
        issued_lp_expression(),
        Rounding::Floor,
    )
}

fn add_liquidity_rate() -> Rate<Ontology> {
    let minted = minted_lp_expression();
    with_pool(Rate::new())
        .consume_computed(
            Role::LiquidityProvider,
            Asset::Energy,
            Q::parameter(path!("energy-amount")),
        )
        .consume_computed(
            Role::LiquidityProvider,
            Asset::Credit,
            Q::parameter(path!("credit-amount")),
        )
        .produce_computed(
            Role::Pool,
            Asset::Energy,
            Q::parameter(path!("energy-amount")),
        )
        .produce_computed(
            Role::Pool,
            Asset::Credit,
            Q::parameter(path!("credit-amount")),
        )
        .consume_computed(Role::Pool, Asset::UnissuedLpShare, minted.clone())
        .produce_computed(Role::LiquidityProvider, Asset::LpShare, minted.clone())
        .condition(
            path!("liquidity-must-preserve-the-discovered-price"),
            Q::parameter(path!("energy-amount")) * Q::balance(Role::Pool, Asset::Credit),
            QuantityComparison::Equal,
            Q::parameter(path!("credit-amount")) * Q::balance(Role::Pool, Asset::Energy),
        )
        .condition(
            path!("minimum-liquidity-shares"),
            minted,
            QuantityComparison::GreaterThanOrEqual,
            Q::parameter(path!("minimum-lp-shares")),
        )
        .distinct(Role::Pool, Role::LiquidityProvider)
}

fn remove_liquidity_rate() -> Rate<Ontology> {
    let energy = withdrawn_reserve(Asset::Energy);
    let credit = withdrawn_reserve(Asset::Credit);
    with_pool(Rate::new())
        .consume(Role::LiquidityProvider, basket([(Asset::LpShare, 1)]))
        .produce(Role::Pool, basket([(Asset::UnissuedLpShare, 1)]))
        .consume_computed(Role::Pool, Asset::Energy, energy.clone())
        .consume_computed(Role::Pool, Asset::Credit, credit.clone())
        .produce_computed(Role::LiquidityProvider, Asset::Energy, energy)
        .produce_computed(Role::LiquidityProvider, Asset::Credit, credit)
        .distinct(Role::Pool, Role::LiquidityProvider)
}

pub fn goal() -> Goal<Ontology> {
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
        .parameter(path!("minimum-output"), Quantity::new(minimum_energy))
}

pub fn sell_energy(actor: Actor, energy_input: u64, minimum_credit: u64) -> Action {
    Exchange::new(RateId::SellEnergy, Quantity::new(energy_input))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::Trader, AccountId::Actor(actor))
        .parameter(path!("minimum-output"), Quantity::new(minimum_credit))
}

pub fn add_liquidity(actor: Actor, energy: u64, credit: u64, minimum_lp_shares: u64) -> Action {
    Exchange::new(RateId::AddLiquidity, Quantity::new(1))
        .bind(Role::Pool, AccountId::Pool)
        .bind(Role::LiquidityProvider, AccountId::Actor(actor))
        .parameter(path!("energy-amount"), Quantity::new(energy))
        .parameter(path!("credit-amount"), Quantity::new(credit))
        .parameter(path!("minimum-lp-shares"), Quantity::new(minimum_lp_shares))
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

/// Each actor with a need: the energy meeting it uses and the utility it
/// yields.
const NEEDS: [(Actor, u64, u64); 2] = [(Actor::Factory, 1_500, 150), (Actor::Household, 350, 100)];

/// Each actor with an obligation, and the credit settling it costs.
const OBLIGATIONS: [(Actor, u64); 2] = [(Actor::Generator, 4_000), (Actor::Factory, 6_000)];

/// The units a season's factory makes for each need it meets.
const GOODS: u64 = 100;

/// The actor whose need or obligation a law meets, `{actor}`.
fn consumer() -> Param<Actor> {
    Param::new(path!("actor"))
}

/// The numbers a need's or an obligation's terms give its law.
fn energy() -> Param<u64> {
    Param::new(path!("energy"))
}
fn utility() -> Param<u64> {
    Param::new(path!("utility"))
}
fn goods() -> Param<u64> {
    Param::new(path!("goods"))
}
fn cost() -> Param<u64> {
    Param::new(path!("cost"))
}

/// `amount` for each unit of the exchange, as a basket's entries are.
fn per_unit(amount: Param<u64>) -> Q<Ontology> {
    amount.quantity() * Q::units()
}

/// Any actor with a need meeting it, `use-energy/{actor}`, its consumer role
/// fixed to the actor: it uses the energy its need takes, at `scale`, for
/// the need's utility, and where the market makes `goods`, the factory turns
/// its unmade goods into goods and the household makes none. What each
/// need takes and gives is law structure: a relation's row for each actor.
pub(crate) fn use_energy_rate(scale: u64, goods: bool) -> Rate<Ontology> {
    let actor = consumer();
    let made = |actor| match actor {
        Actor::Factory => GOODS * scale,
        _ => 0,
    };
    let rate = Rate::new()
        .variable(actor.values(NEEDS.map(|(actor, ..)| actor)))
        .account(Role::Consumer, AccountIdPattern::Actor(actor.var()).into())
        .consume_at(Role::Consumer, one(AssetPattern::Need(actor.var())))
        .consume_computed(Role::Consumer, Asset::Energy, per_unit(energy()))
        .produce_at(
            Role::Consumer,
            one(AssetPattern::SatisfiedNeed(actor.var())),
        )
        .produce_computed(Role::Consumer, Asset::SpentEnergy, per_unit(energy()))
        .produce_computed(Role::Consumer, Asset::Utility, per_unit(utility()));
    if goods {
        rate.relation(Relation::each_giving(
            &actor,
            [energy(), utility(), self::goods()].map(|number| number.name().clone()),
            NEEDS.map(|(actor, energy, utility)| (actor, [energy * scale, utility, made(actor)])),
        ))
        .consume_computed(Role::Consumer, Asset::UnmadeGoods, per_unit(self::goods()))
        .produce_computed(Role::Consumer, Asset::Goods, per_unit(self::goods()))
    } else {
        rate.relation(Relation::each_giving(
            &actor,
            [energy(), utility()].map(|number| number.name().clone()),
            NEEDS.map(|(actor, energy, utility)| (actor, [energy * scale, utility])),
        ))
    }
}

/// Any actor with an obligation settling it with the treasury,
/// `settle-obligation/{actor}`, its consumer role fixed to the actor: it
/// pays the treasury what its obligation costs, at `scale`, a relation's
/// row for each actor.
pub(crate) fn settle_obligation_rate(scale: u64) -> Rate<Ontology> {
    let actor = consumer();
    with_treasury(Rate::new())
        .variable(actor.values(OBLIGATIONS.map(|(actor, _)| actor)))
        .relation(Relation::each_giving(
            &actor,
            [cost().name().clone()],
            OBLIGATIONS.map(|(actor, cost)| (actor, [cost * scale])),
        ))
        .account(Role::Consumer, AccountIdPattern::Actor(actor.var()).into())
        .consume_at(Role::Consumer, one(AssetPattern::Obligation(actor.var())))
        .consume_computed(Role::Consumer, Asset::Credit, per_unit(cost()))
        .produce_at(
            Role::Consumer,
            one(AssetPattern::SettledObligation(actor.var())),
        )
        .produce_computed(Role::Treasury, Asset::Credit, per_unit(cost()))
}

/// One unit of the asset `asset` names.
fn one(asset: AssetPattern) -> [(Pattern<Asset>, Quantity); 1] {
    [(asset.into(), Quantity::new(1))]
}

/// What the tests read of the pool: its reserves, the shares it has issued,
/// its price and its constant product.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PoolState {
    energy: u64,
    credit: u64,
    issued_lp_shares: u64,
    /// Credit per energy, scaled by 1,000.
    price_milli: u64,
    product: u128,
}

#[cfg(test)]
fn pool_state(world: &World) -> PoolState {
    let energy = world.balance(&AccountId::Pool, &Asset::Energy).to_u64();
    let credit = world.balance(&AccountId::Pool, &Asset::Credit).to_u64();
    let unissued = world
        .balance(&AccountId::Pool, &Asset::UnissuedLpShare)
        .to_u64();
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The swap's output computed apart from the kernel, in `u128`: the
    /// oracle the pool's rate law is checked against.
    fn quote_output(world: &World, input: Asset, input_amount: u64) -> Option<u64> {
        let output = match input {
            Asset::Credit => Asset::Energy,
            Asset::Energy => Asset::Credit,
            _ => return None,
        };
        let reserve_in = u128::from(world.balance(&AccountId::Pool, &input).to_u64());
        let reserve_out = u128::from(world.balance(&AccountId::Pool, &output).to_u64());
        let after_fee = u128::from(input_amount) * u128::from(FEE_NUMERATOR);
        let denominator = reserve_in * u128::from(FEE_DENOMINATOR) + after_fee;
        if denominator == 0 {
            return None;
        }
        u64::try_from(reserve_out * after_fee / denominator).ok()
    }

    /// A Micro market day: the adaptive provider deepens the pool, the
    /// generator produces and sells, the factory and household buy what
    /// their needs take and use it, the speculator learns of the shortage
    /// and buys, the whale buys, then the generator and arbitrageur sell
    /// into the scarcity and the generator settles. Each swap's minimum is
    /// 98% of its quote against the reserves the exchanges before it leave.
    fn market_day() -> Vec<Action> {
        vec![
            add_liquidity(Actor::AdaptiveLp, 400, 4_000, 400),
            produce_energy(Actor::Generator, 1_500),
            sell_energy(Actor::Generator, 2_500, 15_592),
            buy_energy(Actor::Factory, 26_000, 3_244),
            use_energy(Actor::Factory),
            settle_obligation(Actor::Factory),
            buy_energy(Actor::Household, 5_900, 344),
            use_energy(Actor::Household),
            learn_shortage(Actor::Speculator),
            buy_energy(Actor::Speculator, 16_000, 665),
            buy_energy(Actor::Whale, 45_000, 930),
            produce_energy(Actor::Generator, 1_800),
            sell_energy(Actor::Generator, 1_800, 62_517),
            sell_energy(Actor::Arbitrageur, 1_200, 14_560),
            settle_obligation(Actor::Generator),
        ]
    }

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
    fn a_market_day_is_closed_replayable_and_reaches_real_actor_goals() {
        let initial = initial();
        let trace: axionomy::Trace<Ontology> = market_day().into_iter().collect();
        let final_world = initial.replayed(&trace).unwrap();

        assert!(final_world.matches(&goal()));
        assert!(
            final_world
                .balance(&AccountId::Actor(Actor::Factory), &Asset::Utility)
                .to_u64()
                > 0
        );
    }

    #[test]
    fn every_market_day_swap_preserves_or_grows_constant_product() {
        let mut world = initial();
        for exchange in market_day() {
            let before = pool_state(&world);
            let swap = matches!(exchange.rate(), RateId::BuyEnergy | RateId::SellEnergy);
            world.apply(exchange).unwrap();
            let after = pool_state(&world);
            if swap {
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
