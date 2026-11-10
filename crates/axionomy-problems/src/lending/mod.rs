//! Lending market: a collateralized bridge loan through a borrow index.
//!
//! Depositors hold vault shares in one pooled market. A borrower owes a bill
//! before its revenue arrives, so it posts collateral and draws cash as scaled
//! debt shares. A clock closes one period at a time: the interest rate follows
//! utilization along a kinked curve, accrues by growing one borrow index
//! instead of touching every account, and a reserve cut of each period's
//! interest stays with the market. An oracle may lower the collateral price
//! once, at a scheduled period. A period cannot close while the loan is past
//! its liquidation threshold; a keeper may then repay it and seize collateral
//! plus a bonus.
//!
//! Every division is an explicit full-width `MulDiv`. Amounts a participant
//! pays round up and amounts a participant receives round down, so rounding
//! dust accrues to the market and no round trip can create value. Shares
//! locked at genesis keep the share price from being manipulated by a first
//! depositor.

use axionomy::ConditionPath;
use axionomy::path;
use axionomy::{
    Account, ApplyError, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant,
    Quantity, QuantityComparison, QuantityExpression, Rate, Rounding, Trace, basket,
};
use axionomy_search::{SearchSolution, bfs, dijkstra};

mod study;
mod view;

pub use study::Lending;

/// Fixed-point scale of the borrow index and every market term (10^18).
pub const WAD: u64 = 1_000_000_000_000_000_000;
/// Shares minted at genesis and permanently locked in the market.
pub const LOCKED_SHARES: u64 = 1_000;
/// Cash contributed with the locked shares.
pub const SEED_CASH: u64 = 1_000;

/// `value` hundredths of a percent, scaled by `WAD`.
pub const fn basis_points(value: u64) -> u64 {
    WAD / 10_000 * value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Market,
    Clock,
    Oracle,
    Depositor(u8),
    Borrower,
    Keeper,
    Supplier,
    Customer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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
    /// Interest the market keeps instead of crediting depositors.
    Reserves,
    /// Collateral its holder may move freely.
    Collateral,
    /// Collateral securing the borrower's loan.
    PostedCollateral,
    /// Cash value of one collateral unit, set by the oracle.
    CollateralPrice,
    /// Interest per period at zero utilization.
    BaseRate,
    /// Utilization where the rate curve steepens.
    Kink,
    /// Interest per unit of utilization below the kink.
    SlopeBelowKink,
    /// Interest per unit of utilization above the kink.
    SlopeAboveKink,
    /// Share of each period's interest kept as reserves.
    ReserveFactor,
    /// Borrowing limit as a share of collateral value.
    LoanToValue,
    /// Debt above this share of collateral value can be liquidated.
    LiquidationThreshold,
    /// Extra collateral a keeper receives, as a share of the debt it repays.
    LiquidationBonus,
    /// Identifies the market's one borrower.
    CreditLine,
    /// The supplier's claim on the borrower's bill.
    Invoice,
    /// The borrower settled its bill.
    BillPaid,
    /// A customer's commitment to pay one revenue installment.
    PurchaseOrder(u8),
    MarketIdentity,
    ClockIdentity,
    OracleIdentity,
    Period,
    ElapsedPeriod,
    /// One while the scheduled price drop has not happened.
    PendingShock,
    /// Closed periods after which the price drop happens.
    ShockAfter,
    /// How far the price drops.
    PriceDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Market,
    Depositor,
    Borrower,
    Keeper,
    Clock,
    Oracle,
    Supplier,
    Customer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    /// Pay cash, receive ⌊cash · supply ÷ assets⌋ shares.
    Deposit,
    /// Return shares, receive ⌊shares · assets ÷ supply⌋ cash.
    Redeem,
    /// Give cash to the market with nothing in return.
    Donate,
    /// Lock collateral behind the loan.
    PostCollateral,
    /// Unlock collateral the loan does not need.
    WithdrawCollateral,
    /// Receive cash, owe ⌈cash · WAD ÷ index⌉ debt shares.
    Borrow,
    /// Pay cash, retire ⌊cash · WAD ÷ index⌋ debt shares.
    Repay,
    /// Retire debt shares, pay ⌈shares · index ÷ WAD⌉ cash.
    RepayShares,
    /// Settle the supplier's invoice before its deadline.
    PayBill,
    /// Collect one revenue installment once it is due.
    CollectRevenue(u8),
    /// Close one period: grow the index and keep the reserve cut.
    Accrue,
    /// Apply the scheduled collateral price drop.
    Reprice,
    /// Repay an unsafe loan and seize collateral plus the bonus.
    Liquidate,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `lending` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "lending";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub type LoanTrace = Trace<Ontology>;
pub type LoanError = Box<ApplyError<Ontology>>;
pub type Solution = SearchSolution<Ontology>;
type Q = QuantityExpression<Ontology>;

/// The market's published terms, each scaled by `WAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terms {
    pub base_rate: u64,
    pub kink: u64,
    pub slope_below_kink: u64,
    pub slope_above_kink: u64,
    pub reserve_factor: u64,
    pub loan_to_value: u64,
    pub liquidation_threshold: u64,
    pub liquidation_bonus: u64,
}

impl Terms {
    /// 0.2% a period when idle, 2.2% at the 80% kink, 12.2% when fully lent;
    /// 10% of interest to reserves; borrow to 60%, liquidated above 75%, with
    /// an 8% liquidation bonus.
    pub const fn standard() -> Self {
        Self {
            base_rate: basis_points(20),
            kink: basis_points(8_000),
            slope_below_kink: basis_points(250),
            slope_above_kink: basis_points(5_000),
            reserve_factor: basis_points(1_000),
            loan_to_value: basis_points(6_000),
            liquidation_threshold: basis_points(7_500),
            liquidation_bonus: basis_points(800),
        }
    }
}

/// An amount of cash tied to a number of closed periods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payment {
    pub amount: u64,
    pub period: u64,
}

