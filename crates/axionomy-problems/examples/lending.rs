mod support;

use axionomy_problems::lending::{
    self, AccountId, Asset, Config, LOCKED_SHARES, SEED_CASH, balance,
};
use tracing::{debug, info};

fn main() {
    support::init(
        "Lending",
        "Accrue interest through a borrow index with every division rounded against the caller.",
    );
    let config = Config::micro();
    let initial = lending::world(&config);
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        locked_shares = LOCKED_SHARES,
        seed_cash = SEED_CASH,
        periods = config.periods,
        rate_per_period = "1%",
        "encoded lending market ready"
    );

    let (world, trace) = lending::lending_cycle(&config).expect("the micro cycle is feasible");
    let replayed = initial
        .replayed(&trace)
        .expect("the cycle must pass the core on replay");
    assert_eq!(replayed.state_key(), world.state_key());
    assert!(world.matches(&lending::cycle_goal(&config)));

    let (borrower_start, limit) = config.borrowers[0];
    let repaid =
        borrower_start + limit * 4 / 5 - balance(&world, AccountId::Borrower(0), Asset::Cash);
    info!(
        borrowed = limit * 4 / 5,
        repaid,
        interest = repaid - limit * 4 / 5,
        "borrower repaid compound interest (debt rounded up each period)"
    );
    for depositor in [0_u8, 1] {
        let start = config.depositors[usize::from(depositor)];
        let end = balance(&world, AccountId::Depositor(depositor), Asset::Cash);
        info!(
            depositor,
            deposited = start / 2,
            earned = end - start,
            "depositor redeemed (payouts rounded down)"
        );
    }
    info!(
        market_cash = balance(&world, AccountId::Market, Asset::Cash),
        locked_shares = balance(&world, AccountId::Market, Asset::LockedShare),
        exchanges = trace.exchanges().len(),
        replay_verified = true,
        "rounding dust and the locked stake stayed with the market"
    );
    debug!(trace = ?trace.exchanges(), "accepted exchange trace");
}
