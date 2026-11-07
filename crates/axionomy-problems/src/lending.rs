//! Lending market: interest through a borrow index, rounding against the caller.
//!
//! Depositors receive vault shares; a borrower draws credit as scaled debt
//! shares; a clock accrues interest by growing one borrow index instead of
//! touching every account, the index pattern used by on-chain lending markets.
//! Every division is an explicit full-width `MulDiv`. Amounts a participant
//! pays round up and amounts a participant receives round down, so rounding
//! dust accrues to the market and no round trip can create value. Shares
//! locked at genesis keep the share price from being manipulated by a first
//! depositor.

use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison, QuantityExpression, Rate, Rounding, Trace, basket,
};

/// Fixed-point scale of the borrow index and the per-period rate (10^18).
pub const WAD: u64 = 1_000_000_000_000_000_000;
/// Shares minted at genesis and permanently locked in the market.
pub const LOCKED_SHARES: u64 = 1_000;
/// Cash contributed with the locked shares.
pub const SEED_CASH: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Market,
    Clock,
    Depositor(u8),
    Borrower(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    /// The settlement asset.
    Cash,
    /// A depositor's claim on the market's assets.
    Share,
    /// Genesis shares no rate can redeem.
    LockedShare,
    /// The market's record of all shares, locked or not.
    ShareSupply,
    /// A borrower's scaled debt: actual debt is `DebtShare × BorrowIndex ÷ WAD`.
    DebtShare,
    /// The market's record of all outstanding debt shares.
    DebtShareSupply,
    /// Cumulative interest multiplier, scaled by `WAD`.
    BorrowIndex,
    /// Interest per period, scaled by `WAD`; market state, not a caller choice.
    RatePerPeriod,
    /// The most cash a borrower may owe.
    CreditLimit,
    MarketIdentity,
    ClockIdentity,
    Period,
    ElapsedPeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Market,
    Depositor,
    Borrower,
    Clock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    /// Pay cash, receive ⌊cash · supply ÷ assets⌋ shares.
    Deposit,
    /// Return shares, receive ⌊shares · assets ÷ supply⌋ cash.
    Redeem,
    /// Receive cash, owe ⌈cash · WAD ÷ index⌉ debt shares.
    Borrow,
    /// Pay cash, retire ⌊cash · WAD ÷ index⌋ debt shares.
    Repay,
    /// Retire debt shares, pay ⌈shares · index ÷ WAD⌉ cash.
    RepayShares,
    /// Grow the index by ⌈index · rate ÷ WAD⌉ for one period.
    Accrue,
    /// Give cash to the market with nothing in return.
    Donate,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type CycleTrace = Trace<RateId, Role, AccountId>;
pub type CycleError = Box<ApplyError<RateId, Role, AccountId, Asset>>;
type Q = QuantityExpression<Role, Asset>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Starting cash for each depositor.
    pub depositors: Vec<u64>,
    /// Starting cash and credit limit for each borrower.
    pub borrowers: Vec<(u64, u64)>,
    /// Interest periods available to the clock.
    pub periods: u64,
    /// Interest per period, scaled by `WAD` (`WAD / 100` is 1%).
    pub rate_per_period: u64,
}

impl Config {
    /// Two depositors, a well-funded third party, and one borrower over a
    /// year of monthly 1% periods.
    pub fn micro() -> Self {
        Self {
            depositors: vec![10_000, 10_000, 100_000],
            borrowers: vec![(1_000, 5_000)],
            periods: 12,
            rate_per_period: WAD / 100,
        }
    }
}

fn market_cash() -> Q {
    Q::balance(Role::Market, Asset::Cash)
}

fn index() -> Q {
    Q::balance(Role::Market, Asset::BorrowIndex)
}

fn supply() -> Q {
    Q::balance(Role::Market, Asset::ShareSupply)
}

fn wad() -> Q {
    Q::constant(WAD)
}

