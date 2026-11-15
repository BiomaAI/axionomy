// Sokoban: how people read the warehouse. Labels name its cells, assets,
// moves and roles; the scene draws the floor, walls, goals and dead squares,
// the exit and the plate, door and latch mechanisms, the player and every
// crate, and counts held plates, latched doors and crates on goals. The
// board's size and its goals are read as the puzzle's economy script reads
// them; the exit, doors, plates and panels as the laws that read them say.

import "sokoban" as sokoban;

// One more than the number `text` writes: crates, plates and doors are
// counted from one.
fn ordinal(text) {
    parse_int(text) + 1
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return if name == "cell" { `Cell ${parts[2]}` } else { "Puzzle" };
    }
    if category == "asset" {
        return switch name {
            "dead-square" => "A crate cannot reach a goal from here",
            "at-exit" => "Player at the exit",
            "away-from-exit" => "Player inside the warehouse",
            "pressed" => `Plate ${ordinal(parts[2])} has a crate`,
            "released" => `Plate ${ordinal(parts[2])} is released`,
            "latch-ready" => `Door ${ordinal(parts[2])} can be latched`,
            "latched" => `Door ${ordinal(parts[2])} latched open`,
            "floor" => "Walkable floor",
            "wall" => "Wall",
            "player" => "Player",
            "crate" => `Crate ${ordinal(parts[2])}`,
            "empty" => "Empty cell",
            "active" => "Puzzle in progress",
            "solved" => "Puzzle solved",
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "from" => "Player's cell",
            "middle" => "Crate's cell",
            "to" => "Destination cell",
            "puzzle" => "Puzzle",
            "goal" => `Goal square ${ordinal(parts[2])}`,
            _ => (),
        };
    }
    switch name {
        "latch" => `Latch door ${ordinal(parts[2])} from panel at cell ${parts[3]}`,
        "move" => `Move from cell ${parts[2]} to cell ${parts[3]}`,
        "push" => `Push crate ${ordinal(parts[5])} from cell ${parts[3]} to cell ${parts[4]}, standing at cell ${parts[2]}`,
        "finish" => "Finish",
        _ => (),
    }
}

fn at_cell(cell, width) {
    #{ kind: "grid_cell", x: cell % width, y: cell / width }
}

fn entity(key, label, glyph, anchor, role, tone, status) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: anchor, role: role, tone: tone, status: status,
        evidence: [], metrics: [],
    }
}

// The entity, showing the balance of `asset` that cell `cell` holds.
fn linked(entity, cell, asset) {
    let account = `sokoban/account/cell/${cell}`;
    entity.account = account;
    entity.evidence = [#{ kind: "balance", account: account, asset: `sokoban/asset/${asset}` }];
    entity
}

fn metric(key, label, value) {
    #{ key: key, label: label, value: value.to_string(), unit: () }
}

// What the laws say each cell is, keyed by the cell: `goal` where finishing
// fixes a goal role; `exit`, `door-0` or `plate-1` where a move or push
// enters the cell `{to}` and its relations give `{to-exit}`, `{to-door-0}`
// or `{to-plate-1}`; `panel-0` where a latch stands.
fn layout(state) {
    let layout = #{};
    for cell in sokoban::goals(state) { layout[cell.to_string()] = ["goal"]; }
    for rate in state.instances("move") + state.instances("push") {
        let values = state.values(rate);
        let at = values["to"].to_string();
        let facts = if at in layout { layout[at] } else { [] };
        for name in values.keys() {
            let fact = name.sub_string(3);
            if name.starts_with("to-") && values[name] == 1 && !facts.contains(fact) { facts.push(fact); }
        }
        layout[at] = facts;
    }
    for rate in state.instances("latch") {
        let values = state.values(rate);
        let at = values["at"].to_string();
        let facts = if at in layout { layout[at] } else { [] };
        facts.push(`panel-${values["circuit"]}`);
        layout[at] = facts;
    }
    layout
}

fn scene(state, frame) {
    let size = sokoban::dimensions(state);
    let width = size[0];
    let height = size[1];
    let puzzle = state.balances("success");
    let layout = layout(state);
    let cells = [];
    let entities = [];
    let on_goals = 0;
    for cell in 0..width * height {
        let holds = state.balances(`cell/${cell}`);
        let facts = if cell.to_string() in layout { layout[cell.to_string()] } else { [] };
        let wall = "wall" in holds;
        let goal = facts.contains("goal");
        let dead = "dead-square" in holds;
        let classes = [if wall { "wall" } else { "floor" }];
        if goal { classes.push("goal"); }
        if dead { classes.push("dead-square"); }
        cells.push(#{
            x: cell % width,
            y: cell / width,
            label: if wall { "Wall" } else if goal { "Goal floor" }
                else if dead { "No crate route to any goal" } else { "Warehouse floor" },
            classes: classes,
            account: `sokoban/account/cell/${cell}`,
        });

        if facts.contains("exit") {
            entities.push(entity(
                "exit", "Exit", "goal", at_cell(cell, width), "structure", "goal",
                "leave after all deliveries",
            ));
        }
        for circuit in 0..2 {
            for mechanism in [["plate", "Plate", "constraint"], ["door", "Door", "door"], ["panel", "Latch panel", "constraint"]] {
                let fact = `${mechanism[0]}-${circuit}`;
                if !facts.contains(fact) { continue; }
                let pressed = `pressed/${circuit}` in puzzle;
                let latched = `latched/${circuit}` in puzzle;
                let status = switch mechanism[0] {
                    "plate" => if pressed { "holding door open" } else { "needs a crate" },
                    "door" => if latched { "latched open" } else if pressed { "open under pressure" } else { "closed" },
                    _ => if latched { "latched" } else { "ready to latch" },
                };
                entities.push(entity(
                    `mechanism/${mechanism[0]}/${circuit}`, `${mechanism[1]} ${circuit + 1}`, mechanism[2],
                    at_cell(cell, width), "structure",
                    if latched || pressed { "success" } else { "warning" }, status,
                ));
            }
        }
        // Only floor holds the player or a crate.
        if !("floor" in holds) { continue; }
        if "player" in holds {
            entities.push(linked(entity(
                "player", "Player", "agent", at_cell(cell, width), "occupant", "active", "player",
            ), cell, "player"));
        }
        let crate_id = ();
        for asset in holds.keys() {
            let parts = asset.split("/");
            if crate_id == () && parts[0] == "crate" { crate_id = parse_int(parts[1]); }
        }
        if crate_id != () {
            if goal { on_goals += 1; }
            let trapped = dead && !goal;
            entities.push(linked(entity(
                `crate/${crate_id}`, `Crate ${crate_id + 1}`, "package", at_cell(cell, width), "occupant",
                if goal { "success" } else if trapped { "danger" } else { "warning" },
                if goal { "on-goal" } else if trapped { "deadlocked" } else { "crate" },
            ), cell, `crate/${crate_id}`));
        }
    }
    let scene = scene::entities(
        scene::grid("Warehouse floor, stable crates, goals, and dead squares", width, height, cells),
        entities,
    );
    let held = 0;
    let latched = 0;
    for circuit in 0..2 {
        if `pressed/${circuit}` in puzzle { held += 1; }
        if `latched/${circuit}` in puzzle { latched += 1; }
    }
    scene.metrics = [
        metric("held-plates", "Crates holding plates", held),
        metric("latched", "Doors latched open", latched),
        metric("goals", "Crates on goals", on_goals),
    ];
    scene
}
