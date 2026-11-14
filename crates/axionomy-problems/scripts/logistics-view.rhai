// Logistics' Micro: how people read one vehicle carrying orders from the
// depot to a customer. Labels and formats name its identifiers; the scene
// draws the route network, the direct road and the safe road through the
// junction, where the vehicle is or which leg it travels, each order
// waiting, aboard or delivered, the fuel station and the weather.

import "logistics" as logistics;

// A name as people read it: `depot` is `Depot`.
fn capital(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

// Where a route starts and ends.
fn route_ends(route) {
    switch route {
        "direct-out" => ["depot", "customer"],
        "direct-back" => ["customer", "depot"],
        "safe-out-first" => ["depot", "junction"],
        "safe-out-second" => ["junction", "customer"],
        "safe-back-first" => ["customer", "junction"],
        "safe-back-second" => ["junction", "depot"],
    }
}

// A route as people read it: whether it is the risky direct road or a leg
// of the safe road through the junction, and where it runs.
fn road(route) {
    let ends = route_ends(route);
    let kind = if route == "direct-out" || route == "direct-back" { "direct" } else { "safe" };
    `${kind} road ${capital(ends[0])} → ${capital(ends[1])}`
}

fn travel(outcome) {
    switch outcome {
        "clear" => "a clear trip",
        "delayed" => "a delay",
        "breakdown" => "a breakdown",
    }
}

fn order_label(order) {
    `Order ${order.to_upper()}`
}

fn asset_label(parts) {
    switch parts[1] {
        "at" => `Vehicle at ${capital(parts[2])}`,
        "traveling" => `Traveling the ${road(parts[2])}`,
        "fuel" => "Fuel left",
        "spent-fuel" => "Fuel spent",
        "money" => "Money",
        "time-remaining" => "Time left",
        "elapsed-time" => "Elapsed time",
        "cargo-space" => "Cargo space available",
        "cargo-occupied" => "Cargo space occupied",
        "repair-tool" => "Repair tool",
        "waiting" => "Order waiting at the depot",
        "in-transit" => "Order in transit",
        "delivered" => "Order delivered",
        "package" => `Package for ${order_label(parts[2])}`,
        "weather-ready" => "Ready to draw travel conditions",
        "outcome-weight" => `Odds of ${travel(parts[3])} on the ${road(parts[2])}`,
        "outcome" => `Drawn: ${travel(parts[3])} on the ${road(parts[2])}`,
        _ => (),
    }
}

fn rate_label(parts) {
    switch parts[1] {
        "load" => `Load ${order_label(parts[2])}`,
        "depart" => `Depart on the ${road(parts[2])}`,
        "resolve" => `Draw ${travel(parts[3])} on the ${road(parts[2])}`,
        "arrive" => `Arrive via the ${road(parts[2])} after ${travel(parts[3])}`,
        "repair" => `Repair a breakdown on the ${road(parts[2])}`,
        "deliver" => `Deliver ${order_label(parts[2])}`,
        "refuel" => "Refuel the vehicle",
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "vehicle" => "Delivery vehicle",
            "order" => order_label(parts[2]),
            "nature" => "Weather and breakdowns",
            "fuel-station" => "Fuel station",
            _ => (),
        },
        "asset" => asset_label(parts),
        "rate" => rate_label(parts),
        "role" => switch parts[1] {
            "vehicle" => "Vehicle",
            "order" => "Order",
            "nature" => "Nature",
            "fuel-station" => "Fuel station",
            _ => (),
        },
        _ => (),
    }
}

// The unit of an asset's quantities, if it has one.
fn unit(asset) {
    switch asset {
        "fuel" | "spent-fuel" => "units",
        "money" => "credits",
        "time-remaining" | "elapsed-time" => "ticks",
        _ => (),
    }
}

// Fuel reads in units, money in credits and time in ticks.
fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

// A metric showing a quantity of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    metric(key, label, quantity, unit(asset))
}

fn entity(key, label, glyph, anchor, role, tone, status) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph,
        anchor: anchor,
        role: role,
        tone: tone,
        status: status,
    }
}

