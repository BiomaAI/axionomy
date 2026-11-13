// Maze: how far the agent is from the goal, as the world records it for the
// node the agent is at. A* reads it as a lower bound on the energy left.

fn heuristic(state) {
    for asset in state.balances("agent").keys() {
        if asset.starts_with("at/") {
            return state.balance("world", "distance/" + asset.sub_string(3)).to_int();
        }
    }
    0
}

// Every rate the economy has, bound to the agent and the world, that applies.
fn candidates(state) {
    // A closure may not borrow `state` while a chain started on it holds it.
    let rates = state.rates("");
    rates
        .map(|rate| exchange(rate, #{ actor: "agent", environment: "world" }, 1))
        .filter(|proposal| state.applicable(proposal))
}

// The energy a move spends.
fn energy(before, exchange, after) {
    after.balance("agent", "spent-energy") - before.balance("agent", "spent-energy")
}
