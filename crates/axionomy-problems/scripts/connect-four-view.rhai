// Connect Four: how people read the game. Labels name its cells, columns,
// counts and moves, counting columns and rows from 1 with row 1 at the
// bottom, as the scene counts them; the scene draws the board, every piece
// on it, and the tactics of the player to move: the columns that win at
// once, the opponent's threats, the drops making two threats, and those
// leaving the opponent no win.

fn player(name) { if name == "red" { "Red" } else { "Yellow" } }

// One more than the number `text` writes.
fn ordinal(text) { parse_int(text) + 1 }

fn cell(column, row) { `column ${ordinal(column)}, row ${ordinal(row)}` }

// Where a count of pieces lies, from the line's path: "in row 2", "on the
// main diagonal".
fn along(line) {
    switch line[0] {
        "row" => `in row ${ordinal(line[1])}`,
        "column" => `in column ${ordinal(line[1])}`,
        "main-diagonal" => "on the main diagonal",
        "anti-diagonal" => "on the anti-diagonal",
        _ => `in line ${ordinal(line[1])}`,
    }
}

// "a, b and c".
fn listed(items) {
    let text = "";
    for index in 0..items.len() {
        if index > 0 { text += if index + 1 == items.len() { " and " } else { ", " }; }
        text += items[index];
    }
    text
}

// An optional count of a move's rate, `none` or `some/<count>`, from
// `parts` at `at`: the count, or nothing, and where the next one starts.
fn optional_count(parts, at) {
    if parts[at] == "none" { [(), at + 1] } else { [parse_int(parts[at + 1]), at + 2] }
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return switch name {
            "game" => "Connect Four game",
            "column" => `Column ${ordinal(parts[2])}`,
            "cell" => `Cell at ${cell(parts[2], parts[3])}`,
            "result" => "Game result",
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "game" => "Game",
            "column" => "Column played",
            "cell" => "Landing cell",
            "result" => "Result",
            _ => if name.starts_with("column") { `Column ${ordinal(name.sub_string(6))}` }
                else if name.starts_with("winning") { `Winning cell ${ordinal(name.sub_string(7))}` }
                else { () },
        };
    }
    if category == "asset" {
        return switch name {
            "segment-vacancies" => `Unoccupied cells in line ${ordinal(parts[2])}`,
            "segment-pieces" => `${player(parts[2])} pieces in line ${ordinal(parts[3])}`,
            "game-identity" => "Game identity",
            "column-identity" => `This is column ${ordinal(parts[2])}`,
            "cell-identity" => `This is the cell at ${cell(parts[2], parts[3])}`,
            "result-identity" => "Result identity",
            "empty" => "Empty cell",
            "piece" => `${player(parts[2])} piece`,
            "next-row" => `Next piece lands on row ${ordinal(parts[2])}`,
            "column-full" => "Column full",
            "turn" => `${player(parts[2])} to move`,
            "line-count" => `${player(parts[2])} has ${parts[parts.len() - 1]} ${along(parts.extract(3, parts.len() - 4))}`,
            "winner" => `${player(parts[2])} wins`,
            "draw" => "Draw",
            "board-size" => `Board is ${parts[2]}×${parts[3]}`,
            "pending" => `${player(parts[2])}'s move awaits a win check`,
            _ => (),
        };
    }
    switch name {
        // A move rate exists for each count of the mover's pieces in the
        // lines through the landing cell; the label names the counts the
        // move makes.
        "move" => {
            let made = [
                `${ordinal(parts[5])} in the row`,
                `${ordinal(parts[6])} in the column`,
            ];
            let main = optional_count(parts, 7);
            if main[0] != () { made.push(`${main[0] + 1} on the main diagonal`); }
            let anti = optional_count(parts, main[1]);
            if anti[0] != () { made.push(`${anti[0] + 1} on the anti-diagonal`); }
            `${player(parts[2])} drops into column ${ordinal(parts[3])}, lands on row ${ordinal(parts[4])}, making ${listed(made)}`
        }
        "standard-move" => `${player(parts[2])} drops into column ${ordinal(parts[3])}, lands on row ${ordinal(parts[4])}`,
        "draw" | "standard-draw" => `Declare a draw after ${player(parts[2])}'s move`,
        "claim-win" => `${player(parts[2])} claims winning line ${ordinal(parts[3])}`,
        "continue" => `No win for ${player(parts[2])}; pass the turn to ${
            player(if parts[2] == "red" { "yellow" } else { "red" })
        }`,
        _ => (),
    }
}

// The board's width and height.
fn dimensions(state) {
    if state.balance("game", "board-size/7/6") > 0 { [7, 6] } else { [4, 4] }
}

// The piece a cell holds: `red`, `yellow` or nothing.
fn piece(state, column, row) {
    let account = `cell/${column}/${row}`;
    if state.balance(account, "piece/red") > 0 { return "red"; }
    if state.balance(account, "piece/yellow") > 0 { return "yellow"; }
    ()
}

