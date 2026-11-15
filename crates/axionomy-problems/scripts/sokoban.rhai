// Sokoban: the exchanges the player may try. Moves and pushes from the
// player's cell, left, right, up and down; finishing; latching a panel. Only
// those that apply, in that order. Each is its rate's exchange, which fixes
// every role to the cell or puzzle it names.

fn number(text, prefix) {
    parse_int(text.sub_string(prefix.len()))
}

// The board's width and height, as the puzzle account records them.
fn dimensions(state) {
    let width = 0;
    let height = 0;
    for asset in state.balances("success").keys() {
        if asset.starts_with("board-width/") { width = number(asset, "board-width/"); }
        if asset.starts_with("board-height/") { height = number(asset, "board-height/"); }
    }
    [width, height]
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

fn candidates(state) {
    let size = dimensions(state);
    let width = size[0];
    let height = size[1];
    // The player's cell, and each crate with the cell it is on.
    let player = -1;
    let crates = [];
    for account in state.accounts("cell/") {
        let cell = number(account, "cell/");
        let holds = state.balances(account);
        if "player" in holds { player = cell; }
        for asset in holds.keys() {
            if asset.starts_with("crate/") { crates.push([number(asset, "crate/"), cell]); }
        }
    }
    if player < 0 { return []; }

    let x = player % width;
    let y = player / width;
    let neighbours = [];
    if x > 0 { neighbours.push(player - 1); }
    if x + 1 < width { neighbours.push(player + 1); }
    if y > 0 { neighbours.push(player - width); }
    if y + 1 < height { neighbours.push(player + width); }

    let proposals = [];
    for next in neighbours {
        if "empty" in state.balances(`cell/${next}`) {
            proposals.push(state.exchange_of(`move/${player}/${next}`));
            continue;
        }
        let pushed = crates.filter(|pair| pair[1] == next);
        if pushed.is_empty() { continue; }
        let to_x = (next % width) * 2 - x;
        let to_y = (next / width) * 2 - y;
        if to_x < 0 || to_y < 0 || to_x >= width || to_y >= height { continue; }
        let to = to_y * width + to_x;
        if "empty" in state.balances(`cell/${to}`) {
            proposals.push(state.exchange_of(`push/${player}/${next}/${to}/${pushed[0][0]}`));
        }
    }
    proposals.push(state.exchange_of("finish"));
    proposals.filter(|proposal| state.applicable(proposal)) + state.moves("latch")
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
