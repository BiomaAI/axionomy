// Mission: how people read the hidden-information mission. Labels and
// formats name its identifiers; the scene draws the base and the two sites
// the victim may be at, where each agent stands and what it knows, mission
// control, Nature's hidden scenario and the mission's outcome.

// A location as people read it: `north` is `North`.
fn place(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

// A hazard as people read it.
fn danger(name) {
    if name == "safe" { "no hazard" } else { "injury hazard" }
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return switch name {
            "agent" => place(parts[2]),
            "nature" => "Hidden mission scenario",
            "mission" => "Mission clock and state",
            "success" => "Mission objective",
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "scout" => "Scout",
            "medic" => "Medic",
            "nature" => "Nature",
            "mission" => "Mission",
            "goal" => "Goal",
            _ => (),
        };
    }
    if category == "asset" {
        return switch name {
            "agent-identity" => `Identity: ${place(parts[2])}`,
            "nature-identity" => "Nature identity",
            "mission-identity" => "Mission identity",
            "success-identity" => "Objective identity",
            "at" => `At ${place(parts[2])}`,
            "energy" => "Energy left",
            "spent-energy" => "Energy spent",
            "sensor" => "Sensor available",
            "used-sensor" => "Sensor used",
            "medical-kit" => "Medical kit available",
            "used-medical-kit" => "Medical kit used",
            "intel" => `Private sighting: ${place(parts[2])}`,
            "shared-intel" => `Shared sighting: ${place(parts[2])}`,
            "injured" => "Agent injured",
            "unresolved" => "Scenario not yet drawn",
            "scenario-weight" => `Odds: objective at ${place(parts[2])}, sensor state ${parts[3]}, ${danger(parts[4])}`,
            "truth" => `Objective at ${place(parts[2])}`,
            "seed" => `Sensor state ${parts[2]}`,
            "hazard" => `Drawn: ${danger(parts[2])}`,
            "hazard-resolved" => "Hazard resolved",
            "time-remaining" => "Time left",
            "elapsed-time" => "Elapsed time",
            "planning" => "Choosing an action",
            "awaiting-scan" => "Waiting for a sighting",
            "awaiting-encounter" => "Waiting for a hazard outcome",
            "needs-treatment" => "Treatment required",
            "area-safe" => "Area made safe",
            "rescued" => "Objective rescued",
            "solved" => "Mission complete",
            _ => (),
        };
    }
    switch name {
        "instantiate" => `Draw scenario: objective at ${place(parts[2])}, sensor state ${parts[3]}, ${danger(parts[4])}`,
        "begin-scan" => "Scout scans the area",
        "resolve-scan" => `Scout sees ${place(parts[4])} (objective at ${place(parts[2])}, sensor state ${parts[3]})`,
        "share" => `Scout shares sighting: ${place(parts[2])}`,
        "move-together" => `Scout and Medic move to ${place(parts[2])}`,
        "move-direct" => `Move directly to ${place(parts[2])}`,
        "encounter" => if parts[3] == "safe" { `${place(parts[2])} turns out safe` }
            else { `Hazard at ${place(parts[2])} injures the Scout` },
        "treat" => "Medic treats the injury",
        "rescue" => `Rescue objective at ${place(parts[2])}`,
        "finish" => "Complete the mission",
        _ => (),
    }
}

// The unit quantities of `asset` are read in, or nothing for a count.
fn unit(asset) {
    switch asset {
        "energy" | "spent-energy" => "units",
        "time-remaining" | "elapsed-time" => "ticks",
        _ => (),
    }
}

fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

fn held(state, account, asset) {
    !state.balance(account, asset).is_zero()
}

// Whether either agent stands at `location`.
fn occupied(state, location) {
    held(state, "agent/scout", `at/${location}`) || held(state, "agent/medic", `at/${location}`)
}

fn entity(key, label, glyph, anchor, role, tone, status, account) {
    let account = `mission/account/${account}`;
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: anchor, role: role, tone: tone, status: status,
        account: account,
        evidence: [#{ kind: "account", account: account }],
        metrics: [],
    }
}

// Where an agent stands, what it saw or was told, and its energy.
fn agent_entity(state, agent) {
    let own = `agent/${agent}`;
    let location = ();
    for spot in ["base", "north", "south"] {
        if held(state, own, `at/${spot}`) { location = spot; break; }
    }
    if location == () { throw "mission agents remain at an encoded location"; }
    let known = ();
    for site in ["north", "south"] {
        if held(state, own, `shared-intel/${site}`) {
            known = `shared ${place(site)}`;
            break;
        }
        if held(state, own, `intel/${site}`) {
            known = `${if agent == "scout" { "saw" } else { "briefed" }} ${place(site)}`;
            break;
        }
    }
    let drawn = entity(
        own, place(agent), if agent == "scout" { "sensor" } else { "agent" },
        #{ kind: "graph_node", node: `location/${location}` }, "occupant", "active",
        if held(state, own, "injured") { "injured" } else if known == () { "ready" } else { known },
        own,
    );
    drawn.evidence = [#{
        kind: "balance", account: drawn.account, asset: `mission/asset/at/${location}`,
    }];
    drawn.metrics = [#{
        key: `energy/${agent}`, label: "Energy",
        value: state.balance(own, "energy").to_string(), unit: "units",
    }];
    drawn
}

fn scene(state, frame) {
    let nodes = [];
    for spot in [["base", 310.0, 250.0], ["north", 110.0, 60.0], ["south", 510.0, 60.0]] {
        nodes.push(#{
            id: #{ key: `location/${spot[0]}`, label: place(spot[0]) },
            classes: if occupied(state, spot[0]) { ["current"] } else { [] },
            x: spot[1], y: spot[2],
        });
    }
    let edges = [];
    for site in ["north", "south"] {
        edges.push(#{
            id: `mission/${site}`,
            source: "location/base",
            target: `location/${site}`,
            label: "hidden truth + hazard",
            classes: if occupied(state, site) { ["current", "uncertain"] } else { ["uncertain"] },
        });
    }
    let entities = [agent_entity(state, "scout"), agent_entity(state, "medic")];
    entities.push(entity(
        "system/mission", "Mission control", "task",
        #{ kind: "graph_node", node: "location/base" }, "state", "neutral", "shared state", "mission",
    ));
    entities.push(entity(
        "system/nature", "Hidden scenario", "weather", #{ kind: "unanchored" }, "context",
        "uncertain", "private truth", "nature",
    ));
    entities.push(entity(
        "system/success", "Mission outcome", "goal", #{ kind: "unanchored" }, "context", "goal",
        if state.balance("success", "solved") >= 1 { "complete" } else { "rescue objective" },
        "success",
    ));
    let reports = 0;
    for site in ["north", "south"] {
        if held(state, "agent/scout", `shared-intel/${site}`) { reports += 1; }
    }
    let scene = scene::entities(
        scene::graph("Where each agent is, and what it knows", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        #{
            key: "time", label: "Mission time remaining",
            value: state.balance("mission", "time-remaining").to_string(), unit: "ticks",
        },
        #{ key: "shared-intel", label: "Shared intel", value: reports.to_string(), unit: "reports" },
    ];
    scene
}
