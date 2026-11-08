mod support;

use axionomy_problems::lending::{self, AccountId, Asset, Config, RateId, balance};
use tracing::{debug, info};

fn main() {
    support::init(
        "Lending",
        "Finance a bill before revenue arrives with a collateralized loan whose every division rounds against the caller.",
    );
    let config = Config::showcase();
    let initial = lending::world(&config);
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        periods = config.periods,
        bill = config.bill.amount,
        bill_due_before_period = config.bill.period + 1,
        installments = config.revenue.len(),
        "encoded lending market ready"
    );

    let cheapest = lending::least_interest(&config).expect("the showcase loan is feasible");
    let fewest = lending::fewest_steps(&config).expect("the showcase loan is feasible");
    for (label, solution) in [("least interest", &cheapest), ("fewest steps", &fewest)] {
        let world = initial
            .replayed(solution.trace())
            .expect("the plan must pass the core on replay");
        assert!(world.matches(&lending::goal(&config)));
        let interest =
            lending::interest_charged(&initial, solution.trace()).expect("the plan replays");
        let borrowed_in = solution
            .trace()
            .exchanges()
            .iter()
            .take_while(|exchange| *exchange.rate() != RateId::Borrow)
            .filter(|exchange| *exchange.rate() == RateId::Accrue)
            .count()
            + 1;
        info!(
            plan = label,
            exchanges = solution.trace().exchanges().len(),
            borrowed_in_period = borrowed_in,
            interest,
            reserves = balance(&world, AccountId::Market, Asset::Reserves),
            "loan repaid and collateral returned"
        );
        debug!(trace = ?solution.trace().exchanges(), "accepted exchange trace");
    }

    let (liquidated, trace) = lending::over_borrowed(&config).expect("the scripted loan replays");
    info!(
        exchanges = trace.exchanges().len(),
        keeper_collateral = balance(&liquidated, AccountId::Keeper, Asset::Collateral),
        borrower_collateral_left =
            balance(&liquidated, AccountId::Borrower, Asset::PostedCollateral),
        "over-borrowed loan liquidated with a bonus"
    );
}
