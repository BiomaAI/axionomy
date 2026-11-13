// Connect Four: Red and Yellow take turns dropping a piece into a column of
// a board, the compact 4×4 one or the standard 7×6 one, where it lands on
// the lowest empty cell. On the compact board a drop that completes a line
// of four wins at once, and a full board without one is drawn. On the
// standard board every drop waits on its adjudication: a claim of the line
// of four it completes, the draw on a full board, or the turn passing on.
//
// The game's players are tactical: a drop that wins, else one that makes two
// threats at once, else one that leaves the opponent no winning drop. Tree
// search weighs those, simulating play that explores at random one time in
// four and otherwise drops where the board scores best; the opponents drop
// nearest the center, or as a depth-five alpha-beta search scores.
//
// Board reasoning reads the pieces as two bitboards, a player's pieces
// each, bit `column * 7 + row` set where the player has a piece: a column
// has seven bits, room for its rows and an empty one above them, so a line
// of four shows as four bits a fixed distance apart. Every move is still
// the economy's exchange, kept only where it applies.

// ---- The economy

fn standard(state) { state.balance("game", "board-size/7/6") > 0 }
fn board_width(state) { if standard(state) { 7 } else { 4 } }
fn board_height(state) { if standard(state) { 6 } else { 4 } }

// The player to drop a piece, if any.
fn to_move(state) {
    for player in ["red", "yellow"] {
        if state.balance("game", `turn/${player}`) > 0 { return player; }
    }
    ()
}

// The player whose drop waits on its adjudication, if any.
fn adjudicating(state) {
    for player in ["red", "yellow"] {
        if state.balance("game", `pending/${player}`) > 0 { return player; }
    }
    ()
}

// Each player's place in the game's payoffs and in its bitboards.
fn seat(player) { if player == "red" { 0 } else { 1 } }

// Each player's pieces as a bitboard, Red's then Yellow's. Pieces stack
// from the bottom row, so a column's first empty cell ends it.
fn pieces(state, width, height) {
    let boards = [0, 0];
    for column in 0..width {
        for row in 0..height {
            let cell = `cell/${column}/${row}`;
            let bit = 1 << (column * 7 + row);
            if state.balance(cell, "piece/red") > 0 {
                boards[0] = boards[0] | bit;
            } else if state.balance(cell, "piece/yellow") > 0 {
                boards[1] = boards[1] | bit;
            } else {
                break;
            }
        }
    }
    boards
}

// The row a drop into `column` lands on, if the column is open.
fn next_row(state, column, height) {
    let holder = `column/${column}`;
    for row in 0..height {
        if state.balance(holder, `next-row/${row}`) > 0 { return row; }
    }
    ()
}

// The column a drop is into, or nothing for an adjudication or a draw.
fn column_of(action) {
    let parts = action.rate().split("/");
    if parts[0] == "move" || parts[0] == "standard-move" { parse_int(parts[2]) } else { () }
}

// The count `player` holds on `line` of the compact board: the one count
// token of the line the game account holds.
fn line_count(state, player, line) {
    for count in 0..5 {
        if state.balance("game", `line-count/${player}/${line}/${count}`) > 0 { return count; }
    }
    throw `${player} holds no count on ${line}`;
}

// A drop on the compact board, consuming the counts of every line through
// its cell and producing their successors, binding the result where it
// completes one.
fn compact_drop(state, player, column, row) {
    let rows = line_count(state, player, `row/${row}`);
    let columns = line_count(state, player, `column/${column}`);
    let main = if column == row { line_count(state, player, "main-diagonal") } else { () };
    let anti = if column + row + 1 == 4 { line_count(state, player, "anti-diagonal") } else { () };
    let bindings = #{ game: "game", column: `column/${column}`, cell: `cell/${column}/${row}` };
    if rows == 3 || columns == 3 || main == 3 || anti == 3 { bindings.result = "result"; }
    let main = if main == () { "none" } else { `some/${main}` };
    let anti = if anti == () { "none" } else { `some/${anti}` };
    exchange(`move/${player}/${column}/${row}/${rows}/${columns}/${main}/${anti}`, bindings, 1)
}

