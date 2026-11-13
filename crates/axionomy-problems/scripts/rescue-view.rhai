// Rescue: how people read the uncertain rescue. Labels and formats name its
// identifiers; the scene draws what the responder can see: the base and the
// sites the hidden prior spreads the survivor over, the responder, its
// beliefs, and Nature's hidden state.

// A location as people read it: `north` is `North`.
fn place(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return if name == "agent" { "Responder" } else { "Hidden scenario" };
    }
    if category == "role" {
        return if name == "actor" { "Responder" } else { "Nature" };
    }
    if category == "asset" {
        return switch name {
            "at" => `Responder at ${place(parts[2])}`,
            "energy" => "Energy left",
            "spent-energy" => "Energy spent",
            "sensor" => "Sensor available",
            "used-sensor" => "Sensor used",
            "unresolved" => "Location not yet drawn",
            "scenario-weight" => `Weight for ${place(parts[2])}, sensor state ${parts[3]}`,
            "truth" => `Survivor is at ${place(parts[2])}`,
            "seed" => `Sensor state ${parts[2]}`,
            "belief" => `Reading points to ${place(parts[2])}`,
            "planning" => "Choosing an action",
            "awaiting-observation" => "Waiting for a sensor reading",
            "committed" => "Destination chosen",
            "rescued" => "Survivor reached",
            "evacuated" => "Survivor evacuated",
            "solved" => "Rescue complete",
            _ => (),
        };
    }
    switch name {
        "instantiate" => `Draw scenario: ${place(parts[2])}, sensor state ${parts[3]}`,
        "begin-observe" => "Use the sensor",
        "resolve-observation" => `Receive sensor reading: ${place(parts[4])} (survivor at ${place(parts[2])}, sensor state ${parts[3]})`,
        "move" => `Move ${place(parts[2])} → ${place(parts[3])}`,
        "rescue" => `Reach survivor at ${place(parts[2])}`,
        "evacuate" => `Evacuate from ${place(parts[2])} to Base`,
        "finish" => "Complete the rescue",
        _ => (),
    }
}

fn format(asset) {
    if asset == "energy" || asset == "spent-energy" { #{ scale: 0, unit: "units" } } else { () }
}

// The sites the hidden prior spreads the survivor over.
fn sites() { ["north", "south", "east", "west", "harbor", "hills"] }

fn held(state, asset) {
    !state.balance("agent", asset).is_zero()
}

fn entity(key, label, glyph, node, role, tone, status) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: #{ kind: "graph_node", node: `location/${node}` },
        role: role, tone: tone, status: status,
        evidence: [], metrics: [],
    }
}

// The entity, showing the balance of `asset` the responder holds.
fn linked(entity, asset) {
    entity.account = "rescue/account/agent";
    entity.evidence = [#{ kind: "balance", account: "rescue/account/agent", asset: `rescue/asset/${asset}` }];
    entity
}

fn scene(state, frame) {
    let locations = [
        ["base", 300.0, 250.0], ["north", 120.0, 60.0], ["south", 480.0, 60.0], ["east", 570.0, 170.0],
        ["west", 30.0, 170.0], ["harbor", 120.0, 360.0], ["hills", 480.0, 360.0],
    ];
    let focus = ();
    for location in locations {
        if focus == () && held(state, `at/${location[0]}`) { focus = location[0]; }
    }
    if focus == () { throw "the responder remains at an encoded location"; }
    let nodes = locations.map(|location| #{
        id: #{ key: `location/${location[0]}`, label: place(location[0]) },
        classes: if location[0] == focus { ["current"] } else { [] },
        x: location[1], y: location[2],
    });
    let edges = sites().map(|site| #{
        id: `route/${site}`,
        source: "location/base",
        target: `location/${site}`,
        label: "hidden victim prior",
        classes: if site == focus { ["current", "uncertain"] } else { ["uncertain"] },
    });
    let energy = state.balance("agent", "energy");
    let agent = linked(entity(
        "agent/responder", "Responder", "agent", focus, "occupant", "active",
        if held(state, "awaiting-observation") { "awaiting observation" } else { "acting" },
    ), `at/${focus}`);
    agent.metrics = [#{ key: "energy", label: "Energy", value: energy.to_string(), unit: "units" }];
    let entities = [agent];
    for site in sites() {
        if held(state, `belief/${site}`) {
            entities.push(linked(entity(
                `belief/${site}`, `Belief: ${place(site)}`, "information", site, "state", "uncertain",
                "actor belief",
            ), `belief/${site}`));
        }
    }
    let nature = entity(
        "system/nature", "Hidden victim state", "information", "base", "context", "uncertain",
        "not visible to actor",
    );
    nature.account = "rescue/account/nature";
    nature.evidence = [#{ kind: "account", account: "rescue/account/nature" }];
    entities.push(nature);
    let scene = scene::entities(
        scene::graph("What the agent can see (the truth is hidden)", nodes, edges, `location/${focus}`),
        entities,
    );
    scene.metrics = [
        #{ key: "energy", label: "Energy remaining", value: energy.to_string(), unit: "units" },
        #{ key: "sensor", label: "Sensor charges", value: state.balance("agent", "sensor").to_string(), unit: () },
    ];
    scene
}
