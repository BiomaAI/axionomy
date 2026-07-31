use axionomy_problems::bridge::{self, AgentId};

fn main() {
    let initial = bridge::initial();
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

    println!(
        "Bridge: BFS {} exchanges, first-come {}, auction {}",
        generic.trace().exchanges().len(),
        first_come.exchanges().len(),
        auction.exchanges().len(),
    );
}