/// The oracle lowers the price by `drop` once `after` periods have closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shock {
    pub after: u64,
    pub drop: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub terms: Terms,
    /// Cash each depositor has already deposited at the genesis share price.
    pub deposits: Vec<u64>,
    /// Cash each depositor still holds.
    pub depositor_cash: u64,
    pub periods: u64,
    pub collateral_price: u64,
    pub shock: Option<Shock>,
    pub borrower_cash: u64,
    pub collateral: u64,
    /// Payable while fewer than `period` periods have closed.
    pub bill: Payment,
    /// Each installment is collectable once `period` periods have closed.
    pub revenue: Vec<Payment>,
    pub keeper_cash: u64,
    /// Search granularity, not semantics: collateral per posting step.
    pub collateral_lot: u64,
}

impl Config {
    /// A two-period gap between the bill and revenue, with a price drop while
    /// the loan is open.
    pub fn micro() -> Self {
        Self {
            terms: Terms::standard(),
            deposits: vec![3_000, 3_000],
            depositor_cash: 2_000,
            periods: 4,
            collateral_price: 100,
            shock: Some(Shock { after: 2, drop: 30 }),
            borrower_cash: 500,
            collateral: 40,
            bill: Payment {
                amount: 2_500,
                period: 2,
            },
            revenue: vec![Payment {
                amount: 3_000,
                period: 3,
            }],
            keeper_cash: 5_000,
            collateral_lot: 20,
        }
    }

    /// A thin pool near its kink, a bill due early, revenue in two
    /// installments, and a 40% price drop while the loan is open.
    pub fn showcase() -> Self {
        Self {
            terms: Terms::standard(),
            deposits: vec![4_000, 3_000, 2_000],
            depositor_cash: 2_000,
            periods: 8,
            collateral_price: 50,
            shock: Some(Shock { after: 4, drop: 20 }),
            borrower_cash: 1_000,
            collateral: 400,
            bill: Payment {
                amount: 8_000,
                period: 3,
            },
            revenue: vec![
                Payment {
                    amount: 4_000,
                    period: 4,
                },
                Payment {
                    amount: 6_000,
                    period: 6,
                },
            ],
            keeper_cash: 20_000,
            collateral_lot: 100,
        }
    }

    /// The Showcase loan at four times the scale over twelve periods, with
    /// revenue in three installments and finer borrowing steps.
    pub fn stress() -> Self {
        Self {
            terms: Terms::standard(),
            deposits: vec![16_000, 12_000, 8_000, 3_000],
            depositor_cash: 8_000,
            periods: 12,
            collateral_price: 50,
            shock: Some(Shock { after: 6, drop: 20 }),
            borrower_cash: 4_000,
            collateral: 1_600,
            bill: Payment {
                amount: 32_000,
                period: 4,
            },
            revenue: vec![
                Payment {
                    amount: 10_000,
                    period: 5,
                },
                Payment {
                    amount: 12_000,
                    period: 7,
                },
                Payment {
                    amount: 18_000,
                    period: 9,
                },
            ],
            keeper_cash: 80_000,
            collateral_lot: 400,
        }
    }
}

fn wad() -> Q {
    Q::constant(WAD)
}

fn market(asset: Asset) -> Q {
    Q::balance(Role::Market, asset)
}

fn index() -> Q {
    market(Asset::BorrowIndex)
}

fn elapsed() -> Q {
    Q::balance(Role::Clock, Asset::ElapsedPeriod)
}

/// Outstanding debt valued rounded down: depositors are never credited with
/// interest the borrowers do not owe (each borrower's own debt rounds up).
fn debt_value() -> Q {
    Q::mul_div(
        market(Asset::DebtShareSupply),
        index(),
        wad(),
        Rounding::Floor,
    )
}

/// Depositors' assets: cash plus debt, less the reserves the market keeps.
fn total_assets() -> Q {
    (market(Asset::Cash) + debt_value()) - market(Asset::Reserves)
}

/// Cash the borrower owes now, rounded up.
fn owed() -> Q {
    owed_for(Q::balance(Role::Borrower, Asset::DebtShare))
}

fn owed_for(debt_shares: Q) -> Q {
    Q::mul_div(debt_shares, index(), wad(), Rounding::Ceil)
}

/// A share of the posted collateral's value, rounded down: the borrower
/// receives borrowing room and protection, so neither rounds in its favor.
fn collateral_limit(posted: Q, term: Asset) -> Q {
    Q::mul_div(
        posted * market(Asset::CollateralPrice),
        market(term),
        wad(),
        Rounding::Floor,
    )
}

fn posted() -> Q {
    Q::balance(Role::Borrower, Asset::PostedCollateral)
}

/// Borrowed share of the market's cash and debt, rounded up.
fn utilization() -> Q {
    let gross = Q::maximum(market(Asset::Cash) + debt_value(), Q::constant(1));
    Q::mul_div(debt_value(), wad(), gross, Rounding::Ceil)
}

/// Interest per period: the base rate, the slope below the kink up to the
/// kink, and the steeper slope beyond it. Owed amounts grow, so each part
/// rounds up.
fn rate_per_period() -> Q {
    let below = Q::mul_div(
        Q::minimum(utilization(), market(Asset::Kink)),
        market(Asset::SlopeBelowKink),
        wad(),
        Rounding::Ceil,
    );
    let above = Q::mul_div(
        Q::maximum(utilization(), market(Asset::Kink)) - market(Asset::Kink),
        market(Asset::SlopeAboveKink),
        wad(),
        Rounding::Ceil,
    );
    market(Asset::BaseRate) + (below + above)
}

fn index_growth() -> Q {
    Q::mul_div(index(), rate_per_period(), wad(), Rounding::Ceil)
}

/// The reserve share of this period's interest. It only moves value between
/// depositors and the market, so rounding it up keeps the dust in the market.
fn reserve_cut() -> Q {
    let debt_after = Q::mul_div(
        market(Asset::DebtShareSupply),
        index() + index_growth(),
        wad(),
        Rounding::Floor,
    );
    Q::mul_div(
        debt_after - debt_value(),
        market(Asset::ReserveFactor),
        wad(),
        Rounding::Ceil,
    )
}

fn one_unit(rate: Rate<Ontology>, name: ConditionPath) -> Rate<Ontology> {
    rate.condition(name, Q::units(), QuantityComparison::Equal, Q::constant(1))
}

fn market_rate() -> Rate<Ontology> {
    Rate::new().preserve(Role::Market, basket([(Asset::MarketIdentity, 1)]))
}