// Whether the game is over: won by either player, or drawn.
fn finished(state) {
    state.balance("result", "winner/red") > 0 || state.balance("result", "winner/yellow") > 0
        || state.balance("result", "draw") > 0
}

// ---- Tactics over bitboards: a player's pieces each, bit `column * 7 +
// row` set where the player has a piece.

// Whether `bits` hold four in a line: down a column, along a row, or along
// either diagonal.
fn won(bits) {
    for shift in [1, 7, 6, 8] {
        let pairs = bits & (bits >> shift);
        if (pairs & (pairs >> (shift * 2))) != 0 { return true; }
    }
    false
}

// The open columns.
fn open_columns(heights, size) {
    let found = [];
    for column in 0..size[0] {
        if heights[column] < size[1] { found.push(column); }
    }
    found
}

// The board after `seat` drops into `column`: its two bitboards and
// column heights. Arguments are copies, so the board passed is unchanged.
fn dropped(board, seat, column) {
    let row = board.heights[column];
    board.bits[seat] = board.bits[seat] | (1 << (column * 7 + row));
    board.heights[column] = row + 1;
    board
}

// The columns where a drop by `seat` gives it four in a line.
fn wins(board, seat, size) {
    let found = [];
    for column in open_columns(board.heights, size) {
        if won(dropped(board, seat, column).bits[seat]) { found.push(column); }
    }
    found
}

// What the player to move, if any, may do: its winning columns, the
// opponent's, the columns after which the opponent cannot win at once,
// and of those the ones making two threats at once.
fn evidence(state, size) {
    let none = #{ player: (), winning: [], threats: [], safe: [], forks: [] };
    if finished(state) { return none; }
    let mover = ();
    for name in ["red", "yellow"] {
        if mover == () && state.balance("game", `turn/${name}`) > 0 { mover = name; }
    }
    if mover == () { return none; }
    let seat = if mover == "red" { 0 } else { 1 };
    let bits = [0, 0];
    let heights = [];
    for column in 0..size[0] {
        let height = 0;
        for row in 0..size[1] {
            let holder = piece(state, column, row);
            if holder == () { continue; }
            let owner = if holder == "red" { 0 } else { 1 };
            bits[owner] = bits[owner] | (1 << (column * 7 + row));
            height = row + 1;
        }
        heights.push(height);
    }
    let board = #{ bits: bits, heights: heights };
    let safe = [];
    let forks = [];
    for column in open_columns(heights, size) {
        let next = dropped(board, seat, column);
        let winning = won(next.bits[seat]);
        if winning || wins(next, 1 - seat, size).is_empty() {
            safe.push(column);
            if !winning && wins(next, seat, size).len() >= 2 { forks.push(column); }
        }
    }
    #{
        player: mover,
        winning: wins(board, seat, size),
        threats: wins(board, 1 - seat, size),
        safe: safe,
        forks: forks,
    }
}

// Columns as people count them, from 1, or `none`.
fn columns(values) {
    if values.is_empty() { return "none"; }
    let text = "";
    for value in values {
        if text != "" { text += ", "; }
        text += `${value + 1}`;
    }
    text
}

fn metric(key, label, value) {
    #{ key: key, label: label, value: value, unit: () }
}

fn scene(state, frame) {
    let size = dimensions(state);
    let width = size[0];
    let height = size[1];
    let cells = [];
    let pieces = [];
    for column in 0..width {
        for row in 0..height {
            let account = `connect-four/account/cell/${column}/${row}`;
            let holder = piece(state, column, row);
            let y = height - row - 1;
            cells.push(#{
                x: column, y: y,
                label: if holder == () { "Open slot" } else { `${player(holder)} piece` },
                classes: ["connect-slot"],
                account: account,
            });
            if holder != () {
                pieces.push(#{
                    id: #{ key: `piece/${holder}/${column}/${row}`, label: player(holder) },
                    glyph: "token",
                    anchor: #{ kind: "grid_cell", x: column, y: y },
                    role: "occupant",
                    tone: if holder == "red" { "danger" } else { "warning" },
                    status: holder,
                    account: account,
                    evidence: [#{ kind: "balance", account: account, asset: `connect-four/asset/piece/${holder}` }],
                    metrics: [],
                });
            }
        }
    }
    let tactics = evidence(state, size);
    let scene = scene::entities(scene::grid("Board and pieces", width, height, cells), pieces);
    scene.metrics = [
        metric("turn", "Next player",
            if tactics.player != () { player(tactics.player) }
            else if finished(state) { "finished" } else { "adjudicating" }),
        metric("wins", "Winning columns", columns(tactics.winning)),
        metric("opponent-wins", "Opponent threats", columns(tactics.threats)),
        metric("forks", "Double-threat moves", columns(tactics.forks)),
        metric("safe", "Safe next columns", columns(tactics.safe)),
    ];
    scene
}
