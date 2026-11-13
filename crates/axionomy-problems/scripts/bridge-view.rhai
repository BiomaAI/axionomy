// Bridge: how people read two travellers sharing one lane. Labels and
// formats name its identifiers; the scene draws the banks, the bridge and,
// in timed traffic, the paid detour, where each traveller stands and why,
// and the lane's capacity, priorities, clock and trips.

// A name as people read it: `agent-a` is `Agent A`.
fn words(name) {
    let parts = [];
    for part in name.split("-") {
        parts.push(part.sub_string(0, 1).to_upper() + part.sub_string(1));
    }
    parts.reduce(|sum, part| if sum == () { part } else { sum + " " + part })
}

// A traveller, `a` or `b`, as people read it.
fn agent(name) {
    `Agent ${name.to_upper()}`
}

fn asset_label(name, parts) {
    switch name {
        "agent-identity" => `Identity: ${agent(parts[2])}`,
        "bridge-identity" => "Bridge identity",
        "at" => if parts[2] == "west" { "At the west bank" } else { "At the east bank" },
        "energy" => "Crossing energy",
        "spent-energy" => "Energy spent crossing",
        "credit" => "Credit available",
        "escrow" => "Credit in escrow",
        "spent-credit" => "Credit spent",
        "bid" => if parts[2] == "1" { "Bid of 1 credit" } else { `Bid of ${parts[2]} credits` },
        "can-bid" => "Can bid or claim",
        "submitted" => "Bid submitted",
        "crossing-right" => "Crossing right",
        "first-turn" => "First crossing open",
        "second-turn" => "Second crossing open",
        "priority-benefit" => "Priority crossings won",
        "waiting" => "Waiting for the lane",
        "crossed" => "Crossed this round",
        "completed-trip" => "Completed prior rounds",
        "rounds-remaining" => "Rounds remaining after this one",
        "capacity-free" => "Lane available",
        "active" => "Allocation in progress",
        "solved" => "Allocation complete",
        "now" => "World time",
        "time-remaining" => "Time remaining",
        "arrival" => "Arrival time",
        "deadline" => "Arrival deadline",
        "lease-until" => "Reservation valid through",
        "ready-at" => "Travel completes at",
        "in-transit" => "On the bridge",
        "on-detour" => "On the alternate route",
        "detouring" => "Detouring",
        "on-time" => "On-time trips",
        "late" => "Late trips",
        "missed-reservation" => "Expired reservations",
        "round" => "Prior traffic waves",
        _ => (),
    }
}

fn rate_label(name, parts) {
    switch name {
        "submit-bid" => `${agent(parts[2])} bids ${parts[3]}`,
        "resolve" => `Settle auction: ${agent(parts[2])} wins ${parts[3]} to ${parts[4]}`,
        "claim-first" => `${agent(parts[2])} claims first crossing`,
        "claim-second" => `${agent(parts[2])} claims second crossing`,
        "yield-to-waiting" => `Grant waiting ${agent(parts[2])} the lane`,
        "cross" => `${agent(parts[2])} crosses`,
        "reset-round" => "Start the next allocation round",
        "finish" => "Complete all crossing rounds",
        "advance" => `Advance world time to ${parts[2]}`,
        "complete" => `${agent(parts[2])} finishes ${if parts[3] == "true" { "detour" } else { "crossing" }} ${if parts[4] == "true" { "late" } else { "on time" }}`,
        "detour" => if parts[3] == "true" {
            `${agent(parts[2])} stops waiting and pays for the alternate route`
        } else {
            `${agent(parts[2])} skips the lane and pays for the alternate route`
        },
        "expire-reservation" => `Release ${agent(parts[2])}'s expired reservation`,
        "withdraw-bid" => `${agent(parts[2])} withdraws bid ${parts[3]}`,
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => if parts[1] == "agent" { agent(parts[2]) } else { "Single-lane bridge" },
        "asset" => asset_label(parts[1], parts),
        "rate" => rate_label(parts[1], parts),
        _ => words(parts[1]),
    }
}