fn borrower_rate() -> Rate<Ontology> {
    market_rate()
        .preserve(Role::Borrower, basket([(Asset::CreditLine, 1)]))
        .distinct(Role::Market, Role::Borrower)
}

fn clock_bound(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.preserve(Role::Clock, basket([(Asset::ClockIdentity, 1)]))
}

fn deposit_rate() -> Rate<Ontology> {
    let minted = Q::mul_div(
        Q::units(),
        market(Asset::ShareSupply),
        total_assets(),
        Rounding::Floor,
    );
    market_rate()
        .consume(Role::Depositor, basket([(Asset::Cash, 1)]))
        .produce(Role::Market, basket([(Asset::Cash, 1)]))
        .produce_computed(Role::Depositor, Asset::Share, minted.clone())
        .produce_computed(Role::Market, Asset::ShareSupply, minted.clone())
        .condition(
            path!("mints-at-least-the-minimum-shares"),
            minted,
            QuantityComparison::GreaterThanOrEqual,
            Q::maximum(Q::parameter(path!("min-shares")), Q::constant(1)),
        )
        .distinct(Role::Market, Role::Depositor)
}

fn redeem_rate() -> Rate<Ontology> {
    let paid = Q::mul_div(
        Q::units(),
        total_assets(),
        market(Asset::ShareSupply),
        Rounding::Floor,
    );
    market_rate()
        .consume(Role::Depositor, basket([(Asset::Share, 1)]))
        .consume(Role::Market, basket([(Asset::ShareSupply, 1)]))
        .consume_computed(Role::Market, Asset::Cash, paid.clone())
        .produce_computed(Role::Depositor, Asset::Cash, paid.clone())
        .condition(
            path!("pays-at-least-the-minimum-cash"),
            paid,
            QuantityComparison::GreaterThanOrEqual,
            Q::parameter(path!("min-cash")),
        )
        .distinct(Role::Market, Role::Depositor)
}

fn donate_rate() -> Rate<Ontology> {
    market_rate()
        .consume(Role::Depositor, basket([(Asset::Cash, 1)]))
        .produce(Role::Market, basket([(Asset::Cash, 1)]))
        .distinct(Role::Market, Role::Depositor)
}

fn post_collateral_rate() -> Rate<Ontology> {
    borrower_rate()
        .consume(Role::Borrower, basket([(Asset::Collateral, 1)]))
        .produce(Role::Borrower, basket([(Asset::PostedCollateral, 1)]))
}

fn withdraw_collateral_rate() -> Rate<Ontology> {
    borrower_rate()
        .consume(Role::Borrower, basket([(Asset::PostedCollateral, 1)]))
        .produce(Role::Borrower, basket([(Asset::Collateral, 1)]))
        .condition(
            path!("the-remaining-collateral-covers-the-loan"),
            owed(),
            QuantityComparison::LessThanOrEqual,
            collateral_limit(posted() - Q::units(), Asset::LoanToValue),
        )
}

fn borrow_rate() -> Rate<Ontology> {
    let minted = Q::mul_div(Q::units(), wad(), index(), Rounding::Ceil);
    let owed_after = owed_for(Q::balance(Role::Borrower, Asset::DebtShare) + minted.clone());
    borrower_rate()
        .consume(Role::Market, basket([(Asset::Cash, 1)]))
        .produce(Role::Borrower, basket([(Asset::Cash, 1)]))
        .produce_computed(Role::Borrower, Asset::DebtShare, minted.clone())
        .produce_computed(Role::Market, Asset::DebtShareSupply, minted)
        .condition(
            path!("within-the-loan-to-value-limit"),
            owed_after,
            QuantityComparison::LessThanOrEqual,
            collateral_limit(posted(), Asset::LoanToValue),
        )
}

fn repay_rate() -> Rate<Ontology> {
    let retired = Q::mul_div(Q::units(), wad(), index(), Rounding::Floor);
    borrower_rate()
        .consume(Role::Borrower, basket([(Asset::Cash, 1)]))
        .produce(Role::Market, basket([(Asset::Cash, 1)]))
        .consume_computed(Role::Borrower, Asset::DebtShare, retired.clone())
        .consume_computed(Role::Market, Asset::DebtShareSupply, retired.clone())
        .condition(
            path!("retires-debt"),
            retired,
            QuantityComparison::GreaterThanOrEqual,
            Q::constant(1),
        )
}

fn repay_shares_rate() -> Rate<Ontology> {
    let paid = owed_for(Q::units());
    borrower_rate()
        .consume(Role::Borrower, basket([(Asset::DebtShare, 1)]))
        .consume(Role::Market, basket([(Asset::DebtShareSupply, 1)]))
        .consume_computed(Role::Borrower, Asset::Cash, paid.clone())
        .produce_computed(Role::Market, Asset::Cash, paid)
}

fn pay_bill_rate(bill: Payment) -> Rate<Ontology> {
    clock_bound(
        Rate::new()
            .preserve(Role::Borrower, basket([(Asset::CreditLine, 1)]))
            .consume(Role::Borrower, basket([(Asset::Cash, bill.amount)]))
            .consume(Role::Supplier, basket([(Asset::Invoice, 1)]))
            .produce(Role::Supplier, basket([(Asset::Cash, bill.amount)]))
            .produce(Role::Borrower, basket([(Asset::BillPaid, 1)])),
    )
    .condition(
        path!("paid-before-the-deadline"),
        elapsed() + Q::constant(1),
        QuantityComparison::LessThanOrEqual,
        Q::constant(bill.period),
    )
    .distinct(Role::Borrower, Role::Supplier)
}

fn collect_revenue_rate(installment: u8, payment: Payment) -> Rate<Ontology> {
    clock_bound(
        Rate::new()
            .preserve(Role::Borrower, basket([(Asset::CreditLine, 1)]))
            .consume(
                Role::Customer,
                basket([
                    (Asset::PurchaseOrder(installment), 1),
                    (Asset::Cash, payment.amount),
                ]),
            )
            .produce(Role::Borrower, basket([(Asset::Cash, payment.amount)])),
    )
    .condition(
        path!("the-installment-is-due"),
        elapsed(),
        QuantityComparison::GreaterThanOrEqual,
        Q::constant(payment.period),
    )
    .distinct(Role::Borrower, Role::Customer)
}

