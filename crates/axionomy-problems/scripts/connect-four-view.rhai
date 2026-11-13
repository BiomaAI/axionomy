// Connect Four: how people read the game. Labels name its cells, columns,
// counts and moves, counting columns and rows from 1 with row 1 at the
// bottom, as the scene counts them; the scene draws the board, every piece
// on it, and the tactics of the player to move: the columns that win at
// once, the opponent's threats, the drops making two threats, and those
// leaving the opponent no win, as the game's own tactics read the board.

import "connect-four" as game;

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

// The piece a cell holds: `red`, `yellow` or nothing.
fn piece(state, column, row) {
    let account = `cell/${column}/${row}`;
    if state.balance(account, "piece/red") > 0 { return "red"; }
    if state.balance(account, "piece/yellow") > 0 { return "yellow"; }
    ()
}

// What the player to move, if any, may do: its winning columns, the
// opponent's, the columns after which the opponent cannot win at once,
// and of those the ones making two threats at once.
fn evidence(state, width, height) {
    let none = #{ player: (), winning: [], threats: [], safe: [], forks: [] };
    let mover = game::to_move(state);
    if game::outcome(state) != () || mover == () { return none; }
    let seat = game::seat(mover);
    let boards = game::pieces(state, width, height);
    let mine = boards[seat];
    let theirs = boards[1 - seat];
    let occupied = mine | theirs;
    let safe = [];
    let forks = [];
    for column in 0..width {
        if !game::is_open(occupied, column, height) { continue; }
        let landed = game::landing(occupied, column);
        let after = occupied | landed;
        let winning = game::won(mine | landed);
        if winning || game::winning(theirs, after, width, height).is_empty() {
            safe.push(column);
            if !winning && game::winning(mine | landed, after, width, height).len() >= 2 {
                forks.push(column);
            }
        }
    }
    #{
        player: mover,
        winning: game::winning(mine, occupied, width, height),
        threats: game::winning(theirs, occupied, width, height),
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
            if tactics.player != () { player(tactics.player) }
            else if game::outcome(state) != () { "finished" } else { "adjudicating" }),
        metric("wins", "Winning columns", columns(tactics.winning)),
        metric("opponent-wins", "Opponent threats", columns(tactics.threats)),
        metric("forks", "Double-threat moves", columns(tactics.forks)),
        metric("safe", "Safe next columns", columns(tactics.safe)),
    ];
    scene
}
