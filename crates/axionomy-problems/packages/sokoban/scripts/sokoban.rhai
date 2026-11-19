// Sokoban: what a search reads beyond the moves, which are the rates'
// instances that apply (every rate fixes each role to the cell or puzzle
// it names): A*'s lower bound, and the cost of a step.

fn number(text, prefix) {
    parse_int(text.sub_string(prefix.len()))
}

// The board's width and height, the economy's constants.
fn dimensions(state) {
    [state.constant("board-width").to_int(), state.constant("board-height").to_int()]
}

// The goal cells: the cells finishing fixes its goal roles to.
fn goals(state) {
    let finish = state.exchange_of("finish");
    let goals = [];
    for role in state.roles("finish") {
        if role.starts_with("goal/") { goals.push(number(finish.account(role), "cell/")); }
    }
    goals
}

// A*'s lower bound: each crate's distance to its nearest goal cell, in
// moves along the grid.
fn crate_goal_distance(state) {
    let width = dimensions(state)[0];
    let crates = [];
    for account in state.accounts("cell/") {
        for asset in state.balances(account).keys() {
            if asset.starts_with("crate/") { crates.push(number(account, "cell/")); }
        }
    }
    let goals = goals(state);
    let total = 0;
    for crate_cell in crates {
        let nearest = ();
        for goal in goals {
            let distance = abs(crate_cell % width - goal % width) + abs(crate_cell / width - goal / width);
            if nearest == () || distance < nearest { nearest = distance; }
        }
        if nearest != () { total += nearest; }
    }
    total
}

// Every exchange costs one.
fn step(before, exchange, after) { 1 }