fn accrue_rate() -> Rate<Ontology> {
    let pending = Q::balance(Role::Oracle, Asset::PendingShock);
    let rate = clock_bound(borrower_rate())
        .preserve(Role::Oracle, basket([(Asset::OracleIdentity, 1)]))
        .consume(Role::Clock, basket([(Asset::Period, 1)]))
        .produce(Role::Clock, basket([(Asset::ElapsedPeriod, 1)]))
        .produce_computed(Role::Market, Asset::BorrowIndex, index_growth())
        .produce_computed(Role::Market, Asset::Reserves, reserve_cut())
        .condition(
            path!("the-loan-is-not-liquidatable"),
            owed(),
            QuantityComparison::LessThanOrEqual,
            collateral_limit(posted(), Asset::LiquidationThreshold),
        )
        // While the drop is pending, the clock cannot pass its period.
        .condition(
            path!("the-scheduled-price-drop-has-happened"),
            pending * (elapsed() + Q::constant(1)),
            QuantityComparison::LessThanOrEqual,
            Q::balance(Role::Oracle, Asset::ShockAfter),
        )
        .distinct(Role::Market, Role::Clock)
        .distinct(Role::Market, Role::Oracle)
        .distinct(Role::Borrower, Role::Clock)
        .distinct(Role::Borrower, Role::Oracle)
        .distinct(Role::Clock, Role::Oracle);
    one_unit(rate, path!("one-period-at-a-time"))
}

fn reprice_rate() -> Rate<Ontology> {
    let rate = clock_bound(market_rate())
        .preserve(Role::Oracle, basket([(Asset::OracleIdentity, 1)]))
        .consume(Role::Oracle, basket([(Asset::PendingShock, 1)]))
        .consume_computed(
            Role::Market,
            Asset::CollateralPrice,
            Q::balance(Role::Oracle, Asset::PriceDrop),
        )
        .condition(
            path!("at-the-scheduled-period"),
            elapsed(),
            QuantityComparison::Equal,
            Q::balance(Role::Oracle, Asset::ShockAfter),
        )
        .distinct(Role::Market, Role::Oracle)
        .distinct(Role::Market, Role::Clock)
        .distinct(Role::Clock, Role::Oracle);
    one_unit(rate, path!("one-price-drop"))
}

fn liquidate_rate() -> Rate<Ontology> {
    let seized = Q::minimum(
        posted(),
        Q::divide(
            Q::mul_div(
                owed(),
                wad() + market(Asset::LiquidationBonus),
                wad(),
                Rounding::Floor,
            ),
            market(Asset::CollateralPrice),
            Rounding::Floor,
        ),
    );
    let debt_shares = Q::balance(Role::Borrower, Asset::DebtShare);
    let rate = borrower_rate()
        .consume_computed(Role::Keeper, Asset::Cash, owed())
        .produce_computed(Role::Market, Asset::Cash, owed())
        .consume_computed(Role::Borrower, Asset::DebtShare, debt_shares.clone())
        .consume_computed(Role::Market, Asset::DebtShareSupply, debt_shares)
        .consume_computed(Role::Borrower, Asset::PostedCollateral, seized.clone())
        .produce_computed(Role::Keeper, Asset::Collateral, seized)
        .condition(
            path!("the-loan-is-past-its-liquidation-threshold"),
            owed(),
            QuantityComparison::GreaterThanOrEqual,
            collateral_limit(posted(), Asset::LiquidationThreshold) + Q::constant(1),
        )
        .distinct(Role::Market, Role::Keeper)
        .distinct(Role::Borrower, Role::Keeper);
    one_unit(rate, path!("one-liquidation"))
}

pub fn world(config: &Config) -> World {
    let deposited: u64 = config.deposits.iter().sum();
    let terms = config.terms;
    let customer: Basket<Asset> = (0_u8..)
        .zip(&config.revenue)
        .map(|(installment, _)| (Asset::PurchaseOrder(installment), Quantity::new(1)))
        .chain([(
            Asset::Cash,
            Quantity::new(config.revenue.iter().map(|payment| payment.amount).sum()),
        )])
        .collect();
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Market,
            Account::from(basket([
                (Asset::Cash, SEED_CASH + deposited),
                (Asset::LockedShare, LOCKED_SHARES),
                (Asset::ShareSupply, LOCKED_SHARES + deposited),
                (Asset::BorrowIndex, WAD),
                (Asset::CollateralPrice, config.collateral_price),
                (Asset::BaseRate, terms.base_rate),
                (Asset::Kink, terms.kink),
                (Asset::SlopeBelowKink, terms.slope_below_kink),
                (Asset::SlopeAboveKink, terms.slope_above_kink),
                (Asset::ReserveFactor, terms.reserve_factor),
                (Asset::LoanToValue, terms.loan_to_value),
                (Asset::LiquidationThreshold, terms.liquidation_threshold),
                (Asset::LiquidationBonus, terms.liquidation_bonus),
                (Asset::MarketIdentity, 1),
            ])),
        )
        .account(
            AccountId::Clock,
            Account::from(basket([
                (Asset::ClockIdentity, 1),
                (Asset::Period, config.periods),
            ])),
        )
        .account(
            AccountId::Oracle,
            Account::from(match config.shock {
                Some(shock) => basket([
                    (Asset::OracleIdentity, 1),
                    (Asset::PendingShock, 1),
                    (Asset::ShockAfter, shock.after),
                    (Asset::PriceDrop, shock.drop),
                ]),
                None => basket([(Asset::OracleIdentity, 1)]),
            }),
        )
        .account(
            AccountId::Borrower,
            Account::from(basket([
                (Asset::CreditLine, 1),
                (Asset::Cash, config.borrower_cash),
                (Asset::Collateral, config.collateral),
            ])),
        )
        .account(
            AccountId::Keeper,
            Account::from(basket([(Asset::Cash, config.keeper_cash)])),
        )
        .account(
            AccountId::Supplier,
            Account::from(basket([(Asset::Invoice, 1)])),
        )
        .account(AccountId::Customer, Account::from(customer));
    for (id, deposit) in (0_u8..).zip(&config.deposits) {
        builder = builder.account(
            AccountId::Depositor(id),
            Account::from(basket([
                (Asset::Cash, config.depositor_cash),
                (Asset::Share, *deposit),
            ])),
        );
    }
    builder = builder
        .rate(RateId::Deposit, deposit_rate())
        .rate(RateId::Redeem, redeem_rate())
        .rate(RateId::Donate, donate_rate())
        .rate(RateId::PostCollateral, post_collateral_rate())
        .rate(RateId::WithdrawCollateral, withdraw_collateral_rate())
        .rate(RateId::Borrow, borrow_rate())
        .rate(RateId::Repay, repay_rate())
        .rate(RateId::RepayShares, repay_shares_rate())
        .rate(RateId::PayBill, pay_bill_rate(config.bill))
        .rate(RateId::Accrue, accrue_rate())
        .rate(RateId::Reprice, reprice_rate())
        .rate(RateId::Liquidate, liquidate_rate());
    for (installment, payment) in (0_u8..).zip(&config.revenue) {
        builder = builder.rate(
            RateId::CollectRevenue(installment),
            collect_revenue_rate(installment, *payment),
        );
    }
    builder
        .invariant(LinearInvariant::new(path!("cash")).weight(Asset::Cash, 1))
        .invariant(
            LinearInvariant::new(path!("vault-shares"))
                .weight(Asset::Share, 1)
                .weight(Asset::LockedShare, 1)
                .weight(Asset::ShareSupply, -1),
        )
        .invariant(
            LinearInvariant::new(path!("debt-shares"))
                .weight(Asset::DebtShare, 1)
                .weight(Asset::DebtShareSupply, -1),
        )
        .invariant(
            LinearInvariant::new(path!("collateral"))
                .weight(Asset::Collateral, 1)
                .weight(Asset::PostedCollateral, 1),
        )
        .build()
        .expect("the lending model is well formed")
}

