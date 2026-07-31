mod support;

use axionomy_problems::bridge::{self, AgentId};
use tracing::{debug, info};

fn main() {
    support::init(
        "Bridge",
        "Compare generic search, first-come allocation, and atomic auction resolution.",
    );
    let initial = bridge::initial();
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        "encoded economy ready"
    );
    let generic = bridge::solve(&initial).expect("both agents can cross");
    let first_come =
        bridge::first_come_proposal(AgentId::B).expect("first-come mechanism is feasible");
    let auction = bridge::auction_proposal(2, 1).expect("auction mechanism is feasible");
    let generic_world = initial
        .replayed(generic.trace())
        .expect("BFS must emit core-valid exchanges");
    let first_come_world = initial
        .replayed(&first_come)
        .expect("the first-come proposal must pass the core");
    let auction_world = initial
        .replayed(&auction)
        .expect("the auction proposal must pass the core");

    assert!(generic_world.matches(&bridge::goal()));
    assert!(first_come_world.matches(&bridge::goal()));
    assert!(auction_world.matches(&bridge::goal()));

    info!(
        mechanism = "BFS",
        exchanges = generic.trace().exchanges().len(),
        expanded = generic.expanded(),
        goal_verified = true,
        "proposal replayed"
    );
    info!(
        mechanism = "first-come",
        first = ?AgentId::B,
        exchanges = first_come.exchanges().len(),
        goal_verified = true,
        "proposal replayed"
    );
    info!(
        mechanism = "auction",
        bid_a = 2,
        bid_b = 1,
        exchanges = auction.exchanges().len(),
        goal_verified = true,
        "proposal replayed"
    );
    debug!(
        mechanism = "BFS",
        trace = ?generic.trace().exchanges(),
        "accepted exchange trace"
    );
    debug!(
        mechanism = "first-come",
        trace = ?first_come.exchanges(),
        "accepted exchange trace"
    );
    debug!(
        mechanism = "auction",
        trace = ?auction.exchanges(),
        "accepted exchange trace"
    );
}
