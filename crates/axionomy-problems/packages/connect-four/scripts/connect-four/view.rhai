// Connect Four: how people read the game. Labels name its cells, columns,
// lines and moves, counting columns and rows from 1 with row 1 at the
// bottom, as the scene counts them; the scene draws the board, every piece
// on it, and the tactics of the player to move: the columns that win at
// once, the opponent's threats, the drops making two threats, and those
// leaving the opponent no win, each read from the lines of four the game
// counts.

import "connect-four" as game;

fn player(name) { if name == "red" { "Red" } else { "Yellow" } }
fn opponent(name) { if name == "red" { "yellow" } else { "red" } }

// One more than the number `text` writes.
fn ordinal(text) { parse_int(text) + 1 }

fn cell(column, row) { `column ${ordinal(column)}, row ${ordinal(row)}` }

// A line of four as people read it, from its direction, rightmost column
// and top row.
fn line(direction, column, row) {
    let way = switch direction {
        "across" => "line along a row",
        "up" => "line up a column",
        "rising" => "rising diagonal",
        _ => "falling diagonal",
    };
    `the ${way} with rightmost column ${ordinal(column)} and top row ${ordinal(row)}`
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
            "full" => `Full column ${ordinal(parts[2])}`,
            _ => (),
        };
    }
    if category == "asset" {
        return switch name {
            "empty" => "Empty cell",
            "piece" => `${player(parts[2])} piece`,
            "height" => "Pieces in the column",
            "turn" => `${player(parts[2])} to move`,
            "line" => `${player(parts[2])} pieces in ${line(parts[3], parts[4], parts[5])}`,
            "winner" => `${player(parts[2])} wins`,
            "draw" => "Draw",
            _ => (),
        };
    }
    switch name {
        "drop" => `${player(parts[2])} drops into column ${ordinal(parts[3])}, lands on row ${ordinal(parts[4])}`,
        "win" => `${player(parts[2])} drops into column ${ordinal(parts[3])}, lands on row ${ordinal(parts[4])} and completes a line of four`,
        "draw" => `Declare a draw at ${player(parts[2])}'s turn`,
        _ => (),
    }
}

// The piece a cell holds: `red`, `yellow` or nothing.
fn piece(state, column, row) {
    let account = `cell/${column}/${row}`;
    if state.balance(account, "piece/red") > 0 { return "red"; }
    if state.balance(account, "piece/yellow") > 0 { return "yellow"; }
    ()
}

// The lines of four through the cell at `column` and `row`, each named by
// its direction, rightmost column and top row, as the game counts them.
fn through(column, row) {
    let found = [];
    for k in 0..4 {
        found.push(`across/${column + k}/${row}`);
        found.push(`up/${column}/${row + k}`);
        found.push(`rising/${column + k}/${row + k}`);
        found.push(`falling/${column + k}/${row + 3 - k}`);
    }
    found
}

// Whether a drop by `mover` on the cell at `column` and `row` completes a
// line of four: a line through it already holds three of its pieces, or
// holds two and `added`, a line a drop just made, is one of them.
fn completes(state, mover, column, row, added) {
    for line in through(column, row) {
        let held = game::held(state, mover, line) + if added.contains(line) { 1 } else { 0 };
        if held >= 3 { return true; }
    }
    false
}

// What the player to move, if any, may do, from the heights of the columns
// and the lines the game counts: its winning columns, the opponent's, the
// columns after which the opponent cannot win at once, and of those the
// ones making two threats at once.
fn evidence(state, width, height) {
    let none = #{ player: (), winning: [], threats: [], safe: [], forks: [] };
    let mover = game::to_move(state);
    if game::outcome(state) != () || mover == () { return none; }
    let other = opponent(mover);
    let landing = [];
    for column in 0..width {
        let row = state.balance(`column/${column}`, "height").to_int();
        landing.push(if row < height { row } else { () });
    }
    // The columns where `who`'s drop would complete a line, once a drop on
    // the cell at `column` and `row` has added its lines, if any.
    let wins = |who, column, row, added| {
        let found = [];
        for at in 0..width {
            let lands = if at == column { if row + 1 < height { row + 1 } else { () } } else { landing[at] };
            if lands != () && completes(state, who, at, lands, added) { found.push(at); }
        }
        found
    };
    let winning = wins.call(mover, -1, 0, []);
    let threats = wins.call(other, -1, 0, []);
    let safe = [];
    let forks = [];
    for column in 0..width {
        let row = landing[column];
        if row == () { continue; }
        if winning.contains(column) {
            safe.push(column);
            continue;
        }
        if !wins.call(other, column, row, []).is_empty() { continue; }
        safe.push(column);
        if wins.call(mover, column, row, through(column, row)).len() >= 2 { forks.push(column); }
    }
    #{ player: mover, winning: winning, threats: threats, safe: safe, forks: forks }
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
    let width = game::board_width(state);
    let height = game::board_height(state);
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
    let tactics = evidence(state, width, height);
    let scene = scene::entities(scene::grid("Board and pieces", width, height, cells), pieces);
    scene.metrics = [
        metric("turn", "Next player",
            if tactics.player != () { player(tactics.player) } else { "finished" }),
        metric("wins", "Winning columns", columns(tactics.winning)),
        metric("opponent-wins", "Opponent threats", columns(tactics.threats)),
        metric("forks", "Double-threat moves", columns(tactics.forks)),
        metric("safe", "Safe next columns", columns(tactics.safe)),
    ];
    scene
}
