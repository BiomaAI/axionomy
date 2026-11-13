// Mission's expedition: how people read the cooperative expedition. Labels
// and formats name its identifiers; the scene draws the sectors and the
// roads between them, each teammate where it stands and what it is doing,
// every objective the Scout has found, and each sector surveyed, hazardous
// or not.

// A name as people read it: `north` is `North`.
fn capital(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

// Even objectives are casualties, odd ones archives.
fn patient(id) { parse_int(id) % 2 == 0 }

// An objective as it reads inside a sentence.
fn objective(id) {
    if patient(id) { `Casualty ${parse_int(id) / 2 + 1}` } else { "the research archive" }
}

// An objective as it reads at the start of a sentence.
fn objective_subject(id) {
    if patient(id) { objective(id) } else { "Research archive" }
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return switch name {
            "world" => "Team clock, weather and supplies",
            "nature" => "Hidden mission sites and reports",
            "agent" => capital(parts[2]),
            "objective" => objective_subject(parts[2]),
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "world" => "Expedition",
            "scout" => "Scout",
            "medic" => "Medic",
            "nature" => "Nature",
            "objective" => "Objective",
            _ => (),
        };
    }
    if category == "asset" {
        return switch name {
            "world-identity" => "Expedition identity",
            "nature-identity" => "Nature identity",
            "agent-identity" => `${capital(parts[2])} identity`,
            "objective-identity" => `${objective_subject(parts[2])} identity`,
            "active" => "Mission in progress",
            "closed" => "Mission closed",
            "time" => "Mission time",
            "time-remaining" => "Time remaining",
            "horizon" => "Mission horizon",
            "count" => "Objectives in the mission",
            "resolved" => "Objectives resolved",
            "completed" => "Objectives completed",
            "failed-count" => "Objectives lost",
            "saved-vitality" => "Evacuated vitality",
            "permit" => `${capital(parts[2])} may act this tick`,
            "used-permit" => `${capital(parts[2])} has acted this tick`,
            "environment-ready" => "No weather change pending",
            "weather-due" => if parts[2] == "true" { "Hazard due to clear" } else { "Hazard due to arrive" },
            "hazard-site" => `Hazard site: ${capital(parts[2])}`,
            "weather-open" => `${capital(parts[2])} open`,
            "weather-blocked" => `${capital(parts[2])} blocked by the hazard`,
            "at" => `At ${capital(parts[2])}`,
            "travelling" => `Travelling to ${capital(parts[2])}`,
            "team-at" => `${capital(parts[2])} at ${capital(parts[3])}`,
            "team-travelling" => `${capital(parts[2])} travelling`,
            "ready" => "Ready",
            "ready-at" => "Busy until",
            "collecting" => `Recovering ${objective(parts[2])}`,
            "extracting" => `Extracting ${objective(parts[2])}`,
            "awaiting-report" => "Awaiting a sensor report",
            "energy" => "Energy",
            "spent-energy" => "Energy spent",
            "medical-kit" => "Medical kits",
            "used-medical-kit" => "Medical kits used",
            "health" => "Health",
            "harm" => "Health lost",
            "radio" => "Radio charges",
            "used-radio" => "Radio charges spent",
            "sensor" => "Sensor charges",
            "used-sensor" => "Sensor charges used",
            "scan-round" => `Sensor reports so far: ${parts[2]}`,
            "signal" => `Sensor report ${parse_int(parts[2]) + 1} will suggest ${capital(parts[3])}`,
            "hint" => `Sensor report ${parse_int(parts[2]) + 1} suggests ${capital(parts[3])}`,
            "unsearched" => `${capital(parts[2])} not yet surveyed`,
            "surveyed" => `${capital(parts[2])} surveyed`,
            "known" => `Knows ${objective(parts[2])} is at ${capital(parts[3])}`,
            "shared" => `Briefed Medic: ${objective(parts[2])} at ${capital(parts[3])}`,
            "truth" => `${objective_subject(parts[2])} is at ${capital(parts[3])}`,
            "deadline" => "Deadline",
            "hidden" => "Not yet found",
            "located" => "Located",
            "working" => "Recovery under way",
            "carried" => "Being carried",
            "complete" => "Delivered to base",
            "failed" => "Lost",
            "stable" => "Stabilized",
            "unstable" => "Needs stabilization",
            "clinical-due" => `Assessment due at tick ${parts[2]}`,
            "clinical-settled" => `Assessed at tick ${parts[2]}`,
            "next-clinical" => "Next casualty assessment",
            "clinical-count" => "Casualties assessed this round",
            "cargo" => `Carrying ${objective(parts[2])}`,
            "seat" => "Free evacuation seat",
            "reserved-seat" => `Seat reserved for ${objective(parts[2])}`,
            "supply" => "Resupplies left",
            "used-supply" => "Resupplies used",
            _ => (),
        };
    }
    switch name {
        "depart" => `${capital(parts[2])} starts travel ${capital(parts[3])} → ${capital(parts[4])}`,
        "arrive" => `${capital(parts[2])} arrives at ${capital(parts[3])}${
            if parts[4] == "true" { " and is injured by the hazard" } else { "" }
        }`,
        "survey" => `Scout surveys ${capital(parts[2])}`,
        "discover" => `Scout discovers ${objective(parts[2])} at ${capital(parts[3])}`,
        "scan" => "Scout requests a fallible sensor report",
        "report" => `Private sensor report ${parse_int(parts[2]) + 1} suggests ${capital(parts[3])}`,
        "share" => `Brief Medic on ${objective(parts[2])} at ${capital(parts[3])} ${
            if parts[4] == "true" { "by radio" } else { "at a rendezvous" }
        }`,
        "collect" => `Scout starts a three-tick recovery of ${objective(parts[2])} at ${capital(parts[3])}`,
        "finish-collect" => `Scout finishes recovery and carries ${objective(parts[2])}`,
        "treat-patient" => `Medic stabilizes ${objective(parts[2])} at ${capital(parts[3])}${
            if parts[4] == "true" { " and restores 1 health" } else { "" }
        }`,
        "extract" => `Scout and Medic begin a two-tick extraction of ${objective(parts[2])} at ${capital(parts[3])}`,
        "finish-extract" => `Joint extraction ends; Medic carries ${objective(parts[2])}`,
        "deliver" => `Deliver ${objective(parts[2])} safely to base`,
        "treat-agent" => if parts[2] == "medic" { `Medic treats their own injury at ${capital(parts[3])}` }
            else { `Medic treats Scout's injury at ${capital(parts[3])}` },
        "resupply" => `${capital(parts[2])} takes finite base energy${
            if parts[2] == "medic" { " and a medical kit" } else { "" }
        }`,
        "weather" => `${capital(parts[2])} ${
            if parts[3] == "true" { "reopens as the hazard clears" } else { "closes as the hazard arrives" }
        }`,
        "clinical" => if parts[4] == "true" { `${objective_subject(parts[2])} deteriorates at tick ${parts[3]}` }
            else { `${objective_subject(parts[2])} at tick ${parts[3]}: outcome already settled` },
        "finish-clinical" => `Finish the tick ${parts[2]} casualty assessment`,
        "fail" => `${objective_subject(parts[2])} ${
            if parts[4] == "true" { "dies" } else { "misses the mission deadline" }
        } ${
            switch parts[3] { "hidden" => "before discovery", "located" => "at its site", _ => "while carried" }
        }`,
        "tick" => `Advance mission time to tick ${parse_int(parts[2]) + 1}`,
        "close" => "Close the mission with every objective accounted for",
        _ => (),
    }
}