/// Cash plus outstanding debt. The debt is valued rounded down, so depositors
/// are never credited with interest the borrowers do not owe (each borrower's
/// own debt rounds up).
fn total_assets() -> Q {
    Q::plus(
        market_cash(),
        Q::mul_div(
            Q::balance(Role::Market, Asset::DebtShareSupply),
            index(),
            wad(),
            Rounding::Floor,
        ),
    )
}

fn market_rate() -> Rate<Role, Asset> {
    Rate::new().preserve(Role::Market, basket([(Asset::MarketIdentity, 1)]))
}

fn deposit_rate() -> Rate<Role, Asset> {
    let minted = Q::mul_div(Q::units(), supply(), total_assets(), Rounding::Floor);
    market_rate()
        .consume(Role::Depositor, basket([(Asset::Cash, 1)]))
        .produce(Role::Market, basket([(Asset::Cash, 1)]))
        .produce_computed(Role::Depositor, Asset::Share, minted.clone())
        .produce_computed(Role::Market, Asset::ShareSupply, minted.clone())
        .condition(
            "mints at least the minimum shares",
            minted,
            QuantityComparison::GreaterThanOrEqual,
            Q::maximum(Q::parameter("min_shares"), Q::constant(1)),
        )
        .distinct(Role::Market, Role::Depositor)
}

fn redeem_rate() -> Rate<Role, Asset> {
    let paid = Q::mul_div(Q::units(), total_assets(), supply(), Rounding::Floor);
    market_rate()
        .consume(Role::Depositor, basket([(Asset::Share, 1)]))
        .consume(Role::Market, basket([(Asset::ShareSupply, 1)]))
        .consume_computed(Role::Market, Asset::Cash, paid.clone())
        .produce_computed(Role::Depositor, Asset::Cash, paid.clone())
        .condition(
            "pays at least the minimum cash",
            paid,
            QuantityComparison::GreaterThanOrEqual,
            Q::parameter("min_cash"),
        )
        .distinct(Role::Market, Role::Depositor)
}

fn borrow_rate() -> Rate<Role, Asset> {
    let minted = Q::mul_div(Q::units(), wad(), index(), Rounding::Ceil);
    let owed_after = Q::mul_div(
        Q::plus(Q::balance(Role::Borrower, Asset::DebtShare), minted.clone()),
        index(),
        wad(),
        Rounding::Ceil,
    );
    market_rate()
        .preserve(Role::Borrower, basket([(Asset::CreditLimit, 1)]))
        .consume(Role::Market, basket([(Asset::Cash, 1)]))
        .produce(Role::Borrower, basket([(Asset::Cash, 1)]))
        .produce_computed(Role::Borrower, Asset::DebtShare, minted.clone())
        .produce_computed(Role::Market, Asset::DebtShareSupply, minted)
        .condition(
            "within credit limit",
            owed_after,
            QuantityComparison::LessThanOrEqual,
            Q::balance(Role::Borrower, Asset::CreditLimit),
        )
        .distinct(Role::Market, Role::Borrower)
}

fn repay_rate() -> Rate<Role, Asset> {
    let retired = Q::mul_div(Q::units(), wad(), index(), Rounding::Floor);
    market_rate()
        .consume(Role::Borrower, basket([(Asset::Cash, 1)]))
        .produce(Role::Market, basket([(Asset::Cash, 1)]))
        .consume_computed(Role::Borrower, Asset::DebtShare, retired.clone())
        .consume_computed(Role::Market, Asset::DebtShareSupply, retired.clone())
        .condition(
            "retires debt",
            retired,
            QuantityComparison::GreaterThanOrEqual,
            Q::constant(1),
        )
        .distinct(Role::Market, Role::Borrower)
}

fn repay_shares_rate() -> Rate<Role, Asset> {
    let owed = Q::mul_div(Q::units(), index(), wad(), Rounding::Ceil);
    market_rate()
        .consume(Role::Borrower, basket([(Asset::DebtShare, 1)]))
        .consume(Role::Market, basket([(Asset::DebtShareSupply, 1)]))
        .consume_computed(Role::Borrower, Asset::Cash, owed.clone())
        .produce_computed(Role::Market, Asset::Cash, owed)
        .distinct(Role::Market, Role::Borrower)
}