// The standard board's lines of four, each the cells it runs through: the
// rows', the columns', then each diagonal rising and falling from a corner.
fn segments() {
    let found = [];
    for row in 0..6 {
        for column in 0..4 {
            found.push([[column, row], [column + 1, row], [column + 2, row], [column + 3, row]]);
        }
    }
    for column in 0..7 {
        for row in 0..3 {
            found.push([[column, row], [column, row + 1], [column, row + 2], [column, row + 3]]);
        }
    }
    for column in 0..4 {
        for row in 0..3 {
            found.push([[column, row], [column + 1, row + 1], [column + 2, row + 2], [column + 3, row + 3]]);
            found.push([[column, row + 3], [column + 1, row + 2], [column + 2, row + 1], [column + 3, row]]);
        }
    }
    found
}

// What follows `player`'s drop on the standard board: the claims of the
// lines of four it holds, else the draw on a full board, else the turn
// passing on.
fn adjudications(state, player) {
    let claims = [];
    let lines = ();
    for segment in 0..69 {
        if state.balance("game", `segment-pieces/${player}/${segment}`) == 4 {
            if lines == () { lines = segments(); }
            let bindings = #{ game: "game", result: "result" };
            let cells = lines[segment];
            for place in 0..4 {
                bindings[`winning${place}`] = `cell/${cells[place][0]}/${cells[place][1]}`;
            }
            claims.push(exchange(`claim-win/${player}/${segment}`, bindings, 1));
        }
    }
    if !claims.is_empty() { return claims; }
    let full = true;
    for column in 0..7 {
        if state.balance(`column/${column}`, "column-full") == 0 { full = false; }
    }
    if full {
        let bindings = #{ game: "game", result: "result" };
        for column in 0..7 { bindings[`column${column}`] = `column/${column}`; }
        return [exchange(`standard-draw/${player}`, bindings, 1)];
    }
    [exchange(`continue/${player}`, #{ game: "game" }, 1)]
}

// Every legal move, in column order: the drops into the open columns, the
// compact board's draw once none is open, or a standard drop's
// adjudications. Each consumes what the balances it reads say is there, so
// each applies, and a candidate set keeps only those that do.
fn moves(state) {
    let large = standard(state);
    if large {
        let pending = adjudicating(state);
        if pending != () { return adjudications(state, pending); }
    }
    let player = to_move(state);
    if player == () { return []; }
    let height = board_height(state);
    let drops = [];
    for column in 0..board_width(state) {
        let row = next_row(state, column, height);
        if row == () { continue; }
        drops.push(if large {
            exchange(`standard-move/${player}/${column}/${row}`,
                #{ game: "game", column: `column/${column}`, cell: `cell/${column}/${row}` }, 1)
        } else {
            compact_drop(state, player, column, row)
        });
    }
    if drops.is_empty() && !large {
        let bindings = #{ game: "game", result: "result" };
        for column in 0..4 { bindings[`column${column}`] = `column/${column}`; }
        drops.push(exchange(`draw/${player}`, bindings, 1));
    }
    drops
}

// ---- Board reasoning over bitboards

// Whether `bits` hold four in a line: down a column, along a row, or along
// either diagonal.
fn won(bits) {
    let pairs = bits & (bits >> 1);
    if (pairs & (pairs >> 2)) != 0 { return true; }
    pairs = bits & (bits >> 7);
    if (pairs & (pairs >> 14)) != 0 { return true; }
    pairs = bits & (bits >> 6);
    if (pairs & (pairs >> 12)) != 0 { return true; }
    pairs = bits & (bits >> 8);
    (pairs & (pairs >> 16)) != 0
}

// Whether `column` has an empty cell, its top one empty.
fn is_open(occupied, column, height) { (occupied & (1 << (column * 7 + height - 1))) == 0 }

// The bit of the cell a drop into `column` lands on: the column's lowest
// empty one, one above its stacked pieces.
fn landing(occupied, column) { (occupied + (1 << (column * 7))) & (127 << (column * 7)) }

// The open columns where a drop gives `mine` four in a line.
fn winning(mine, occupied, width, height) {
    let found = [];
    for column in 0..width {
        if is_open(occupied, column, height) && won(mine | landing(occupied, column)) {
            found.push(column);
        }
    }
    found
}

// The columns tactics prefer for the player holding `mine`: those that win
// at once; else those making two winning threats at once; else those
// leaving the opponent no winning drop; none where every drop does.
fn preferred(mine, theirs, width, height) {
    let occupied = mine | theirs;
    let wins = winning(mine, occupied, width, height);
    if !wins.is_empty() { return wins; }
    let safe = [];
    let forks = [];
    for column in 0..width {
        if !is_open(occupied, column, height) { continue; }
        let after = occupied | landing(occupied, column);
        if !winning(theirs, after, width, height).is_empty() { continue; }
        safe.push(column);
        if winning(mine | landing(occupied, column), after, width, height).len() >= 2 {
            forks.push(column);
        }
    }
    if forks.is_empty() { safe } else { forks }
}

// Every window of four cells in a line on the board, as bit masks.
fn windows(width, height) {
    let found = [];
    for row in 0..height {
        for column in 0..(width - 3) { found.push(0x204081 << (column * 7 + row)); }
    }
    for column in 0..width {
        for row in 0..(height - 3) { found.push(15 << (column * 7 + row)); }
    }
    for column in 0..(width - 3) {
        for row in 0..(height - 3) {
            found.push(0x1010101 << (column * 7 + row));
            found.push(0x208208 << (column * 7 + row));
        }
    }
    found
}

// The board's worth to the player holding `mine`: a hundred thousand for a
// line of four, as much less for the opponent's, and otherwise what the
// windows only one player holds pieces in are worth, the player's own less
// the opponent's, counting windows gravity cannot reach yet. A window
// holding one, two or three of a player's pieces is worth 1, 8 or 60.
fn score(mine, theirs, windows) {
    if won(mine) { return 100000; }
    if won(theirs) { return -100000; }
    let worth = [0, 1, 8, 60, 100000];
    let total = 0;
    for window in windows {
        let own = mine & window;
        let enemy = theirs & window;
        if enemy == 0 {
            let count = 0;
            while own != 0 { own = own & (own - 1); count += 1; }
            total += worth[count];
        } else if own == 0 {
            let count = 0;
            while enemy != 0 { enemy = enemy & (enemy - 1); count += 1; }
            total -= worth[count];
        }
    }
    total
}

// How far `column` is from the board's center, in half columns.
fn off_center(column, width) { abs(column * 2 - width + 1) }

// The columns nearest the center first, of equals the left one first.
fn center_first(width) { if width == 7 { [3, 2, 4, 1, 5, 0, 6] } else { [1, 2, 0, 3] } }

// Negamax with alpha-beta pruning: the board's worth to the player holding
// `mine`, to move, looking `depth` drops ahead, the center's first; a loss
// counts more the sooner it comes, and a full board is even.
fn negamax(mine, theirs, depth, alpha, beta, windows, width, height) {
    if won(theirs) { return -100000 - depth; }
    let occupied = mine | theirs;
    let open = [];
    for column in center_first(width) {
        if is_open(occupied, column, height) { open.push(column); }
    }
    if open.is_empty() { return 0; }
    if depth == 0 { return score(mine, theirs, windows); }
    let best = -2147483648;
    for column in open {
        let value = -negamax(theirs, mine | landing(occupied, column), depth - 1, -beta, -alpha,
            windows, width, height);
        if value > best { best = value; }
        if value > alpha { alpha = value; }
        if alpha >= beta { break; }
    }
    best
}

// The player to move's pieces and the opponent's, as bitboards.
fn sides(state, player) {
    let boards = pieces(state, board_width(state), board_height(state));
    let mine = seat(player);
    [boards[mine], boards[1 - mine]]
}

// ---- The players

// The tactical moves tree search weighs: of the drops, those in the
// columns tactics prefer, or every drop where it prefers none; every move
// where no player is to drop.
fn tactical(state) {
    let legal = moves(state);
    let player = to_move(state);
    if player == () { return legal; }
    let board = sides(state, player);
    let columns = preferred(board[0], board[1], board_width(state), board_height(state));
    if columns.is_empty() { return legal; }
    let kept = [];
    for action in legal {
        let column = column_of(action);
        if column == () || columns.contains(column) { kept.push(action); }
    }
    kept
}

// Who chooses each turn: Red while Red is to drop, and Yellow otherwise, so
// Yellow takes every drop's adjudication, an opponent the first the economy
// offers. Every adjudication of a drop leads to the same position, so who
// weighs one in a search changes nothing.
fn mover(state) { if state.balance("game", "turn/red") > 0 { 0 } else { 1 } }

// Each player's payoff where the game is over: one for a win, a half for a
// draw, nothing for a loss.
fn outcome(state) {
    if state.balance("result", "winner/red") > 0 { return [1.0, 0.0]; }
    if state.balance("result", "winner/yellow") > 0 { return [0.0, 1.0]; }
    if state.balance("result", "draw") > 0 { return [0.5, 0.5]; }
    ()
}

// Each player's expected payoff where a simulation stops short of the end:
// a half, moved toward the player the board's score favours.
fn estimate(state) {
    let over = outcome(state);
    if over != () { return over; }
    let width = board_width(state);
    let height = board_height(state);
    let boards = pieces(state, width, height);
    let worth = score(boards[0], boards[1], windows(width, height)).to_float();
    let red = 0.5 + 0.45 * worth / (abs(worth) + 120.0);
    [red, 1.0 - red]
}

// How simulations play: the only move there is; one of the candidates at
// random one time in four, drawn with the search's own tickets; otherwise
// the drop scoring best for the player, eight times its score less how far
// it is from the center, of equals the last.
fn playout(state, candidates, random) {
    if candidates.is_empty() { return (); }
    let player = to_move(state);
    if candidates.len() == 1 || player == () { return candidates[0]; }
    if random.sample([1, 1, 1, 1]) == 0 {
        return candidates[random.sample(candidates.map(|candidate| 1))];
    }
    let width = board_width(state);
    let board = sides(state, player);
    let occupied = board[0] | board[1];
    let masks = windows(width, board_height(state));
    let best = ();
    let most = 0;
    for action in candidates {
        let column = column_of(action);
        let worth = if column == () {
            0
        } else {
            score(board[0] | landing(occupied, column), board[1], masks) * 8
                - off_center(column, width)
        };
        if best == () || worth >= most {
            best = action;
            most = worth;
        }
    }
    best
}

// Yellow's drop as an opponent: of the legal drops, the one nearest the
// center, or the one the depth-five search scores best, of equals the
// last; an adjudication is the first the economy offers.
fn opponent(state, searching) {
    let legal = moves(state);
    if legal.is_empty() { return (); }
    let player = to_move(state);
    if player == () { return legal[0]; }
    let width = board_width(state);
    let height = board_height(state);
    let board = sides(state, player);
    let occupied = board[0] | board[1];
    let masks = if searching { windows(width, height) } else { [] };
    let best = ();
    let most = 0;
    for action in legal {
        let column = column_of(action);
        let worth = if searching {
            -negamax(board[1], board[0] | landing(occupied, column), 4, -200000, 200000, masks,
                width, height)
        } else {
            -off_center(column, width)
        };
        if best == () || worth >= most {
            best = action;
            most = worth;
        }
    }
    best
}

fn center_opponent(state, random) { opponent(state, false) }
fn search_opponent(state, random) { opponent(state, true) }

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