// The unit quantities of `asset` are read in, or nothing for a count.
fn unit(asset) {
    switch asset {
        "time" | "time-remaining" | "horizon" | "ready-at" | "deadline" | "next-clinical" => "ticks",
        "energy" | "spent-energy" => "units",
        _ => (),
    }
}

fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

// Every location, in the order the expedition lists them.
fn locations() { ["base", "north", "east", "south", "west"] }

fn val(state, account, asset) { state.balance(account, asset).to_int() }

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

// A metric showing `quantity` of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    metric(key, label, quantity, unit(asset))
}

// An entity shown as `account`, citing the account as its evidence.
fn entity(key, label, glyph, anchor, role, tone, status, account) {
    let account = `mission/expedition/account/${account}`;
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: anchor, role: role, tone: tone, status: status,
        account: account,
        evidence: [#{ kind: "account", account: account }],
        metrics: [],
    }
}

// The first location where `account` holds `<asset>/<location>`, if any.
fn found_at(state, account, asset) {
    for location in locations() {
        if val(state, account, `${asset}/${location}`) > 0 { return location; }
    }
    ()
}

// Whether `account` holds any of `<asset>/0`, `<asset>/1` and `<asset>/2`.
fn any_of(state, account, asset) {
    for id in 0..3 {
        if val(state, account, `${asset}/${id}`) > 0 { return true; }
    }
    false
}

