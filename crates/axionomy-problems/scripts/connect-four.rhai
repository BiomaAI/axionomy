// Connect Four: Red and Yellow take turns dropping a piece into a column of
// a board, the compact 4×4 one or the standard 7×6 one, where it lands on
// the column's height. A drop completing a line of four wins at once, and a
// full board without one is drawn. Every move is one of the economy's rates'
// instances, and the game counts each player's pieces in every line of four
// cells, named by its direction, its rightmost column and its top row.
//
// Red searches by tree search; Yellow by the same, by dropping nearest the
// center, or by minimax over the board's worth, which `evaluate` scores from
// those counts.

// The board's columns, and its rows: the accounts of its columns, and of
// its first column's cells.
fn board_width(state) {
    let width = 0;
    while state.has_account(`column/${width}`) { width += 1; }
    width
}
fn board_height(state) {
    let height = 0;
    while state.has_account(`cell/0/${height}`) { height += 1; }
    height
}

// The player to drop a piece, if any.
fn to_move(state) {
    for player in ["red", "yellow"] {
        if state.balance("game", `turn/${player}`) > 0 { return player; }
    }
    ()
}

// The lines of four that lie on the board, each as its direction, rightmost
// column and top row name it: those along a row, up a column, and rising
// and falling to the right.
fn lines(width, height) {
    let found = [];
    for column in 3..width {
        for row in 0..height { found.push(`across/${column}/${row}`); }
    }
    for column in 0..width {
        for row in 3..height { found.push(`up/${column}/${row}`); }
    }
    for column in 3..width {
        for row in 3..height {
            found.push(`rising/${column}/${row}`);
            found.push(`falling/${column}/${row}`);
        }
    }
    found
}

// The pieces `player` holds in `line`, as the game counts them.
fn held(state, player, line) { state.balance("game", `line/${player}/${line}`).to_int() }

// What the searches try: at each column with room, the drop landing on its
// height, which passes the turn or wins; the draw once every column is
// full. Each is its rate's instance, as `state.exchange_of` binds it; of the
// instances the economy has, only these may apply, and only those that do
// are kept.
fn drops(state) {
    let player = to_move(state);
    if player == () { return []; }
    let height = board_height(state);
    let proposals = [];
    for column in 0..board_width(state) {
        let row = state.balance(`column/${column}`, "height").to_int();
        if row < height {
            proposals.push(state.exchange_of(`drop/${player}/${column}/${row}`));
            proposals.push(state.exchange_of(`win/${player}/${column}/${row}`));
        }
    }
    if proposals.is_empty() { proposals.push(state.exchange_of(`draw/${player}`)); }
    proposals.filter(|proposal| state.applicable(proposal))
}

// Who chooses each turn: Red while Red is to drop, and Yellow otherwise.
fn mover(state) { if state.balance("game", "turn/red") > 0 { 0 } else { 1 } }

// Each player's payoff where the game is over: one for a win, a half for a
// draw, nothing for a loss.
fn outcome(state) {
    if state.balance("result", "winner/red") > 0 { return [1.0, 0.0]; }
    if state.balance("result", "winner/yellow") > 0 { return [0.0, 1.0]; }
    if state.balance("result", "draw") > 0 { return [0.5, 0.5]; }
    ()
}

// The board's worth to Red: a hundred thousand for Red's win, as much less
// for Yellow's, nothing for a draw, and otherwise what the lines of four
// only one player holds pieces in are worth, Red's less Yellow's. A line
// holding one, two or three of a player's pieces is worth 1, 8 or 60.
fn evaluate(state) {
    if state.balance("result", "winner/red") > 0 { return 100000; }
    if state.balance("result", "winner/yellow") > 0 { return -100000; }
    if state.balance("result", "draw") > 0 { return 0; }
    let worth = [0, 1, 8, 60];
    let total = 0;
    for line in lines(board_width(state), board_height(state)) {
        let red = held(state, "red", line);
        let yellow = held(state, "yellow", line);
        if yellow == 0 {
            total += worth[red];
        } else if red == 0 {
            total -= worth[yellow];
        }
    }
    total
}

// Each player's expected payoff where a simulation stops short of the end:
// a half, moved toward the player the board's worth favours.
fn estimate(state) {
    let over = outcome(state);
    if over != () { return over; }
    let worth = evaluate(state).to_float();
    let red = 0.5 + 0.45 * worth / (abs(worth) + 120.0);
    [red, 1.0 - red]
}

// Yellow's move as an opponent: of the drops the economy offers, the one
// nearest the center, of equals the last; a draw where it is the only move.
fn center_opponent(state, random) {
    let width = board_width(state);
    let best = ();
    let most = 0;
    for action in state.moves("") {
        let column = action.account("column");
        if column == () { return action; }
        let worth = -abs(parse_int(column.split("/")[1]) * 2 - width + 1);
        if best == () || worth >= most {
            best = action;
            most = worth;
        }
    }
    best
}

// ---- Objectives

// A player's value where the game ends, in tenths: ten for a win, five for
// a draw, none for a loss.
fn tenths(state, player) {
    if state.balance("result", `winner/${player}`) > 0 { return 10; }
    if state.balance("result", "draw") > 0 { return 5; }
    0
}

fn red(state) { tenths(state, "red") }
fn yellow(state) { tenths(state, "yellow") }