fn accrue_rate() -> Rate<Role, Asset> {
    let interest = Q::mul_div(
        index(),
        Q::balance(Role::Market, Asset::RatePerPeriod),
        wad(),
        Rounding::Ceil,
    );
    market_rate()
        .preserve(Role::Clock, basket([(Asset::ClockIdentity, 1)]))
        .consume(Role::Clock, basket([(Asset::Period, 1)]))
        .produce(Role::Clock, basket([(Asset::ElapsedPeriod, 1)]))
        .produce_computed(Role::Market, Asset::BorrowIndex, interest)
        .condition(
            "one period at a time",
            Q::units(),
            QuantityComparison::Equal,
            Q::constant(1),
        )
        .distinct(Role::Market, Role::Clock)
}

fn donate_rate() -> Rate<Role, Asset> {
    market_rate()
        .consume(Role::Depositor, basket([(Asset::Cash, 1)]))
        .produce(Role::Market, basket([(Asset::Cash, 1)]))
        .distinct(Role::Market, Role::Depositor)
}

pub fn world(config: &Config) -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Market,
            Account::from(basket([
                (Asset::Cash, SEED_CASH),
                (Asset::LockedShare, LOCKED_SHARES),
                (Asset::ShareSupply, LOCKED_SHARES),
                (Asset::BorrowIndex, WAD),
                (Asset::RatePerPeriod, config.rate_per_period),
                (Asset::MarketIdentity, 1),
            ])),
        )
        .account(
            AccountId::Clock,
            Account::from(basket([
                (Asset::ClockIdentity, 1),
                (Asset::Period, config.periods),
            ])),
        );
    for (id, cash) in (0_u8..).zip(&config.depositors) {
        builder = builder.account(
            AccountId::Depositor(id),
            Account::from(basket([(Asset::Cash, *cash)])),
        );
    }
    for (id, (cash, limit)) in (0_u8..).zip(&config.borrowers) {
        builder = builder.account(
            AccountId::Borrower(id),
            Account::from(basket([(Asset::Cash, *cash), (Asset::CreditLimit, *limit)])),
        );
    }
    builder
        .rate(RateId::Deposit, deposit_rate())
        .rate(RateId::Redeem, redeem_rate())
        .rate(RateId::Borrow, borrow_rate())
        .rate(RateId::Repay, repay_rate())
        .rate(RateId::RepayShares, repay_shares_rate())
        .rate(RateId::Accrue, accrue_rate())
        .rate(RateId::Donate, donate_rate())
        .invariant(LinearInvariant::new("cash").weight(Asset::Cash, 1))
        .invariant(
            LinearInvariant::new("vault shares")
                .weight(Asset::Share, 1)
                .weight(Asset::LockedShare, 1)
                .weight(Asset::ShareSupply, -1),
        )
        .invariant(
            LinearInvariant::new("debt shares")
                .weight(Asset::DebtShare, 1)
                .weight(Asset::DebtShareSupply, -1),
        )
        .build()
        .expect("the lending model is well formed")
}

pub fn micro() -> World {
    world(&Config::micro())
}

fn with_market(rate: RateId, units: u64) -> Action {
    Exchange::new(rate, Quantity::new(units)).bind(Role::Market, AccountId::Market)
}

pub fn deposit(depositor: u8, cash: u64, min_shares: u64) -> Action {
    with_market(RateId::Deposit, cash)
        .bind(Role::Depositor, AccountId::Depositor(depositor))
        .parameter("min_shares", Quantity::new(min_shares))
}

pub fn redeem(depositor: u8, shares: u64, min_cash: u64) -> Action {
    with_market(RateId::Redeem, shares)
        .bind(Role::Depositor, AccountId::Depositor(depositor))
        .parameter("min_cash", Quantity::new(min_cash))
}

pub fn borrow(borrower: u8, cash: u64) -> Action {
    with_market(RateId::Borrow, cash).bind(Role::Borrower, AccountId::Borrower(borrower))
}

pub fn repay(borrower: u8, cash: u64) -> Action {
    with_market(RateId::Repay, cash).bind(Role::Borrower, AccountId::Borrower(borrower))
}

