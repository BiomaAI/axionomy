// Logistics' fleet: how people read two vans sharing one clock, a trunk
// road, a fuel pump, a repair bay and a relay hub. Labels and formats name
// its identifiers; the scene draws the depot, the hub and the customers,
// each van where it stands or how far along its road, the cargo aboard,
// every order and its window, and the shared services' free capacity. The
// vans, places, roads and orders are the fleet's economy script's.

import "logistics/fleet" as fleet;

// A name as people read it: `cold` is `Cold`.
fn capital(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

fn van_label(van) {
    `${capital(van)} van`
}

// An order, counted from zero, as people count it.
fn order_label(id) {
    `Order ${parse_int(id) + 1}`
}

fn asset_label(parts) {
    switch parts[1] {
        "world-identity" => "Fleet identity",
        "station-identity" => "Station identity",
        "nature-identity" => "Nature identity",
        "vehicle-identity" => `${van_label(parts[2])} identity`,
        "order-identity" => `${order_label(parts[2])} identity`,
        "active" => "Fleet run in progress",
        "closed" => "Fleet run closed",
        "time" => "Fleet time",
        "time-remaining" => "Time remaining",
        "horizon" => "Fleet horizon",
        "count" => "Orders in the run",
        "resolved" => "Orders resolved",
        "delivered-count" => "Orders delivered",
        "on-time-count" => "Orders on time",
        "expired-count" => "Orders expired",
        "permit" => `${van_label(parts[2])} may act this tick`,
        "used-permit" => `${van_label(parts[2])} has acted this tick`,
        "road-space" => "Trunk road free",
        "on-main-road" => "On the trunk road",
        "dock-space" => "Free hub staging space",
        "dock-used" => "Hub staging space used",
        "at" => `At ${capital(parts[2])}`,
        "travelling" => `Travelling ${capital(parts[2])} → ${capital(parts[3])}`,
        "awaiting-outcome" => "Awaiting the trip outcome",
        "ready" => "Ready",
        "departed-at" => "Departed at",
        "ready-at" => "Busy until",
        "trip" => "Trips taken",
        "integrity" => "Roadworthy",
        "damage" => "Broken down",
        "wear" => "Wear",
        "maintained-wear" => "Wear serviced",
        "fuel" => "Fuel",
        "tank-space" => "Free tank space",
        "spent-fuel" => "Fuel spent",
        "space" => "Free payload space",
        "cold-space" => "Free refrigerated slot",
        "cargo" => `Carrying ${order_label(parts[2])}`,
        "money" => "Cash",
        "revenue" => "Revenue",
        "price" => "Price",
        "fuel-pump" => "Free fuel pump",
        "refueling" => "Refueling",
        "repair-bay" => "Free repair bay",
        "repairing" => "In the repair bay",
        "part" => "Spare parts",
        "used-part" => "Parts used",
        "tow-truck" => "Free tow truck",
        "towing" => "Being towed",
        "scheduled" => "Arriving later",
        "waiting" => "Waiting for pickup",
        "in-transit" => "In transit",
        "delivered" => "Delivered",
        "expired" => "Expired",
        "arrival" => "Released at",
        "deadline" => "Delivery deadline",
        "destination" => `Bound for ${capital(parts[2])}`,
        "weight" => "Weight",
        "cold-cargo" => "Needs refrigeration",
        "ticket-word" =>
            `Hidden trip outcomes: ${van_label(parts[2])}, ${capital(parts[3])} → ${capital(parts[4])}`,
        "ticket-divisor" => "Trip ticket cursor",
        "transfer-count" => "Van-to-van transfers",
        "staged-count" => "Hub handoffs staged",
        "damage-count" => "Breakdowns",
        _ => (),
    }
}

fn rate_label(parts) {
    switch parts[1] {
        "release" => `${order_label(parts[2])} becomes available`,
        "load" => `${van_label(parts[2])} loads ${order_label(parts[3])} at ${capital(parts[4])}`,
        "stage" => `${van_label(parts[2])} stages ${order_label(parts[3])} at the shared hub`,
        "transfer" =>
            `Transfer ${order_label(parts[4])} from ${van_label(parts[2])} to ${van_label(parts[3])} at ${capital(parts[5])}`,
        "depart" => `${van_label(parts[2])} departs ${capital(parts[3])} → ${capital(parts[4])}`,
        "resolve" => `${van_label(parts[2])}'s trip ${capital(parts[3])} → ${capital(parts[4])} ${
            switch parts[5] {
                "clear" => "is clear",
                "delayed" => "is delayed",
                _ => "breaks down",
            }
        }`,
        "arrive" => `${van_label(parts[2])} arrives at ${capital(parts[4])} from ${capital(parts[3])}`,
        "deliver" => `${van_label(parts[2])} delivers ${order_label(parts[3])} ${
            if parts[4] == "true" { "late for half payment" } else { "within its window" }
        }`,
        "expire" => if parts[3] == "some" {
            `${order_label(parts[2])} expires undelivered aboard the ${van_label(parts[4])}`
        } else {
            `${order_label(parts[2])} expires undelivered at ${capital(parts[4])}`
        },
        "refuel" => `${van_label(parts[2])} occupies the pump and buys 3 fuel`,
        "end-refuel" => `${van_label(parts[2])} finishes refueling and releases the pump`,
        "repair" => `${van_label(parts[2])} starts ${
            if parts[3] == "true" { "breakdown repair" } else { "preventive maintenance" }
        } in the repair bay`,
        "end-repair" => `${van_label(parts[2])} finishes ${
            if parts[3] == "true" { "breakdown repair" } else { "maintenance" }
        } and releases the repair bay`,
        "tow" => `Dispatch the shared tow truck for the ${van_label(parts[2])} at ${capital(parts[3])}`,
        "end-tow" => `Tow the ${van_label(parts[2])} to the depot and release the tow truck`,
        "tick" => "Advance shared fleet time one tick",
        "close" => "Close the fleet run with all orders and occupied services settled",
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "world" => "Fleet clock, budget and road capacity",
            "station" => "Fuel, repair and towing operator",
            "nature" => "Seeded vehicle and route tickets",
            "vehicle" => van_label(parts[2]),
            "order" => order_label(parts[2]),
            _ => (),
        },
        "asset" => asset_label(parts),
        "rate" => rate_label(parts),
        "role" => switch parts[1] {
            "world" => "Fleet",
            "vehicle" => "Van",
            "other" => "Other van",
            "order" => "Order",
            "station" => "Station",
            "nature" => "Nature",
            _ => (),
        },
        _ => (),
    }
}

