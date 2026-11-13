// Maze: how people read the key-door maze. Labels and formats name its
// identifiers; the scene draws the rooms and passages, the explorer, the key
// and the gate, and the route a trace plans, has taken and is taking.

// A room's name as people read it: `key-room` is `Key Room`.
fn words(name) {
    let parts = [];
    for part in name.split("-") {
        parts.push(part.sub_string(0, 1).to_upper() + part.sub_string(1));
    }
    parts.reduce(|sum, part| if sum == () { part } else { sum + " " + part })
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return if name == "agent" { "Explorer" } else { "Vault District" };
    }
    if category == "role" {
        return words(name);
    }
    if category == "asset" {
        return switch name {
            "power-transit" => "Power routed to the canal crossing",
            "power-vault" => "Power routed to the vault shutter",
            "health" => "Health reserve",
            "injury" => "Injury delay per action",
            "shield" => "Protective equipment",
            "used-shield" => "Spent protection",
            "medicine" => "Available medicine",
            "used-medicine" => "Medicine used",
            "at" => `Standing in ${words(parts[2])}`,
            "edge" => `Passage ${words(parts[2])} → ${words(parts[3])}`,
            "key" => "Key",
            "locked" => "Vault gate locked",
            "open" => "Vault gate open",
            "energy" => "Energy left",
            "spent-energy" => "Energy spent",
            "time" => "Time left",
            "spent-time" => "Time spent",
            "target" => `Goal: reach ${words(parts[2])}`,
            "distance" => `Distance from ${words(parts[2])} to the exit`,
            "active" => "Expedition in progress",
            "solved" => "Explorer escaped",
            _ => (),
        };
    }
    switch name {
        "divert-power" => `Divert power at ${words(parts[2])} to ${if parts[3] == "true" { "vault" } else { "canal" }}`,
        "take-protection" => `Collect protective equipment at ${words(parts[2])}`,
        "treat" => `Treat injuries at ${words(parts[2])}`,
        "protected-move" => `Protected crossing ${words(parts[2])} → ${words(parts[3])} for ${parts[4]} energy`,
        "move" => `Walk ${words(parts[2])} → ${words(parts[3])} for ${parts[4]} energy${if parts[5] == "true" { " once the vault gate is open" } else { "" }}`,
        "take-key" => `Pick up the key in ${words(parts[2])}`,
        "unlock-door" => `Unlock the gate from ${words(parts[2])}`,
        "finish" => "Reach the exit",
        _ => (),
    }
}

fn format(asset) {
    let name = asset.split("/")[0];
    if name == "energy" || name == "spent-energy" || name == "distance" {
        return #{ scale: 0, unit: "units" };
    }
    if name == "time" || name == "spent-time" || name == "injury" {
        return #{ scale: 0, unit: "ticks" };
    }
    if name == "shield" || name == "used-shield" {
        return #{ scale: 0, unit: "charges" };
    }
    ()
}

// Where a room is drawn.
fn position(node) {
    switch node {
        "start" => [40.0, 300.0], "atrium" => [220.0, 260.0], "library" => [400.0, 90.0],
        "key-room" => [590.0, 40.0], "gallery" => [500.0, 260.0], "gate" => [690.0, 260.0],
        "vault" => [870.0, 260.0], "exit" => [1060.0, 300.0], "garden" => [220.0, 460.0],
        "market" => [410.0, 460.0], "canal" => [610.0, 460.0], "bridge" => [820.0, 460.0],
        "workshop" => [410.0, 650.0], "tunnel" => [110.0, 650.0], "detour" => [720.0, 650.0],
        "archive" => [220.0, 50.0], "scriptorium" => [400.0, -90.0], "docks" => [300.0, 840.0],
        "foundry" => [500.0, 840.0], "tower" => [700.0, 840.0], "observatory" => [980.0, 650.0],
        "ridge" => [80.0, 850.0], "ruins" => [180.0, 1030.0], "chapel" => [600.0, 1030.0],
    }
}

