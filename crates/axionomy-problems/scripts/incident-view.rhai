// Rescue's incident: how people read the multi-patient incident. Labels and
// formats name its identifiers; the scene draws the incident's roads and
// sites, the evacuation crew, and every casualty someone has found.

// A location as people read it: `north` is `North`.
fn place(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

// A patient as people read it, counting from one.
fn patient(id) {
    `Patient ${parse_int(id) + 1}`
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return switch name {
            "world" => "Incident clock and hospital",
            "responder" => "Evacuation crew",
            "nature" => "Hidden patient locations and sensor reports",
            "patient" => patient(parts[2]),
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "world" => "Incident",
            "responder" => "Crew",
            "nature" => "Nature",
            "patient" => "Patient",
            _ => (),
        };
    }
    if category == "asset" {
        return switch name {
            "world-identity" => "Incident identity",
            "responder-identity" => "Crew identity",
            "nature-identity" => "Nature identity",
            "patient-identity" => `${patient(parts[2])} identity`,
            "active" => "Incident in progress",
            "closed" => "Incident closed",
            "time" => "Incident time",
            "horizon" => "Incident horizon",
            "time-remaining" => "Time remaining",
            "patient-count" => "Patients in the incident",
            "resolved" => "Outcomes recorded",
            "saved-count" => "Patients evacuated",
            "death-count" => "Patients who died",
            "left-count" => "Patients left behind",
            "saved-vitality" => "Vitality at hospital arrival",
            "at" => `At ${place(parts[2])}`,
            "site" => `Incident site: ${place(parts[2])}`,
            "surveyed" => `${place(parts[2])} searched`,
            "unsearched" => `${place(parts[2])} not yet searched`,
            "known" => `${patient(parts[2])} found at ${place(parts[3])}`,
            "truth" => `${patient(parts[2])} is at ${place(parts[3])}`,
            "ready" => "Crew ready",
            "awaiting-scan" => "Awaiting a sensor report",
            "sensor" => "Sensor charges",
            "used-sensor" => "Sensor charges used",
            "scan-round" => `Sensor reports so far: ${parts[2]}`,
            "signal" => `Sensor report ${parse_int(parts[2]) + 1} will suggest ${place(parts[3])}`,
            "hint" => `Sensor hint: ${place(parts[2])}`,
            "fuel" => "Fuel",
            "spent-fuel" => "Fuel spent",
            "energy" => "Energy",
            "spent-energy" => "Energy spent",
            "medical-kit" => "Medical kits",
            "used-medical-kit" => "Medical kits used",
            "supply-available" => "Resupply available",
            "supply-used" => "Resupply used",
            "seat" => "Free evacuation seat",
            "carrying" => `Carrying ${patient(parts[2])}`,
            "hidden" => "Not yet found",
            "found" => "Found",
            "onboard" => "On board",
            "alive" => "Alive",
            "saved" => "Evacuated",
            "dead" => "Died",
            "left-behind" => "Left behind",
            "vitality" => "Vitality",
            "harm" => "Vitality lost",
            "next-decline" => "Next deterioration",
            "deadline" => "Evacuation deadline",
            "treatable" => "Can be stabilized",
            "stabilized" => "Stabilized",
            _ => (),
        };
    }
    switch name {
        "drive" => `Drive ${place(parts[2])} → ${place(parts[3])}${
            if parts[4] == "true" { " at emergency speed" } else { " conserving fuel" }
        }`,
        "search" => `Search ${place(parts[2])} for casualties`,
        "scan" => "Spend a sensor charge on a new location hint",
        "report" => `Sensor report ${parse_int(parts[2]) + 1} suggests ${place(parts[3])}; hint may be wrong`,
        "discover" => `Discover ${patient(parts[2])} at ${place(parts[3])}`,
        "deteriorate" => `${patient(parts[2])} loses vitality`,
        "stabilize" => `Stabilize ${patient(parts[2])} at ${place(parts[3])}, ${
            if parts[4] == "true" { "restoring 1 vitality and extending the window" }
            else { "extending the evacuation window" }
        }`,
        "load" => `Load ${patient(parts[2])} at ${place(parts[3])} into the evacuation seat`,
        "unload" => `Admit ${patient(parts[2])} to the hospital`,
        "lose" => `${patient(parts[2])} ${
            if parts[4] == "horizon" { "is left behind" } else { "dies" }
        } ${
            switch parts[3] { "hidden" => "unfound", "found" => "at the scene", _ => "on board" }
        }: ${
            switch parts[4] {
                "deterioration" => "no vitality left",
                "deadline" => "deadline passed",
                _ => "incident over",
            }
        }`,
        "resupply" => "Take the hospital's finite fuel, energy and medical reserve",
        "wait" => "Advance incident time by one tick",
        "close" => "Close the incident with all patient outcomes recorded",
        _ => (),
    }
}

// The unit quantities of `asset` are read in, or nothing for a count.
fn unit(asset) {
    switch asset {
        "time" | "horizon" | "time-remaining" | "next-decline" | "deadline" => "ticks",
        "fuel" | "spent-fuel" | "energy" | "spent-energy" => "units",
        _ => (),
    }
}

fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

// Every location, in the order the incident lists them.
fn locations() { ["base", "north", "south", "east", "harbor", "west"] }