// The unit of an asset's quantities, if it has one.
fn unit(asset) {
    switch asset {
        "time" | "time-remaining" | "horizon" | "departed-at" | "ready-at" | "arrival" | "deadline" =>
            "ticks",
        "fuel" | "tank-space" | "spent-fuel" | "space" | "weight" | "dock-space" | "dock-used" =>
            "units",
        "money" | "revenue" | "price" => "credits",
        _ => (),
    }
}

// Time reads in ticks of the fleet's clock, fuel and payload in units, and
// money in credits.
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

// An entity shown as `account`, citing the account as its evidence.
fn entity(key, label, glyph, anchor, role, tone, status, account) {
    let account = `logistics/fleet/account/${account}`;
    #{
        id: #{ key: key, label: label },
        glyph: glyph,
        anchor: anchor,
        role: role,
        tone: tone,
        status: status,
        account: account,
        evidence: [#{ kind: "account", account: account }],
    }
}

// A van where it stands, or how far along the road it travels.
fn van_entity(state, van, now) {
    let own = `vehicle/${van}`;
    let at = fleet::whereabouts(state, own);
    let travel = ();
    for road in fleet::roads() {
        for leg in [[road[0], road[1]], [road[1], road[0]]] {
            if travel == () && fleet::value(state, own, `travelling/${leg[0]}/${leg[1]}`) > 0 {
                travel = leg;
            }
        }
    }
    let ready_at = fleet::value(state, own, "ready-at");
    let anchor = if at != () {
        #{ kind: "graph_node", node: at }
    } else if travel != () {
        let road = ();
        for candidate in fleet::roads() {
            if road == () && ((candidate[0] == travel[0] && candidate[1] == travel[1])
                || (candidate[0] == travel[1] && candidate[1] == travel[0])) {
                road = candidate;
            }
        }
        let start = fleet::value(state, own, "departed-at");
        let progress = if ready_at > start {
            let elapsed = if now > start { now - start } else { 0 };
            let fraction = elapsed.to_float() / (ready_at - start).to_float();
            if fraction < 0.0 { 0.0 } else if fraction > 1.0 { 1.0 } else { fraction }
        } else {
            0.0
        };
        #{
            kind: "graph_edge",
            edge: `road/${road[0]}/${road[1]}`,
            progress: if road[0] == travel[0] { progress } else { 1.0 - progress },
        }
    } else {
        #{ kind: "unanchored" }
    };
    let status = if travel != () {
        if fleet::value(state, own, "awaiting-outcome") > 0 {
            `To ${capital(travel[1])} · awaiting route outcome`
        } else {
            `To ${capital(travel[1])} · arrives ${ready_at}`
        }
    } else if fleet::value(state, own, "refueling") > 0 {
        `Pump occupied until ${ready_at}`
    } else if fleet::value(state, own, "repairing") > 0 {
        `Repair bay occupied until ${ready_at}`
    } else if fleet::value(state, own, "towing") > 0 {
        `Being towed until ${ready_at}`
    } else if fleet::value(state, own, "damage") > 0 {
        "Broken down · transfer cargo or tow"
    } else {
        "Ready for dispatch"
    };
    let drawn = entity(
        `van/${van}`, van_label(van), "vehicle", anchor,
        if at != () || travel != () { "occupant" } else { "context" },
        if fleet::value(state, own, "damage") > 0 { "danger" } else { "active" },
        status, own,
    );
    drawn.metrics = [
        metric("fuel", "Fuel", fleet::value(state, own, "fuel"), "/ 8"),
        asset_metric("space", "Free payload space", "space", fleet::value(state, own, "space")),
        metric("wear", "Wear", fleet::value(state, own, "wear"), "/ 6"),
    ];
    drawn
}

