// Sokoban: the exchanges the player may try. Moves and pushes from the
// player's cell, left, right, up and down; finishing once every crate rests on
// a goal; latching a panel the player stands on. Only those that apply, in
// that order.

fn number(text, prefix) {
    parse_int(text.sub_string(prefix.len()))
}

fn candidates(state) {
    let width = 0;
    let height = 0;
    for asset in state.balances("success").keys() {
        if asset.starts_with("board-width/") { width = number(asset, "board-width/"); }
        if asset.starts_with("board-height/") { height = number(asset, "board-height/"); }
    }
    // The player's cell, and each crate with the cell it is on.
    let player = -1;
    let crates = [];
    for account in state.accounts("cell/") {
        let cell = number(account, "cell/");
        let holds = state.balances(account);
        if "floor" in holds && "player" in holds { player = cell; }
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
            proposals.push(exchange(`move/${player}/${next}`,
                #{ puzzle: "success", from: `cell/${player}`, to: `cell/${next}` }, 1));
            continue;
        }
        let pushed = crates.filter(|pair| pair[1] == next);
        if pushed.is_empty() { continue; }
        let to_x = (next % width) * 2 - x;
        let to_y = (next / width) * 2 - y;
        if to_x < 0 || to_y < 0 || to_x >= width || to_y >= height { continue; }
        let to = to_y * width + to_x;
        if "empty" in state.balances(`cell/${to}`) {
            proposals.push(exchange(`push/${player}/${next}/${to}/${pushed[0][0]}`,
                #{ puzzle: "success", from: `cell/${player}`, middle: `cell/${next}`, to: `cell/${to}` }, 1));
        }
    }
    if !crates.is_empty() && crates.all(|pair| "goal-cell" in state.balances(`cell/${pair[1]}`)) {
        let bindings = #{ puzzle: "success" };
        for pair in crates { bindings[`goal/${pair[0]}`] = `cell/${pair[1]}`; }
        proposals.push(exchange("finish", bindings, 1));
    }
    for circuit in 0..2 {
        if `panel/${circuit}` in state.balances(`cell/${player}`) {
            proposals.push(exchange(`latch/${circuit}/${player}`,
                #{ puzzle: "success", from: `cell/${player}` }, 1));
        }
    }
    proposals.filter(|proposal| state.applicable(proposal))
}

// A*'s lower bound: each crate's distance to its nearest goal cell, in
// moves along the grid.
fn crate_goal_distance(state) {
    let width = 0;
    for asset in state.balances("success").keys() {
        if asset.starts_with("board-width/") { width = number(asset, "board-width/"); }
    }
    let crates = [];
    let goals = [];
    for account in state.accounts("cell/") {
        let holds = state.balances(account);
        if !("floor" in holds) { continue; }
        let cell = number(account, "cell/");
        if "goal-cell" in holds { goals.push(cell); }
        for asset in holds.keys() {
            if asset.starts_with("crate/") { crates.push(cell); }
        }
    }
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