// `entity` shown as `account`, citing the account as its evidence.
fn with_account(entity, account) {
    let key = `logistics/account/${account}`;
    entity.account = key;
    entity.evidence = [#{ kind: "account", account: key }];
    entity
}

fn at_location(location) {
    #{ kind: "graph_node", node: `location/${location}` }
}

fn scene(state, frame) {
    // The direct road runs straight from the depot to the customer; the safe
    // road dips through the junction below it, so neither hides the other.
    let locations = [["depot", 70.0, 150.0], ["junction", 320.0, 330.0], ["customer", 570.0, 150.0]];
    let focus = logistics::location(state);
    let traveling = logistics::active_route(state);
    let nodes = locations.map(|location| #{
        id: #{ key: `location/${location[0]}`, label: capital(location[0]) },
        classes: if focus == location[0] { ["current"] }
            else if location[0] == "customer" { ["goal"] }
            else { [] },
        x: location[1],
        y: location[2],
    });
    let edges = [];
    for route in logistics::routes() {
        let ends = route_ends(route);
        edges.push(#{
            id: `route/${route}`,
            source: `location/${ends[0]}`,
            target: `location/${ends[1]}`,
            label: switch route {
                "direct-out" => "Direct, outbound",
                "direct-back" => "Direct, return",
                "safe-out-first" | "safe-out-second" => "Safe, outbound",
                _ => "Safe, return",
            },
            classes: if route == traveling { ["current", "uncertain"] } else { ["uncertain"] },
        });
    }

    let vehicle_anchor = if focus != () {
        at_location(focus)
    } else if traveling != () {
        #{ kind: "graph_edge", edge: `route/${traveling}`, progress: 0.5 }
    } else {
        #{ kind: "unanchored" }
    };
    let vehicle_asset = if focus != () { `at/${focus}` } else { `traveling/${traveling}` };
    let vehicle = entity(
        "vehicle/fleet-1", "Vehicle 1", "vehicle", vehicle_anchor, "occupant", "active",
        if traveling != () { `traveling the ${road(traveling)}` } else { () },
    );
    vehicle.account = "logistics/account/vehicle";
    vehicle.evidence = [#{
        kind: "balance", account: "logistics/account/vehicle",
        asset: `logistics/asset/${vehicle_asset}`,
    }];
    vehicle.metrics = [
        asset_metric("fuel", "Fuel", "fuel", state.balance("vehicle", "fuel")),
        asset_metric("cargo", "Orders aboard", "cargo-occupied", state.balance("vehicle", "cargo-occupied")),
        asset_metric("elapsed", "Elapsed", "elapsed-time", state.balance("vehicle", "elapsed-time")),
    ];
    let entities = [vehicle];
    for order in logistics::orders() {
        let account = `order/${order}`;
        let placed = if state.balance(account, "delivered") > 0 {
            [at_location("customer"), "success", "delivered"]
        } else if state.balance(account, "in-transit") > 0 {
            [vehicle_anchor, "active", "in transit"]
        } else {
            [at_location("depot"), "neutral", "waiting"]
        };
        entities.push(with_account(
            entity(account, order_label(order), "package", placed[0], "attachment", placed[1], placed[2]),
            account,
        ));
    }
    entities.push(with_account(
        entity("system/fuel-station", "Fuel station", "fuel", at_location("depot"), "state", "neutral", "refuel supply"),
        "fuel-station",
    ));
    entities.push(with_account(
        entity("system/nature", "Travel conditions", "weather", at_location("junction"), "context", "uncertain", "sampled chance"),
        "nature",
    ));

    let scene = scene::entities(
        scene::graph(
            "Route network and travel risk", nodes, edges,
            if focus != () { `location/${focus}` } else { () },
        ),
        entities,
    );
    scene.metrics = [
        metric("delivered", "Orders delivered", logistics::delivered(state), "orders"),
        asset_metric("remaining-time", "Time remaining", "time-remaining", state.balance("vehicle", "time-remaining")),
        asset_metric("fuel", "Fuel remaining", "fuel", state.balance("vehicle", "fuel")),
    ];
    scene
}