// Energy reads in units and credit in credits; every time the bridge keeps
// is a tick of its world clock.
fn format(asset) {
    switch asset.split("/")[0] {
        "energy" | "spent-energy" => #{ scale: 0, unit: "units" },
        "credit" | "escrow" | "spent-credit" => #{ scale: 0, unit: "credits" },
        "now" | "time-remaining" | "arrival" | "deadline" | "lease-until" | "ready-at" =>
            #{ scale: 0, unit: "ticks" },
        _ => (),
    }
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

// A traveller, where it stands and why, showing the balance that places it.
fn traveler(state, name, timed, clock) {
    let account = `agent/${name}`;
    let east = state.balance(account, "at/east") > 0;
    let crossing = state.balance(account, "in-transit") > 0;
    let detour = state.balance(account, "on-detour") > 0;
    let place = if crossing {
        ["bridge", "in-transit", `crossing until ${state.balance(account, "ready-at")}`]
    } else if detour {
        ["detour", "on-detour", `detouring until ${state.balance(account, "ready-at")}`]
    } else if east {
        ["east", "at/east", "arrived"]
    } else if timed && clock < state.balance(account, "arrival") {
        ["west", "at/west", `arrives at ${state.balance(account, "arrival")}`]
    } else if state.balance(account, "crossing-right") > 0 {
        ["west", "at/west", `reserved through ${state.balance(account, "lease-until")}`]
    } else if state.balance(account, "submitted") > 0 {
        ["west", "at/west", "bid in escrow"]
    } else {
        ["west", "at/west", "waiting"]
    };
    let metrics = [metric("credit", "Credit", state.balance(account, "credit"), "credits")];
    if timed {
        metrics += [
            metric("deadline", "Deadline", state.balance(account, "deadline"), "ticks"),
            metric("ontime", "On-time trips", state.balance(account, "on-time"), ()),
            metric("late", "Late trips", state.balance(account, "late"), ()),
        ];
    }
    #{
        id: #{ key: `traveler/${name}`, label: agent(name) },
        glyph: "agent",
        anchor: #{ kind: "graph_node", node: place[0] },
        role: "occupant",
        tone: if east { "success" } else { "active" },
        status: place[2],
        account: `bridge/account/${account}`,
        evidence: [#{ kind: "balance", account: `bridge/account/${account}`, asset: `bridge/asset/${place[1]}` }],
        metrics: metrics,
    }
}

fn scene(state, frame) {
    let timed = "advance/1" in state.rates("advance/");
    let clock = state.balance("bridge", "now");
    let places = [
        ["west", "West bank", 70.0, 100.0],
        ["bridge", "Capacity-one bridge", 320.0, 100.0],
        ["east", "East bank", 570.0, 100.0],
    ];
    if timed {
        places.push(["detour", "Paid alternate route", 320.0, 300.0]);
    }
    let nodes = places.map(|place| #{
        id: #{ key: place[0], label: place[1] },
        classes: if place[0] == "bridge" { ["resource"] } else { [] },
        x: place[2],
        y: place[3],
    });
    let routes = [
        ["west", "bridge", "reserve lane"],
        ["bridge", "east", if timed { "A: 3 ticks · B: 2 ticks" } else { "cross atomically" }],
    ];
    if timed {
        routes += [
            ["west", "detour", "1 credit"],
            ["detour", "east", "3 ticks · independent capacity"],
        ];
    }
    let edges = routes.map(|route| #{
        id: `route/${route[0]}/${route[1]}`,
        source: route[0],
        target: route[1],
        label: route[2],
        classes: [],
    });
    let travelers = [];
    for name in ["a", "b"] {
        travelers.push(traveler(state, name, timed, clock));
    }
    let scene = scene::entities(
        scene::graph("Timed traffic and one shared lane", nodes, edges, ()),
        travelers,
    );
    scene.metrics = [
        metric(
            "capacity", "Bridge capacity",
            if state.balance("bridge", "capacity-free").is_zero() { "reserved / occupied" } else { "available" },
            (),
        ),
        metric("priority-a", "A priority", state.balance("agent/a", "priority-benefit"), ()),
        metric("priority-b", "B priority", state.balance("agent/b", "priority-benefit"), ()),
    ];
    if timed {
        scene.metrics += [
            metric("world-time", "World time", clock, "ticks"),
            metric("wave", "Traffic wave", state.balance("bridge", "round") + 1, ()),
            metric("ontime", "On-time trips", state.balance("agent/a", "on-time") + state.balance("agent/b", "on-time"), ()),
            metric("late", "Late trips", state.balance("agent/a", "late") + state.balance("agent/b", "late"), ()),
        ];
    }
    scene
}
