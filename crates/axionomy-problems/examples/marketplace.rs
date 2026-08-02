mod support;

use axionomy_problems::marketplace::{self, Assessment, Asset};
use tracing::{debug, info};

fn operational_cost(assessment: &Assessment) -> u64 {
    assessment
        .shortfalls()
        .iter()
        .flat_map(|shortfall| shortfall.missing().iter())
        .map(|(asset, quantity)| {
            let weight = match asset {
                Asset::Money => 1,
                Asset::ShippingCapacity => 50,
                Asset::Item(_) => 100,
                _ => 1_000,
            };
            weight * quantity.get()
        })
        .sum()
}

fn main() {
    support::init(
        "Marketplace",
        "Assess buyer, seller, and carrier combinations before six-account settlement.",
    );
    let market = marketplace::initial();
    info!(
        accounts = market.accounts().count(),
        rates = market.rate_ids().count(),
        "encoded economy ready"
    );
    let candidate_count = marketplace::candidates(&market).len();
    let exact = marketplace::exact_matches(&market);
    let near = marketplace::rank_near_matches(&market, operational_cost);
    info!(
        candidates = candidate_count,
        exact_matches = exact.len(),
        near_matches = near.len(),
        "candidate exchanges assessed"
    );

    let closest = near.first().expect("the bounded market has near matches");
    assert_eq!(closest.assessment().shortfalls().len(), 1);
    info!(
        candidate = ?closest.candidate(),
        missing_accounts = closest.assessment().shortfalls().len(),
        operational_cost = operational_cost(closest.assessment()),
        "closest infeasible match ranked by caller policy"
    );
    debug!(
        assessment = ?closest.assessment(),
        "complete near-match assessment"
    );

    let settlement = exact.first().expect("the market has exact matches").clone();
    let projected_accounts = market
        .assess(&settlement)
        .projected_deltas()
        .expect("an exact match projects its effects")
        .len();
    let mut settlement_branch = market.fork();
    let receipt = settlement_branch
        .apply(settlement)
        .expect("the exact match settles");

    assert_eq!(receipt.deltas().len(), projected_accounts);

    info!(
        touched_accounts = receipt.deltas().len(),
        projected_accounts, "one exact match settled atomically on an isolated branch"
    );
    debug!(receipt = ?receipt, "accepted settlement receipt");

    let clearing = marketplace::clear_market(&market);
    let cleared = market
        .replayed(clearing.trace())
        .expect("the clearing trace must replay");
    assert!(cleared.matches(&marketplace::goal()));
    info!(
        settlements = clearing.settled_orders(),
        gross_value = clearing.gross_value(),
        exchanges = clearing.trace().exchanges().len(),
        goal_verified = true,
        "compatible multi-order clearing selected and replayed"
    );
    debug!(trace = ?clearing.trace().exchanges(), "clearing settlement trace");
}