fn held(state, account, asset) {
    !state.balance(account, asset).is_zero()
}

// The passage an exchange walks, if it walks one.
fn route(exchange) {
    let parts = exchange.rate().split("/");
    if parts[0] == "move" || parts[0] == "protected-move" {
        `edge/${parts[1]}/${parts[2]}`
    } else {
        ()
    }
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

fn entity(key, label, glyph, anchor, role, tone, status) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: anchor, role: role, tone: tone, status: status,
        evidence: [], metrics: [],
    }
}

// The entity, showing the balance of `asset` that `account` holds.
fn linked(entity, account, asset) {
    entity.account = `maze/account/${account}`;
    entity.evidence = [#{ kind: "balance", account: `maze/account/${account}`, asset: `maze/asset/${asset}` }];
    entity
}

fn at(node) {
    #{ kind: "graph_node", node: `node/${node}` }
}

fn scene(state, frame) {
    let nodes = [];
    for rate in state.rates("move") {
        let parts = rate.split("/");
        for node in [parts[1], parts[2]] {
            if !nodes.contains(node) { nodes.push(node); }
        }
    }
    nodes.sort();
    let focus = ();
    for node in nodes {
        if focus == () && held(state, "agent", `at/${node}`) { focus = node; }
    }
    let graph_nodes = nodes.map(|node| {
        let classes = if node == "gate" { ["facility"] } else { ["location"] };
        if focus == node { classes.push("current"); }
        if node == "exit" { classes.push("goal"); }
        let position = position(node);
        #{ id: #{ key: `node/${node}`, label: words(node) }, classes: classes, x: position[0], y: position[1] }
    });
    let health = held(state, "agent", "health");
    let open = held(state, "world", "open");
    let vault_power = held(state, "world", "power-vault");
    let transit_power = held(state, "world", "power-transit");
    let graph_edges = [];
    for rate in state.rates("move") {
        let parts = rate.split("/");
        let gated = parts[4] == "true";
        let locked = (gated && (!open || (health && !vault_power)))
            || (parts[1] == "canal" && parts[2] == "bridge" && !transit_power && health);
        graph_edges.push(#{
            id: `edge/${parts[1]}/${parts[2]}`,
            source: `node/${parts[1]}`,
            target: `node/${parts[2]}`,
            label: if gated { `${parts[3]} E · gate` } else { `${parts[3]} E` },
            classes: if locked { ["locked"] } else { [] },
        });
    }
    let scene = scene::graph(
        "The Vault District — find the key, open the gate, reach the exit",
        graph_nodes,
        graph_edges,
        if focus == () { () } else { `node/${focus}` },
    );

    if focus != () {
        let energy = state.balance("agent", "energy");
        let time = state.balance("agent", "time");
        let escaped = focus == "exit";
        let explorer = linked(entity(
            "agent/explorer", "Explorer", "agent", at(focus), "occupant",
            if escaped { "success" } else { "active" },
            if escaped { `escaped · E ${energy} · T ${time}` } else { `E ${energy} · T ${time}` },
        ), "agent", `at/${focus}`);
        explorer.metrics = [
            metric("energy", "Energy", energy, "units"),
            metric("time", "Time", time, "ticks"),
        ];
        scene.entities.push(explorer);
    }
    if held(state, "world", "key") {
        scene.entities.push(linked(entity(
            "maze/key/brass", "Brass key", "key", at("key-room"), "attachment", "warning", "available",
        ), "world", "key"));
    } else if held(state, "agent", "key") {
        scene.entities.push(linked(entity(
            "maze/key/brass", "Brass key", "key", #{ kind: "entity", entity: "agent/explorer" },
            "attachment", "active", "carried",
        ), "agent", "key"));
    }
    let gate_open = open && (!health || vault_power);
    let gate_status = if gate_open { "open" } else if open { "unlocked · needs power" } else { "locked" };
    scene.entities.push(linked(entity(
        "maze/gate/state", "Vault gate", "door", at("gate"), "state",
        if gate_open { "success" } else { "danger" }, gate_status,
    ), "world", if gate_open { "open" } else { "locked" }));

    let key_state = if held(state, "world", "key") { "waiting" }
        else if held(state, "agent", "key") { "carried" } else { "used" };
    scene.metrics = [
        metric("energy-left", "Explorer energy", state.balance("agent", "energy"), "units"),
        metric("time-left", "Time remaining", state.balance("agent", "time"), "ticks"),
        metric("key-state", "Key", key_state, ()),
        metric("gate-state", "Vault gate", gate_status, ()),
    ];
    if health {
        let power = if vault_power { "vault powered" } else { "canal powered" };
        scene.metrics += [
            metric("power", "Shared power", if vault_power { "vault shutter" } else { "canal crossing" }, ()),
            metric("injury", "Injury delay per action", state.balance("agent", "injury"), "ticks"),
            metric("shield", "Protection remaining", state.balance("agent", "shield"), "charges"),
            metric("health", "Health", state.balance("agent", "health"), ()),
        ];
        let protection = if held(state, "world", "shield") { "2 charges · 1 tick" } else { "collected" };
        for station in [
            ["power-library", "Power switch", "library", "energy", power],
            ["power-workshop", "Power switch", "workshop", "energy", power],
            ["medical", "Aid station", "garden", "repair", "treat injuries · 2 ticks"],
            ["protection", "Protective equipment", "start", "shield", protection],
        ] {
            scene.entities.push(entity(
                station[0], station[1], station[3], at(station[2]), "attachment", "neutral", station[4],
            ));
        }
    }
    scene.legend = [
        #{ label: "Room", glyph: "location", tone: "neutral" },
        #{ label: "Explorer", glyph: "agent", tone: "active" },
        #{ label: "Key", glyph: "key", tone: "warning" },
        #{
            label: if gate_open { "Open gate" } else { "Locked gate" },
            glyph: "door",
            tone: if gate_open { "success" } else { "danger" },
        },
        #{ label: "Exit", glyph: "goal", tone: "goal" },
    ];
    routes(scene, frame)
}