pub fn repay_shares(borrower: u8, debt_shares: u64) -> Action {
    with_market(RateId::RepayShares, debt_shares)
        .bind(Role::Borrower, AccountId::Borrower(borrower))
}

pub fn accrue() -> Action {
    with_market(RateId::Accrue, 1).bind(Role::Clock, AccountId::Clock)
}

pub fn donate(depositor: u8, cash: u64) -> Action {
    with_market(RateId::Donate, cash).bind(Role::Depositor, AccountId::Depositor(depositor))
}

/// Every period has elapsed and the first depositor holds more cash than it
/// started with: interest was earned and paid out.
pub fn cycle_goal(config: &Config) -> Goal<AccountId, Asset> {
    Goal::new()
        .require(
            AccountId::Clock,
            basket([(Asset::ElapsedPeriod, config.periods)]),
        )
        .require(
            AccountId::Depositor(0),
            basket([(Asset::Cash, config.depositors[0] + 1)]),
        )
}

pub fn balance(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).get()
}

// Disposable u128 views, independent of the kernel's expression evaluator.
fn floor(left: u64, right: u64, denominator: u64) -> u128 {
    u128::from(left) * u128::from(right) / u128::from(denominator)
}

fn ceil(left: u64, right: u64, denominator: u64) -> u128 {
    (u128::from(left) * u128::from(right)).div_ceil(u128::from(denominator))
}

/// Market cash plus outstanding debt valued rounded down.
pub fn total_assets_view(world: &World) -> u128 {
    let market = |asset| balance(world, AccountId::Market, asset);
    u128::from(market(Asset::Cash))
        + floor(
            market(Asset::DebtShareSupply),
            market(Asset::BorrowIndex),
            WAD,
        )
}

/// Cash a borrower owes now, rounded up.
pub fn debt_view(world: &World, borrower: u8) -> u128 {
    ceil(
        balance(world, AccountId::Borrower(borrower), Asset::DebtShare),
        balance(world, AccountId::Market, Asset::BorrowIndex),
        WAD,
    )
}

/// Cash a depositor's shares redeem for now, rounded down.
pub fn share_value_view(world: &World, depositor: u8) -> u128 {
    let shares = u128::from(balance(
        world,
        AccountId::Depositor(depositor),
        Asset::Share,
    ));
    shares * total_assets_view(world)
        / u128::from(balance(world, AccountId::Market, Asset::ShareSupply))
}

