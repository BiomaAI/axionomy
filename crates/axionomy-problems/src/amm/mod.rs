#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{Economy, Exchange, Goal, Param, Quantity, Relation, basket};

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
    Showcase,
    Stress,
}

impl MarketSize {
    /// What the market's stock and every law's amounts are multiplied by:
    /// its economy's `{scale}`.
    const fn scale(self) -> u64 {
        match self {
            Self::Showcase => 1,
            Self::Stress => 4,
        }
    }
}

pub fn initial() -> World {
    crate::fixtures::initial(axionomy::path!("amm/micro"))
}

pub fn initial_showcase() -> World {
    initial_showcase_with_seed(0)
}

pub fn initial_stress() -> World {
    initial_stress_with_seed(0)
}

pub fn initial_showcase_with_seed(seed: u64) -> World {
    season::build(MarketSize::Showcase, 8, seed)
}

pub fn initial_stress_with_seed(seed: u64) -> World {
    season::build(MarketSize::Stress, 12, seed)
}

// The trader receives the output, so it rounds down; the product is full width.

// The provider receives shares, so each side rounds down.

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