fn at_node(location) {
    if location == () { #{ kind: "unanchored" } } else { #{ kind: "graph_node", node: location } }
}

// A teammate where it stands, what it is doing, and its supplies.
fn teammate(state, agent) {
    let own = `agent/${agent}`;
    let moving = found_at(state, own, "travelling");
    let at = found_at(state, own, "at");
    let harmed = val(state, own, "harm") > 0;
    let status = if moving != () {
        `Travelling to ${capital(moving)} · arrives ${val(state, own, "ready-at")}`
    } else if val(state, own, "awaiting-report") > 0 {
        "Awaiting private sensor report"
    } else if any_of(state, own, "extracting") {
        `Joint extraction until ${val(state, own, "ready-at")}`
    } else if any_of(state, own, "collecting") {
        `Archive recovery until ${val(state, own, "ready-at")}`
    } else if any_of(state, own, "cargo") {
        "Carrying objective to base"
    } else if harmed {
        "Ready · injury slows travel"
    } else {
        "Ready"
    };
    let drawn = entity(
        own, capital(agent), if agent == "scout" { "sensor" } else { "person" }, at_node(at),
        if at != () { "occupant" } else { "context" }, if harmed { "warning" } else { "active" },
        status, own,
    );
    let supply = if agent == "scout" { ["radio", "Radio charges"] } else { ["medical-kit", "Medical kits"] };
    drawn.metrics = [
        asset_metric("energy", "Energy", "energy", val(state, own, "energy")),
        // Each point of health lost adds a tick to every journey.
        metric("injury", "Injury delay", val(state, own, "harm"), "ticks"),
        asset_metric("capacity", supply[1], supply[0], val(state, own, supply[0])),
    ];
    drawn
}

// An objective the Scout has found at `site`: where it is, and how its
// recovery stands.
fn found_objective(state, id, site) {
    let account = `objective/${id}`;
    let complete = val(state, account, "complete") > 0;
    let failed = val(state, account, "failed") > 0;
    let carrying = val(state, account, "carried") > 0;
    let casualty = id % 2 == 0;
    let node = if complete {
        "base"
    } else if carrying {
        found_at(state, if casualty { "agent/medic" } else { "agent/scout" }, "at")
    } else {
        site
    };
    let status = if complete { "Delivered at base" }
        else if failed { "Objective lost" }
        else if carrying { "Carried by teammate" }
        else if val(state, account, "working") > 0 { "Recovery in progress" }
        else if casualty && val(state, account, "stable") == 0 { "Needs stabilization" }
        else { "Ready for recovery" };
    let drawn = entity(
        account, if casualty { `Casualty ${id / 2 + 1}` } else { "Research archive" },
        if casualty { "person" } else { "information" }, at_node(node),
        if node != () { "attachment" } else { "context" },
        if complete { "success" } else if failed { "danger" } else { "warning" },
        status, account,
    );
    drawn.metrics = [
        metric("knowledge", "Medic briefed",
            if val(state, "agent/medic", `known/${id}/${site}`) > 0 { "yes" } else { "no" }, ()),
        asset_metric("deadline", "Deadline", "deadline", val(state, account, "deadline")),
    ];
    if casualty {
        drawn.metrics.insert(0, metric("health", "Vitality", val(state, account, "health"), "/ 3"));
    }
    drawn
}

fn scene(state, frame) {
    let nodes = [];
    for spot in [
        ["base", 400.0, 300.0], ["north", 140.0, 100.0], ["east", 730.0, 100.0],
        ["south", 730.0, 520.0], ["west", 140.0, 520.0],
    ] {
        nodes.push(#{
            id: #{ key: spot[0], label: capital(spot[0]) },
            classes: ["location"],
            x: spot[1], y: spot[2],
        });
    }
    let edges = [];
    for road in [
        ["base", "north"], ["base", "south"], ["north", "east"], ["east", "south"],
        ["south", "west"], ["west", "north"],
    ] {
        edges.push(#{
            id: `road/${road[0]}/${road[1]}`, source: road[0], target: road[1],
            label: "1 tick + injury delay", classes: [],
        });
    }
    let entities = [teammate(state, "scout"), teammate(state, "medic")];
    for id in 0..val(state, "world", "count") {
        let site = found_at(state, "agent/scout", `known/${id}`);
        if site == () { continue; }
        entities.push(found_objective(state, id, site));
    }
    let busy = 0;
    for agent in ["scout", "medic"] {
        if val(state, `agent/${agent}`, "ready") == 0 { busy += 1; }
    }
    let scene = scene::entities(
        scene::graph("Independent agents, scarce communication and evolving hazards", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        asset_metric("time", "Mission time", "time", val(state, "world", "time")),
        asset_metric("complete", "Objectives complete", "completed", val(state, "world", "completed")),
        asset_metric("failed", "Objectives lost", "failed-count", val(state, "world", "failed-count")),
        metric("busy", "Teammates occupied", busy, "/ 2"),
        asset_metric("radio", "Radio charges spent", "used-radio", val(state, "agent/scout", "used-radio")),
    ];
    for location in locations() {
        for index in 0..scene.entities.len() {
            if scene.entities[index].id.key != location { continue; }
            let blocked = val(state, "world", `weather-blocked/${location}`) > 0;
            scene.entities[index].status = if blocked { "Hazard · departures toward this sector blocked" }
                else if location == "base" { "Hospital, archive delivery and finite resupply" }
                else if val(state, "agent/scout", `surveyed/${location}`) > 0 { "Surveyed" }
                else { "Unsearched" };
            if blocked { scene.entities[index].tone = "danger"; }
            break;
        }
    }
    scene
}