pub fn initial() -> World {
    world(&Config::micro())
}

pub fn initial_showcase() -> World {
    world(&Config::showcase())
}

pub fn initial_stress() -> World {
    world(&Config::stress())
}

/// A fresh market with only the genesis seed, where depositors hold cash.
pub fn genesis(config: &Config) -> World {
    world(&Config {
        deposits: vec![0; config.deposits.len()],
        ..config.clone()
    })
}

fn with_market(rate: RateId, units: u64) -> Action {
    Exchange::new(rate, Quantity::new(units)).bind(Role::Market, AccountId::Market)
}

fn for_borrower(rate: RateId, units: u64) -> Action {
    with_market(rate, units).bind(Role::Borrower, AccountId::Borrower)
}

pub fn deposit(depositor: u8, cash: u64, min_shares: u64) -> Action {
    with_market(RateId::Deposit, cash)
        .bind(Role::Depositor, AccountId::Depositor(depositor))
        .parameter(path!("min-shares"), Quantity::new(min_shares))
}

pub fn redeem(depositor: u8, shares: u64, min_cash: u64) -> Action {
    with_market(RateId::Redeem, shares)
        .bind(Role::Depositor, AccountId::Depositor(depositor))
        .parameter(path!("min-cash"), Quantity::new(min_cash))
}

pub fn donate(depositor: u8, cash: u64) -> Action {
    with_market(RateId::Donate, cash).bind(Role::Depositor, AccountId::Depositor(depositor))
}

pub fn post_collateral(units: u64) -> Action {
    for_borrower(RateId::PostCollateral, units)
}

pub fn withdraw_collateral(units: u64) -> Action {
    for_borrower(RateId::WithdrawCollateral, units)
}

pub fn borrow(cash: u64) -> Action {
    for_borrower(RateId::Borrow, cash)
}

pub fn repay(cash: u64) -> Action {
    for_borrower(RateId::Repay, cash)
}

pub fn repay_shares(debt_shares: u64) -> Action {
    for_borrower(RateId::RepayShares, debt_shares)
}

pub fn pay_bill() -> Action {
    Exchange::new(RateId::PayBill, Quantity::new(1))
        .bind(Role::Borrower, AccountId::Borrower)
        .bind(Role::Supplier, AccountId::Supplier)
        .bind(Role::Clock, AccountId::Clock)
}

pub fn collect_revenue(installment: u8) -> Action {
    Exchange::new(RateId::CollectRevenue(installment), Quantity::new(1))
        .bind(Role::Borrower, AccountId::Borrower)
        .bind(Role::Customer, AccountId::Customer)
        .bind(Role::Clock, AccountId::Clock)
}

pub fn accrue() -> Action {
    for_borrower(RateId::Accrue, 1)
        .bind(Role::Clock, AccountId::Clock)
        .bind(Role::Oracle, AccountId::Oracle)
}

pub fn reprice() -> Action {
    with_market(RateId::Reprice, 1)
        .bind(Role::Clock, AccountId::Clock)
        .bind(Role::Oracle, AccountId::Oracle)
}

pub fn liquidate() -> Action {
    for_borrower(RateId::Liquidate, 1).bind(Role::Keeper, AccountId::Keeper)
}

/// The bill is paid and every unit of collateral is free again, which the
/// withdrawal rule only allows once the loan is fully repaid.
pub fn goal(config: &Config) -> Goal<Ontology> {
    Goal::new().require(
        AccountId::Borrower,
        basket([(Asset::BillPaid, 1), (Asset::Collateral, config.collateral)]),
    )
}

/// The borrower's moves, then the keeper, oracle and clock.
///
/// Amounts and phases are search policy, not semantics. The bill is paid at
/// once, so the borrower borrows exactly its shortfall while it is unpaid;
/// afterwards it repays everything it owes, or all the cash it has, and takes
/// collateral back once the loan is closed. Collateral moves all at once or
/// in lots. Every candidate is still assessed by the kernel.
pub fn candidates(config: &Config) -> impl Fn(&World) -> Vec<Action> + '_ {
    move |world| {
        let borrower = |asset| balance(world, AccountId::Borrower, asset);
        let cash = borrower(Asset::Cash);
        let posted = borrower(Asset::PostedCollateral);
        let debt_shares = borrower(Asset::DebtShare);
        let bill_paid = borrower(Asset::BillPaid) > 0;
        let mut actions = Vec::new();
        if !bill_paid || debt_shares > 0 {
            for units in lots(borrower(Asset::Collateral), config.collateral_lot) {
                actions.push(post_collateral(units));
            }
        }
        if !bill_paid {
            if cash < config.bill.amount {
                actions.push(borrow(config.bill.amount - cash));
            }
            actions.push(pay_bill());
        } else if debt_shares > 0 {
            if u128::from(cash) >= debt_view(world) {
                actions.push(repay_shares(debt_shares));
            } else if cash > 0 {
                actions.push(repay(cash));
            }
        } else if posted > 0 {
            actions.push(withdraw_collateral(posted));
        }
        actions.extend(
            (0_u8..)
                .zip(&config.revenue)
                .map(|(id, _)| collect_revenue(id)),
        );
        actions.extend([liquidate(), reprice(), accrue()]);
        world.applicable(actions)
    }
}