/// A full cycle: two deposits, a borrow against the credit line, every
/// period accrued, all debt repaid, and both depositors fully redeemed.
/// Amounts that depend on state are read from the evolving world, so the
/// trace is ordinary core exchanges that replay deterministically.
pub fn lending_cycle(config: &Config) -> Result<(World, CycleTrace), CycleError> {
    let mut world = world(config);
    let mut trace = Trace::new();
    let mut step = |world: &mut World, action: Action| -> Result<(), CycleError> {
        world.apply(action.clone()).map_err(Box::new)?;
        trace.push(action);
        Ok(())
    };
    let first = config.depositors[0] / 2;
    let second = config.depositors[1] / 2;
    step(&mut world, deposit(0, first, 1))?;
    step(&mut world, deposit(1, second, 1))?;
    let (_, limit) = config.borrowers[0];
    step(&mut world, borrow(0, limit * 4 / 5))?;
    for _ in 0..config.periods {
        step(&mut world, accrue())?;
    }
    let debt_shares = balance(&world, AccountId::Borrower(0), Asset::DebtShare);
    step(&mut world, repay_shares(0, debt_shares))?;
    for depositor in [0, 1] {
        let shares = balance(&world, AccountId::Depositor(depositor), Asset::Share);
        step(&mut world, redeem(depositor, shares, 0))?;
    }
    Ok((world, trace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::AssessmentStatus;
    use proptest::prelude::*;

    fn cash_total(world: &World, config: &Config) -> u128 {
        let accounts = [AccountId::Market]
            .into_iter()
            .chain((0..config.depositors.len() as u8).map(AccountId::Depositor))
            .chain((0..config.borrowers.len() as u8).map(AccountId::Borrower));
        accounts
            .map(|account| u128::from(balance(world, account, Asset::Cash)))
            .sum()
    }

    #[test]
    fn full_cycle_pays_compound_interest_and_conserves_cash() {
        let config = Config::micro();
        let initial = world(&config);
        let (world, trace) = lending_cycle(&config).unwrap();
        assert!(world.matches(&cycle_goal(&config)));
        assert_eq!(cash_total(&world, &config), cash_total(&initial, &config));

        // The borrower paid at least 4000 · 1.01¹² ≈ 4507.30 and at most one
        // atom of rounding per period more.
        let principal = 4_000_u128;
        let paid =
            principal + 1_000 - u128::from(balance(&world, AccountId::Borrower(0), Asset::Cash));
        let compounded_floor = principal * 101_u128.pow(12) / 100_u128.pow(12);
        assert!(paid > compounded_floor, "paid {paid}");
        assert!(paid <= compounded_floor + 1 + 12, "paid {paid}");
        assert_eq!(balance(&world, AccountId::Borrower(0), Asset::DebtShare), 0);
        assert_eq!(
            balance(&world, AccountId::Market, Asset::DebtShareSupply),
            0
        );

        // Depositors earned interest; the market kept the locked shares'
        // stake plus rounding dust, never less than its seed.
        for depositor in [0_u8, 1] {
            let start = config.depositors[usize::from(depositor)];
            assert!(balance(&world, AccountId::Depositor(depositor), Asset::Cash) > start);
            assert_eq!(
                balance(&world, AccountId::Depositor(depositor), Asset::Share),
                0
            );
        }
        assert!(balance(&world, AccountId::Market, Asset::Cash) >= SEED_CASH);
        assert_eq!(
            balance(&world, AccountId::Market, Asset::ShareSupply),
            LOCKED_SHARES
        );

        // The accepted trace replays to the identical state.
        let replayed = initial.replayed(&trace).unwrap();
        assert_eq!(replayed.state_key(), world.state_key());
    }

    #[test]
    fn depositor_interest_never_exceeds_borrower_interest() {
        let config = Config::micro();
        let (world, _) = lending_cycle(&config).unwrap();
        let borrower_interest = u128::from(config.borrowers[0].0)
            - u128::from(balance(&world, AccountId::Borrower(0), Asset::Cash));
        let depositor_interest: u128 = [0_u8, 1]
            .into_iter()
            .map(|d| {
                u128::from(balance(&world, AccountId::Depositor(d), Asset::Cash))
                    - u128::from(config.depositors[usize::from(d)])
            })
            .sum();
        let market_gain =
            u128::from(balance(&world, AccountId::Market, Asset::Cash)) - u128::from(SEED_CASH);
        assert!(depositor_interest <= borrower_interest);
        assert_eq!(depositor_interest + market_gain, borrower_interest);
    }

    #[test]
    fn credit_limit_and_liquidity_are_authoritative() {
        let mut world = micro();
        let before = world.state_key();
        let over_limit = world.assess(&borrow(0, 5_001));
        assert!(over_limit.issues().iter().any(|issue| matches!(
            issue,
            ApplyError::RateConditionFailed { condition, .. } if condition == "within credit limit"
        )));
        // Only the seed cash is available before anyone deposits.
        let illiquid = world.assess(&borrow(0, 1_001));
        assert_eq!(illiquid.status(), AssessmentStatus::Infeasible);
        assert_eq!(
            illiquid.shortfall(&AccountId::Market),
            Some(&basket([(Asset::Cash, 1)]))
        );
        assert_eq!(world.state_key(), before);
        world.apply(borrow(0, 1_000)).unwrap();
        assert_eq!(debt_view(&world, 0), 1_000);
    }

    #[test]
    fn accrual_is_one_period_at_a_time_and_clock_bound() {
        let world = micro();
        let double = with_market(RateId::Accrue, 2).bind(Role::Clock, AccountId::Clock);
        assert!(!world.is_applicable(&double));
        // A depositor account cannot pose as the clock.
        let forged = with_market(RateId::Accrue, 1).bind(Role::Clock, AccountId::Depositor(0));
        assert!(!world.is_applicable(&forged));
        // Nor can any account pose as the market.
        let forged_market = Exchange::new(RateId::Deposit, Quantity::new(10))
            .bind(Role::Market, AccountId::Depositor(1))
            .bind(Role::Depositor, AccountId::Depositor(0))
            .parameter("min_shares", Quantity::new(1));
        assert!(!world.is_applicable(&forged_market));
    }

    #[test]
    fn locked_shares_defeat_a_first_depositor_inflation_attack() {
        let config = Config::micro();
        let mut world = world(&config);
        // The attacker deposits one atom, then donates heavily to inflate the
        // share price before the victim deposits.
        world.apply(deposit(2, 1, 1)).unwrap();
        world.apply(donate(2, 50_000)).unwrap();
        let victim_deposit = 10_000;
        world.apply(deposit(0, victim_deposit, 1)).unwrap();
        let victim_shares = balance(&world, AccountId::Depositor(0), Asset::Share);
        assert!(victim_shares > 0);
        // The victim loses less than one share's worth to rounding.
        let price = total_assets_view(&world).div_ceil(u128::from(balance(
            &world,
            AccountId::Market,
            Asset::ShareSupply,
        )));
        assert!(u128::from(victim_deposit) - share_value_view(&world, 0) < price);

        // The attacker cannot recover the donation: it accrues to every share,
        // overwhelmingly the locked ones.
        let attacker_shares = balance(&world, AccountId::Depositor(2), Asset::Share);
        world.apply(redeem(2, attacker_shares, 0)).unwrap();
        let attacker_cash = balance(&world, AccountId::Depositor(2), Asset::Cash);
        assert!(
            attacker_cash < config.depositors[2] - 40_000,
            "attacker kept {attacker_cash}"
        );
    }

    #[test]
    fn slippage_bounds_are_enforced() {
        let world = micro();
        assert!(world.is_applicable(&deposit(0, 1_000, 1_000)));
        assert!(!world.is_applicable(&deposit(0, 1_000, 1_001)));
        // Dust deposits that would mint nothing are rejected, not absorbed.
        let mut inflated = micro();
        inflated.apply(donate(2, 99_000)).unwrap();
        assert!(!inflated.is_applicable(&deposit(0, 50, 0)));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn no_round_trip_creates_value(
            deposited in 1_u64..5_000,
            borrowed in 0_u64..4_000,
            periods in 0_u64..12,
            amount in 1_u64..5_000,
        ) {
            let config = Config::micro();
            let mut world = world(&config);
            world.apply(deposit(1, deposited, 1)).unwrap();
            let borrowed = borrowed.min(deposited + SEED_CASH);
            if borrowed > 0 {
                world.apply(borrow(0, borrowed)).unwrap();
            }
            for _ in 0..periods {
                world.apply(accrue()).unwrap();
            }

            // Deposit then redeem never returns more than was deposited.
            let mut vault = world.fork();
            if vault.apply(deposit(0, amount, 1)).is_ok() {
                let shares = balance(&vault, AccountId::Depositor(0), Asset::Share);
                vault.apply(redeem(0, shares, 0)).unwrap();
                prop_assert!(balance(&vault, AccountId::Depositor(0), Asset::Cash) <= config.depositors[0]);
            }

            // Borrow then repay everything never costs less than was borrowed.
            let mut credit = world.fork();
            let start = balance(&credit, AccountId::Borrower(0), Asset::Cash);
            let debt_before = balance(&credit, AccountId::Borrower(0), Asset::DebtShare);
            let cash = amount.min(balance(&credit, AccountId::Market, Asset::Cash));
            if credit.apply(borrow(0, cash)).is_ok() {
                let shares = balance(&credit, AccountId::Borrower(0), Asset::DebtShare) - debt_before;
                credit.apply(repay_shares(0, shares)).unwrap();
                prop_assert!(balance(&credit, AccountId::Borrower(0), Asset::Cash) <= start);
            }
        }
    }
}