// The route the history plans, has taken before this frame, and takes in it.
fn routes(scene, frame) {
    let planned = [];
    let traversed = [];
    let current = ();
    for (exchange, index) in frame.exchanges {
        let walked = route(exchange);
        if walked == () { continue; }
        if !planned.contains(walked) { planned.push(walked); }
        if index + 1 < frame.index && !traversed.contains(walked) { traversed.push(walked); }
        if index + 1 == frame.index { current = walked; }
    }
    let remaining = planned.some(|path| !traversed.contains(path) && path != current);
    if remaining {
        scene.legend.push(#{ label: "Planned next", glyph: "move", tone: "warning", path: "candidate" });
    }
    if traversed.len() > 0 {
        scene.legend.push(#{ label: "Traversed route", glyph: "move", tone: "active", path: "traversed" });
    }
    if current != () {
        scene.legend.push(#{ label: "Current move", glyph: "move", tone: "success", path: "current" });
    }
    for index in 0..scene.surface.edges.len() {
        let id = scene.surface.edges[index].id;
        let classes = scene.surface.edges[index].classes
            .filter(|class| class != "current" && class != "traversed" && class != "candidate");
        if current == id { classes.push("current"); }
        else if traversed.contains(id) { classes.push("traversed"); }
        else if planned.contains(id) { classes.push("candidate"); }
        scene.surface.edges[index].classes = classes;
    }
    for index in 0..scene.paths.len() {
        let id = scene.paths[index].id;
        let status = scene.paths[index].status;
        scene.paths[index].status = if current == id { "current" }
            else if traversed.contains(id) { "traversed" }
            else if planned.contains(id) { "candidate" }
            else if status == "blocked" { "blocked" }
            else { "available" };
    }
    scene
}