fn scene(state, frame) {
    let now = fleet::value(state, "world", "time");
    let count = fleet::value(state, "world", "count");
    let nodes = [
        ["depot", "Depot and service yard", 120.0, 300.0],
        ["hub", "Shared relay hub", 430.0, 300.0],
        ["north", "North customer", 430.0, 50.0],
        ["south", "South customer", 430.0, 590.0],
        ["east", "East customer", 780.0, 300.0],
    ].map(|node| #{
        id: #{ key: node[0], label: node[1] },
        classes: ["location"],
        x: node[2],
        y: node[3],
    });
    let edges = fleet::roads().map(|road| #{
        id: `road/${road[0]}/${road[1]}`,
        source: road[0],
        target: road[1],
        label: if road[0] == "depot" && road[1] == "hub" { "one vehicle at a time" }
            else if road[0] == "depot" && road[1] == "north" { "express · 2 fuel · breakdown risk" }
            else { "2 ticks + possible delay" },
        classes: [],
    });

    let entities = [];
    for van in fleet::vans() {
        entities.push(van_entity(state, van, now));
        for id in 0..count {
            if fleet::value(state, `vehicle/${van}`, `cargo/${id}`) > 0 {
                let cargo = entity(
                    `cargo/${id}`, `${order_label(id.to_string())} cargo`, "package",
                    #{ kind: "entity", entity: `van/${van}` }, "attachment", "neutral",
                    if fleet::cold(id) { "Refrigerated" } else if fleet::heavy(id) { "Heavy freight" } else { "Standard freight" },
                    `order/${id}`,
                );
                cargo.metrics = [
                    asset_metric("weight", "Payload", "weight", fleet::value(state, `order/${id}`, "weight")),
                ];
                entities.push(cargo);
            }
        }
    }
    for id in 0..count {
        let own = `order/${id}`;
        let delivered = fleet::value(state, own, "delivered") > 0;
        let expired = fleet::value(state, own, "expired") > 0;
        let staged = fleet::value(state, own, "dock-used") > 0;
        let status = if delivered { "Delivered and paid" }
            else if expired { "Expired undelivered" }
            else if staged { "Staged at hub for another van" }
            else if fleet::value(state, own, "in-transit") > 0 { "Cargo assigned to vehicle" }
            else if fleet::value(state, own, "scheduled") > 0 { "Customer order arriving later" }
            else { "Cargo waiting at depot" };
        let kind = if fleet::cold(id) { "cold" } else if fleet::heavy(id) { "heavy" } else { "standard" };
        let order = entity(
            own, `${order_label(id.to_string())} · ${kind}`, "task",
            #{ kind: "graph_node", node: if staged { "hub" } else { fleet::destination(id) } },
            "attachment",
            if delivered { "success" } else if expired { "danger" } else { "neutral" },
            status, own,
        );
        order.metrics = [
            asset_metric("deadline", "Delivery window", "deadline", fleet::value(state, own, "deadline")),
            asset_metric("arrival", "Order release", "arrival", fleet::value(state, own, "arrival")),
        ];
        entities.push(order);
    }

    let scene = scene::entities(
        scene::graph("Fleet cargo, shared roads and occupied service capacity", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        asset_metric("time", "Fleet time", "time", now),
        asset_metric("delivered", "Orders delivered", "delivered-count", fleet::value(state, "world", "delivered-count")),
        asset_metric("on-time", "Orders on time", "on-time-count", fleet::value(state, "world", "on-time-count")),
        asset_metric("expired", "Orders expired", "expired-count", fleet::value(state, "world", "expired-count")),
        asset_metric("staged", "Hub handoffs staged", "staged-count", fleet::value(state, "world", "staged-count")),
        asset_metric("cash", "Fleet cash", "money", fleet::value(state, "world", "money")),
    ];
    // The depot's and the hub's shared capacity, on the places themselves.
    for index in 0..scene.entities.len() {
        let key = scene.entities[index].id.key;
        if key == "depot" {
            scene.entities[index].status = "Shared paid services";
            scene.entities[index].metrics = [
                metric("pump", "Free fuel pump", fleet::value(state, "station", "fuel-pump"), "/ 1"),
                metric("bay", "Free repair bay", fleet::value(state, "station", "repair-bay"), "/ 1"),
                asset_metric("fuel", "Station fuel", "fuel", fleet::value(state, "station", "fuel")),
            ];
        } else if key == "hub" {
            scene.entities[index].status = "Finite cargo relay";
            scene.entities[index].metrics = [
                metric("dock", "Free staging space", fleet::value(state, "world", "dock-space"), "/ 2"),
                metric("road", "Trunk road available", fleet::value(state, "world", "road-space"), "/ 1"),
            ];
        }
    }
    scene
}