fn value(state, account, asset) { state.balance(account, asset).to_int() }

// A metric showing `quantity` of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    #{ key: key, label: label, value: quantity.to_string(), unit: unit(asset) }
}

fn entity(key, label, glyph, node, role, tone, status, account) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: #{ kind: "graph_node", node: node },
        role: role, tone: tone, status: status,
        account: `rescue/incident/account/${account}`,
        evidence: [#{ kind: "account", account: `rescue/incident/account/${account}` }],
        metrics: [],
    }
}

// Whether the incident takes place at `location`: the base, or a site.
fn present(state, location) {
    location == "base" || value(state, "world", `site/${location}`) > 0
}

fn scene(state, frame) {
    let nodes = [];
    for spot in [
        ["base", 420.0, 280.0], ["north", 120.0, 80.0], ["east", 720.0, 80.0],
        ["harbor", 720.0, 480.0], ["south", 120.0, 480.0], ["west", 420.0, 650.0],
    ] {
        if !present(state, spot[0]) { continue; }
        nodes.push(#{
            id: #{ key: spot[0], label: if spot[0] == "base" { "Base hospital" } else { place(spot[0]) } },
            classes: ["location"],
            x: spot[1], y: spot[2],
        });
    }
    let edges = [];
    for road in [
        ["base", "north"], ["base", "south"], ["base", "harbor"], ["north", "east"],
        ["east", "harbor"], ["south", "west"], ["west", "harbor"],
    ] {
        if !present(state, road[0]) || !present(state, road[1]) { continue; }
        edges.push(#{
            id: `road/${road[0]}/${road[1]}`, source: road[0], target: road[1],
            label: "2 ticks / 1 fuel · fast 1 / 2", classes: [],
        });
    }
    let at = "base";
    for location in locations() {
        if value(state, "responder", `at/${location}`) > 0 { at = location; break; }
    }
    let crew = entity(
        "crew", "Evacuation crew", "vehicle", at, "occupant", "active",
        if value(state, "responder", "seat") == 0 { "Transporting one patient" }
        else if value(state, "responder", "awaiting-scan") > 0 { "Awaiting sensor report" }
        else { "Evacuation seat available" },
        "responder",
    );
    crew.metrics = [
        asset_metric("fuel", "Fuel", "fuel", state.balance("responder", "fuel")),
        asset_metric("energy", "Crew energy", "energy", state.balance("responder", "energy")),
        asset_metric("medicine", "Medical kits", "medical-kit", state.balance("responder", "medical-kit")),
    ];
    let entities = [crew];
    for id in 0..value(state, "world", "patient-count") {
        let site = ();
        for location in locations() {
            if value(state, "world", `known/${id}/${location}`) > 0 { site = location; break; }
        }
        if site == () { continue; }
        let account = `patient/${id}`;
        let saved = value(state, account, "saved") > 0;
        let dead = value(state, account, "dead") > 0;
        let onboard = value(state, account, "onboard") > 0;
        let casualty = entity(
            `patient/${id}`, `Patient ${id + 1}`, "person",
            if saved { "base" } else if onboard { at } else { site },
            "attachment",
            if saved { "success" }
            else if dead || value(state, account, "vitality") <= 1 { "danger" }
            else { "warning" },
            if saved { "Evacuated" }
            else if dead { "Died" }
            else if value(state, account, "left-behind") > 0 { "Left behind" }
            else if onboard { "In transit" }
            else if value(state, account, "stabilized") > 0 { "Stabilized · awaiting transport" }
            else { "Found · awaiting care" },
            account,
        );
        casualty.metrics = [
            #{ key: "vitality", label: "Vitality", value: state.balance(account, "vitality").to_string(), unit: "/ 3" },
            asset_metric("decline", "Next deterioration", "next-decline", state.balance(account, "next-decline")),
            asset_metric("deadline", "Evacuation deadline", "deadline", state.balance(account, "deadline")),
        ];
        entities.push(casualty);
    }
    let now = state.balance("world", "time");
    let saved = state.balance("world", "saved-count");
    let deaths = state.balance("world", "death-count");
    let scene = scene::entities(
        scene::graph("Rescue incident · physical roads and observed casualties", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        asset_metric("time", "Incident time", "time", now),
        asset_metric("saved", "Patients evacuated", "saved-count", saved),
        asset_metric("deaths", "Patients lost", "death-count", deaths),
        asset_metric("fuel", "Fuel remaining", "fuel", state.balance("responder", "fuel")),
    ];
    for location in locations() {
        if !present(state, location) { continue; }
        for index in 0..scene.entities.len() {
            if scene.entities[index].id.key != location { continue; }
            scene.entities[index].status = if location == "base" {
                if value(state, "world", "closed") > 0 { "Incident closed" } else { "Hospital and one finite resupply" }
            } else if value(state, "responder", `surveyed/${location}`) > 0 { "Searched" }
            else if value(state, "responder", `hint/${location}`) > 0 { "Unverified sensor hint" }
            else { "Unsearched" };
            if location == "base" {
                scene.entities[index].metrics = [
                    asset_metric("time", "Incident time", "time", now),
                    asset_metric("saved", "Patients evacuated", "saved-count", saved),
                    asset_metric("lost", "Patients lost", "death-count", deaths),
                ];
            }
            break;
        }
    }
    scene
}