/// All of `available`, and one lot when that is less.
fn lots(available: u64, lot: u64) -> Vec<u64> {
    match available {
        0 => Vec::new(),
        all if all > lot => vec![all, lot],
        all => vec![all],
    }
}

/// Fewest exchanges: blind to when cash is borrowed, so it pays for it.
pub fn fewest_steps(config: &Config) -> Option<Solution> {
    bfs(&world(config), &goal(config), candidates(config))
}

/// Least interest charged to the borrower.
pub fn least_interest(config: &Config) -> Option<Solution> {
    dijkstra(
        &world(config),
        &goal(config),
        candidates(config),
        interest_step,
    )
}

/// Interest one exchange charges the borrower: only closing a period does.
pub fn interest_step(before: &World, exchange: &Action, after: &World) -> u64 {
    if *exchange.rate() == RateId::Accrue {
        u64::try_from(debt_view(after).saturating_sub(debt_view(before)))
            .expect("interest per period fits the cash scale")
    } else {
        0
    }
}

/// Interest charged over a whole trace.
pub fn interest_charged(initial: &World, trace: &LoanTrace) -> Result<u64, LoanError> {
    let mut world = initial.clone();
    let mut total = 0;
    for exchange in trace.exchanges() {
        let before = world.clone();
        world.apply(exchange.clone()).map_err(Box::new)?;
        total += interest_step(&before, exchange, &world);
    }
    Ok(total)
}

/// Posts all collateral and borrows as much as the pool and the loan-to-value
/// limit allow in the first period, then lets the clock run. Interest at a
/// high utilization or the price drop pushes the loan past its threshold, and
/// the keeper liquidates it.
pub fn over_borrowed(config: &Config) -> Result<(World, LoanTrace), LoanError> {
    let mut world = world(config);
    let mut trace = Trace::new();
    let mut step = |world: &mut World, action: Action| -> Result<(), LoanError> {
        world.apply(action.clone()).map_err(Box::new)?;
        trace.push(action);
        Ok(())
    };
    step(&mut world, post_collateral(config.collateral))?;
    let room = borrow_limit_view(&world)
        .saturating_sub(debt_view(&world))
        .min(u128::from(market_balance(&world, Asset::Cash)));
    let room = u64::try_from(room).expect("room is bounded by market cash");
    if room > 0 && world.is_applicable(&borrow(room)) {
        step(&mut world, borrow(room))?;
    }
    step(&mut world, pay_bill())?;
    loop {
        if world.is_applicable(&liquidate()) {
            step(&mut world, liquidate())?;
            break;
        }
        if world.is_applicable(&reprice()) {
            step(&mut world, reprice())?;
        } else if world.is_applicable(&accrue()) {
            step(&mut world, accrue())?;
        } else {
            break;
        }
    }
    Ok((world, trace))
}

/// An attacker deposits one atom into a fresh market and donates heavily to
/// inflate the share price before a victim deposits, then redeems.
pub fn first_depositor_attack(config: &Config) -> Result<(World, LoanTrace), LoanError> {
    let mut world = genesis(config);
    let mut trace = Trace::new();
    let donation = config.depositor_cash - 1;
    for action in [
        deposit(0, 1, 1),
        donate(0, donation),
        deposit(1, config.depositor_cash, 1),
        redeem(0, 1, 0),
    ] {
        world.apply(action.clone()).map_err(Box::new)?;
        trace.push(action);
    }
    Ok((world, trace))
}

pub fn balance(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}

// Disposable u128 views, independent of the kernel's expression evaluator.
fn floor(left: u64, right: u64, denominator: u64) -> u128 {
    u128::from(left) * u128::from(right) / u128::from(denominator)
}

fn ceil(left: u64, right: u64, denominator: u64) -> u128 {
    (u128::from(left) * u128::from(right)).div_ceil(u128::from(denominator))
}

fn market_balance(world: &World, asset: Asset) -> u64 {
    balance(world, AccountId::Market, asset)
}

/// Outstanding debt valued rounded down.
pub fn debt_value_view(world: &World) -> u128 {
    floor(
        market_balance(world, Asset::DebtShareSupply),
        market_balance(world, Asset::BorrowIndex),
        WAD,
    )
}

/// Market cash plus outstanding debt, less reserves.
pub fn total_assets_view(world: &World) -> u128 {
    u128::from(market_balance(world, Asset::Cash)) + debt_value_view(world)
        - u128::from(market_balance(world, Asset::Reserves))
}

/// Cash the borrower owes now, rounded up.
pub fn debt_view(world: &World) -> u128 {
    ceil(
        balance(world, AccountId::Borrower, Asset::DebtShare),
        market_balance(world, Asset::BorrowIndex),
        WAD,
    )
}

/// Borrowed share of cash plus debt, scaled by `WAD`, rounded up.
pub fn utilization_view(world: &World) -> u128 {
    let debt = debt_value_view(world);
    let gross = (u128::from(market_balance(world, Asset::Cash)) + debt).max(1);
    (debt * u128::from(WAD)).div_ceil(gross)
}

/// Interest per period at the current utilization, scaled by `WAD`.
pub fn rate_view(world: &World) -> u128 {
    let terms = |asset| u128::from(market_balance(world, asset));
    let wad = u128::from(WAD);
    let utilization = utilization_view(world);
    let kink = terms(Asset::Kink);
    terms(Asset::BaseRate)
        + (utilization.min(kink) * terms(Asset::SlopeBelowKink)).div_ceil(wad)
        + ((utilization.max(kink) - kink) * terms(Asset::SlopeAboveKink)).div_ceil(wad)
}

