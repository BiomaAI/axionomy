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