/// A share of the posted collateral's value, rounded down.
fn limit_view(world: &World, term: Asset) -> u128 {
    let value = u128::from(balance(world, AccountId::Borrower, Asset::PostedCollateral))
        * u128::from(market_balance(world, Asset::CollateralPrice));
    value * u128::from(market_balance(world, term)) / u128::from(WAD)
}

pub fn borrow_limit_view(world: &World) -> u128 {
    limit_view(world, Asset::LoanToValue)
}

pub fn liquidation_limit_view(world: &World) -> u128 {
    limit_view(world, Asset::LiquidationThreshold)
}

/// Cash a depositor's shares redeem for now, rounded down.
pub fn share_value_view(world: &World, depositor: u8) -> u128 {
    let shares = u128::from(balance(
        world,
        AccountId::Depositor(depositor),
        Asset::Share,
    ));
    shares * total_assets_view(world) / u128::from(market_balance(world, Asset::ShareSupply))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::AssessmentStatus;
    use proptest::prelude::*;

    fn cash_total(world: &World) -> u128 {
        world
            .accounts()
            .map(|(id, _)| u128::from(balance(world, *id, Asset::Cash)))
            .sum()
    }

    fn assert_replays(config: &Config, solution: &Solution) -> World {
        let initial = world(config);
        let replayed = initial.replayed(solution.trace()).unwrap();
        assert!(replayed.matches(&goal(config)));
        assert_eq!(cash_total(&replayed), cash_total(&initial));
        replayed
    }

    #[test]
    fn least_interest_borrows_late_and_repays_early() {
        for config in [Config::micro(), Config::showcase()] {
            let fewest = fewest_steps(&config).unwrap();
            let cheapest = least_interest(&config).unwrap();
            assert_replays(&config, &fewest);
            let after = assert_replays(&config, &cheapest);
            let initial = world(&config);
            let fewest_interest = interest_charged(&initial, fewest.trace()).unwrap();
            assert_eq!(
                cheapest.cost(),
                interest_charged(&initial, cheapest.trace()).unwrap()
            );
            assert!(cheapest.cost() < fewest_interest, "{config:?}");
            assert!(fewest.trace().exchanges().len() <= cheapest.trace().exchanges().len());
            assert_eq!(balance(&after, AccountId::Borrower, Asset::DebtShare), 0);
            assert_eq!(
                balance(&after, AccountId::Market, Asset::DebtShareSupply),
                0
            );
        }
    }

    #[test]
    fn depositors_and_reserves_split_exactly_the_interest_paid() {
        let config = Config::showcase();
        let initial = world(&config);
        let solution = least_interest(&config).unwrap();
        let after = initial.replayed(solution.trace()).unwrap();
        let market_gain = u128::from(market_balance(&after, Asset::Cash))
            - u128::from(market_balance(&initial, Asset::Cash));
        // With the loan repaid, every unit the market gained came from the
        // borrower, and the reserves are a part of it, never more.
        let borrower_paid = u128::from(config.borrower_cash)
            + config
                .revenue
                .iter()
                .map(|p| u128::from(p.amount))
                .sum::<u128>()
            - u128::from(config.bill.amount)
            - u128::from(balance(&after, AccountId::Borrower, Asset::Cash));
        assert_eq!(market_gain, borrower_paid);
        let reserves = u128::from(market_balance(&after, Asset::Reserves));
        assert!(reserves > 0 && reserves < market_gain);
        assert!(u128::from(solution.cost()) <= borrower_paid);
        // Depositors' shares grew by the interest less the reserve cut.
        let depositors_before = total_assets_view(&initial);
        assert_eq!(
            total_assets_view(&after),
            depositors_before + market_gain - reserves
        );
    }

    #[test]
    fn utilization_drives_the_kinked_rate() {
        let config = Config::showcase();
        let mut world = world(&config);
        assert_eq!(rate_view(&world), u128::from(config.terms.base_rate));
        world.apply(post_collateral(config.collateral)).unwrap();
        let mut previous_slope: u128 = 0;
        let mut previous_rate = rate_view(&world);
        for _ in 0..9 {
            world.apply(borrow(1_000)).unwrap();
            let rate = rate_view(&world);
            let slope = rate - previous_rate;
            assert!(slope >= previous_slope.saturating_sub(1));
            previous_slope = slope;
            previous_rate = rate;
        }
        // 9,000 of 10,000 lent: past the 80% kink, the steep slope applies.
        assert_eq!(utilization_view(&world), u128::from(basis_points(9_000)));
        assert_eq!(rate_view(&world), u128::from(basis_points(20 + 200 + 500)));
        // The kernel accrues exactly the rate the view computes.
        let index = market_balance(&world, Asset::BorrowIndex);
        world.apply(accrue()).unwrap();
        assert_eq!(
            u128::from(market_balance(&world, Asset::BorrowIndex)),
            u128::from(index) + ceil(index, basis_points(720), WAD)
        );
    }

    #[test]
    fn borrowing_and_withdrawal_respect_the_loan_to_value_limit() {
        let config = Config::micro();
        let mut world = world(&config);
        let refused = world.assess(&borrow(1));
        assert!(refused.issues().iter().any(|issue| matches!(
            issue,
            ApplyError::RateConditionFailed { condition, .. }
                if *condition == path!("within-the-loan-to-value-limit")
        )));
        world.apply(post_collateral(40)).unwrap();
        // 40 units at 100 with a 60% limit.
        assert_eq!(borrow_limit_view(&world), 2_400);
        assert!(world.is_applicable(&borrow(2_400)));
        assert!(!world.is_applicable(&borrow(2_401)));
        world.apply(borrow(1_200)).unwrap();
        // 20 units still cover 1,200 at 60%; 21 withdrawn would not.
        assert!(world.is_applicable(&withdraw_collateral(20)));
        assert!(!world.is_applicable(&withdraw_collateral(21)));
        let debt_shares = balance(&world, AccountId::Borrower, Asset::DebtShare);
        world.apply(repay_shares(debt_shares)).unwrap();
        assert!(world.is_applicable(&withdraw_collateral(40)));
    }

    #[test]
    fn the_price_drop_happens_on_schedule_and_once() {
        let config = Config::micro();
        let mut world = world(&config);
        assert!(!world.is_applicable(&reprice()));
        world.apply(accrue()).unwrap();
        world.apply(accrue()).unwrap();
        // The clock cannot pass the scheduled period before the drop.
        assert!(!world.is_applicable(&accrue()));
        world.apply(reprice()).unwrap();
        assert_eq!(market_balance(&world, Asset::CollateralPrice), 70);
        assert!(!world.is_applicable(&reprice()));
        world.apply(accrue()).unwrap();
    }

    #[test]
    fn over_borrowing_ends_in_liquidation_with_a_bonus() {
        for config in [Config::micro(), Config::showcase()] {
            let initial = world(&config);
            let (after, trace) = over_borrowed(&config).unwrap();
            let last = trace.exchanges().last().unwrap();
            assert_eq!(*last.rate(), RateId::Liquidate);
            assert!(!after.matches(&goal(&config)));
            assert_eq!(balance(&after, AccountId::Borrower, Asset::DebtShare), 0);
            assert_eq!(cash_total(&after), cash_total(&initial));
            // The keeper paid the debt and received more collateral value
            // than it paid, up to what was posted.
            let before = initial.replayed(&{
                let mut prefix = Trace::new();
                for exchange in &trace.exchanges()[..trace.exchanges().len() - 1] {
                    prefix.push(exchange.clone());
                }
                prefix
            });
            let before = before.unwrap();
            let paid = debt_view(&before);
            let seized = u128::from(balance(&after, AccountId::Keeper, Asset::Collateral));
            let price = u128::from(market_balance(&after, Asset::CollateralPrice));
            assert!(seized * price > paid || seized == u128::from(config.collateral));
            assert!(paid > liquidation_limit_view(&before));
        }
    }

    #[test]
    fn a_safe_loan_cannot_be_liquidated_and_an_unsafe_one_stops_the_clock() {
        let config = Config::micro();
        let mut world = world(&config);
        world.apply(post_collateral(34)).unwrap();
        world.apply(borrow(2_000)).unwrap();
        assert!(!world.is_applicable(&liquidate()));
        world.apply(pay_bill()).unwrap();
        world.apply(accrue()).unwrap();
        world.apply(accrue()).unwrap();
        world.apply(reprice()).unwrap();
        // 34 units at 70 with a 75% threshold cover 1,785; more is owed.
        assert_eq!(liquidation_limit_view(&world), 1_785);
        assert!(debt_view(&world) > 1_785);
        let stalled = world.assess(&accrue());
        assert_eq!(stalled.status(), AssessmentStatus::Invalid);
        assert!(stalled.issues().iter().any(|issue| matches!(
            issue,
            ApplyError::RateConditionFailed { condition, .. }
                if *condition == path!("the-loan-is-not-liquidatable")
        )));
        // Posting the remaining collateral cures it.
        let mut cured = world.clone();
        cured.apply(post_collateral(6)).unwrap();
        assert!(cured.is_applicable(&accrue()));
        assert!(!cured.is_applicable(&liquidate()));
        world.apply(liquidate()).unwrap();
        assert!(world.is_applicable(&accrue()));
    }

    #[test]
    fn locked_shares_defeat_a_first_depositor_inflation_attack() {
        let config = Config::showcase();
        let (after, _) = first_depositor_attack(&config).unwrap();
        let victim_shares = balance(&after, AccountId::Depositor(1), Asset::Share);
        assert!(victim_shares > 0);
        let price = total_assets_view(&after)
            .div_ceil(u128::from(market_balance(&after, Asset::ShareSupply)));
        assert!(u128::from(config.depositor_cash) - share_value_view(&after, 1) < price);
        // The donation accrues to every share, overwhelmingly the locked ones.
        let attacker = balance(&after, AccountId::Depositor(0), Asset::Cash);
        assert!(
            attacker < config.depositor_cash / 4,
            "attacker kept {attacker}"
        );
    }

    #[test]
    fn slippage_bounds_are_enforced() {
        let config = Config::micro();
        let world = genesis(&config);
        assert!(world.is_applicable(&deposit(0, 1_000, 1_000)));
        assert!(!world.is_applicable(&deposit(0, 1_000, 1_001)));
        let mut inflated = genesis(&config);
        inflated.apply(donate(1, 1_999)).unwrap();
        assert!(!inflated.is_applicable(&deposit(0, 1, 0)));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn no_round_trip_creates_value(
            posted in 1_u64..=40,
            borrowed in 0_u64..2_400,
            periods in 0_u64..=2,
            amount in 1_u64..2_000,
        ) {
            let config = Config { shock: None, ..Config::micro() };
            let mut world = world(&config);
            world.apply(post_collateral(posted)).unwrap();
            let borrowed = borrowed.min(u64::try_from(borrow_limit_view(&world)).unwrap());
            if borrowed > 0 {
                world.apply(borrow(borrowed)).unwrap();
            }
            for _ in 0..periods {
                if world.apply(accrue()).is_err() {
                    break;
                }
            }

            // Deposit then redeem never returns more than was deposited.
            let mut vault = world.clone();
            let start = balance(&vault, AccountId::Depositor(0), Asset::Cash);
            let shares_before = balance(&vault, AccountId::Depositor(0), Asset::Share);
            if vault.apply(deposit(0, amount, 1)).is_ok() {
                let minted = balance(&vault, AccountId::Depositor(0), Asset::Share) - shares_before;
                vault.apply(redeem(0, minted, 0)).unwrap();
                prop_assert!(balance(&vault, AccountId::Depositor(0), Asset::Cash) <= start);
            }

            // Borrow then repay everything never costs less than was borrowed.
            let mut credit = world.clone();
            let start = balance(&credit, AccountId::Borrower, Asset::Cash);
            let debt_before = balance(&credit, AccountId::Borrower, Asset::DebtShare);
            if credit.apply(borrow(amount)).is_ok() {
                let shares = balance(&credit, AccountId::Borrower, Asset::DebtShare) - debt_before;
                credit.apply(repay_shares(shares)).unwrap();
                prop_assert!(balance(&credit, AccountId::Borrower, Asset::Cash) <= start);
            }
        }
    }
}
