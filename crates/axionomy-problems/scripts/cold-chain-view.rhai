// Perishables' cold chains: how people read cohorts of food moving through
// a warehouse, a finite fridge and a preserving line toward funded
// deliveries, on power and fuel they compete for. Labels and formats name
// its identifiers; the scene draws the chain's places and what links them,
// each cohort in use with its thermal history, each place's stock, and the
// day's grid, refrigeration, stock, deliveries and reserves. Batches and
// totals are read as the cold chain's economy script reads them.

import "perishables/cold-chain" as chain;

// A cohort, counted from zero, as people count it.
fn cohort_label(id) {
    `Cohort ${parse_int(id) + 1}`
}

fn place(place) {
    switch place {
        "warehouse" => "the warehouse",
        "fridge" => "the fridge",
    }
}

fn asset_label(parts) {
    switch parts[1] {
        "world-identity" => "Cold chain identity",
        "batch-identity" => `${cohort_label(parts[2])} identity`,
        "buyer-identity" => "Buyer identity",
        "active" => "Cohort in use",
        "dormant" => "Cohort not yet in use",
        "at" => `In ${place(parts[2])}`,
        "fresh" => "Fresh food",
        "preserved" => "Preserved food",
        "spoiled" => "Spoiled food",
        "disposed" => "Discarded food",
        "delivered" => "Food delivered",
        "heat" => "Heat exposure",
        "origin" => switch parts[2] {
            "0" => "Ambient origin",
            "1" => "Cold origin",
            _ => `Origin ${parse_int(parts[2]) + 1}`,
        },
        "cold-space" => "Free cold storage",
        "cold-occupancy" => "Cold storage used",
        "energy" => "Electricity",
        "spent-energy" => "Electricity used",
        "grid-potential" => "Grid supply still to come",
        "spilled-energy" => "Grid supply spilled",
        "fuel" => "Fuel",
        "generator-fuel" => "Fuel burned for electricity",
        "transport-fuel" => "Fuel used for deliveries",
        "cooling-on" => "Refrigeration on today",
        "cooling-off" => "Refrigeration off",
        "labor" => "Labor left today",
        "spent-labor" => "Labor used",
        "future-labor" => "Labor in later days",
        "unused-labor" => "Labor left unused",
        "freight" => "Shipments left today",
        "used-freight" => "Shipments made",
        "future-freight" => "Shipments in later days",
        "unused-freight" => "Shipments left unused",
        "money" => "Money",
        "demand" => "Food owed today",
        "future-demand" => "Food owed in later days",
        "satisfied-demand" => "Food obligations met",
        "missed-demand" => "Food obligations missed",
        "day" => "Time elapsed",
        "days" => "Run length",
        "outage" => `Grid outage on day ${parse_int(parts[2]) + 1}`,
        "decisions" => "Taking today's decisions",
        "aging" => "Settling today's heat exposure",
        "pending-age" => `${cohort_label(parts[2])} still to age today`,
        "solved" => "Horizon closed",
        _ => (),
    }
}

fn rate_label(parts) {
    switch parts[1] {
        "split" =>
            `Split ${cohort_label(parts[2])} into ${cohort_label(parts[3])} in ${place(parts[4])}, keeping its heat history`,
        "move" => `Move ${cohort_label(parts[2])} to ${place(parts[4])}`,
        "process" => `Preserve fresh food in ${cohort_label(parts[2])}`,
        "deliver" => `Deliver ${if parts[3] == "true" { "preserved" } else { "fresh" }} food from ${cohort_label(parts[2])} in ${place(parts[4])}`,
        "discard" => if parts[3] == "fridge" {
            `Discard spoiled food from ${cohort_label(parts[2])} and free fridge space`
        } else {
            `Discard spoiled food from ${cohort_label(parts[2])} in the warehouse`
        },
        "cool" => "Power refrigeration for today",
        "generate" => "Burn shared fuel for backup electricity",
        "end-day" => "Close today's decisions and settle thermal exposure",
        "age-empty" => `${cohort_label(parts[2])} has no fresh food to age`,
        "age-cold" => `${cohort_label(parts[2])} stays cold without resetting its prior heat`,
        "age-warm" => `${cohort_label(parts[2])} ${
            if parts[4] == "true" { "crosses the spoilage threshold" } else { "accumulates heat" }
        } in ${if parts[3] == "warehouse" { "the warehouse" } else { "the unpowered fridge" }}`,
        "open-day" =>
            `Open day ${parse_int(parts[2]) + 1} with its scheduled grid supply and new obligations`,
        "close" => "Close the cold-chain horizon and record unmet demand",
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "world" => "Cold-chain resources",
            "buyer" => "Funded food deliveries",
            "batch" => cohort_label(parts[2]),
            _ => (),
        },
        "asset" => asset_label(parts),
        "rate" => rate_label(parts),
        "role" => switch parts[1] {
            "world" => "Cold chain",
            "batch" => "Cohort",
            "source" => "Source cohort",
            "destination" => "Destination cohort",
            "buyer" => "Buyer",
            _ => (),
        },
        _ => (),
    }
}

// The unit of an asset's quantities, if it has one.
fn unit(asset) {
    switch asset.split("/")[0] {
        "fresh" | "preserved" | "spoiled" | "disposed" | "delivered" | "cold-space"
            | "cold-occupancy" | "demand" | "future-demand" | "satisfied-demand"
            | "missed-demand" => "crates",
        "energy" | "spent-energy" | "grid-potential" | "spilled-energy" | "fuel"
            | "generator-fuel" | "transport-fuel" | "labor" | "spent-labor" | "future-labor"
            | "unused-labor" => "units",
        "money" => "credits",
        "day" | "days" => "days",
        _ => (),
    }
}

// Food reads in crates, energy, fuel and labor in units, money in credits
// and time in days.
fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

fn val(state, account, asset) {
    state.balance(account, asset).to_int()
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

// A metric showing a quantity of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    metric(key, label, quantity, unit(asset))
}

// An entity on the graph node `node`, shown as `account` and citing it.
fn entity(key, label, glyph, node, role, tone, status, account) {
    let account = `perishables/cold-chain/account/${account}`;
    #{
        id: #{ key: key, label: label },
        glyph: glyph,
        anchor: #{ kind: "graph_node", node: node },
        role: role,
        tone: tone,
        status: status,
        account: account,
        evidence: [#{ kind: "account", account: account }],
    }
}

fn scene(state, frame) {
    let nodes = [
        ["warehouse", "Ambient warehouse", 120.0, 140.0],
        ["fridge", "Finite cold storage", 460.0, 140.0],
        ["processing", "Preserving line", 120.0, 340.0],
        ["buyer", "Food deliveries", 770.0, 260.0],
        ["power", "Grid and battery", 460.0, 340.0],
        ["fuel", "Shared fuel reserve", 770.0, 70.0],
    ].map(|node| #{
        id: #{ key: node[0], label: node[1] },
        classes: ["resource"],
        x: node[2],
        y: node[3],
    });
    let edges = [
        ["warehouse", "fridge", "space + labor"],
        ["warehouse", "processing", "energy + labor"],
        ["warehouse", "buyer", "up to 2 crates/day"],
        ["fridge", "buyer", "delivery frees space"],
        ["power", "fridge", "2 energy/day"],
        ["power", "processing", "1 energy/crate"],
        ["fuel", "power", "1 fuel → 3 energy"],
        ["fuel", "buyer", "1 fuel/trip"],
    ].map(|route| #{
        id: `cold/${route[0]}/${route[1]}`,
        source: route[0],
        target: route[1],
        label: route[2],
        classes: [],
    });

    let entities = [];
    for id in chain::batch_ids() {
        let account = `batch/${id}`;
        let batch = chain::batch_shadow(state, id);
        let at = batch.at;
        if at == "" { continue; }
        let fresh = batch.fresh;
        let stable = batch.preserved;
        let spoiled = batch.spoiled;
        let cohort = entity(
            `cohort/${id}`, cohort_label(id.to_string()), "package",
            if at == "fridge" { "fridge" } else { "warehouse" }, "attachment",
            if spoiled > 0 { "danger" } else if fresh + stable == 0 { "muted" } else { "active" },
            `${fresh} fresh · ${stable} preserved · ${spoiled} spoiled`, account,
        );
        cohort.metrics = [
            metric("heat", "Heat exposure", val(state, account, "heat"), "/ 4"),
            metric(
                "origin", "Thermal lineage",
                if val(state, account, "origin/0") > 0 { "ambient origin" } else { "cold origin" },
                (),
            ),
        ];
        entities.push(cohort);
    }
    for stock in [
        ["fridge", "stock/space", "space", "Free cold capacity", "cold-space", val(state, "world", "cold-space")],
        ["power", "stock/energy", "energy", "Stored energy", "energy", chain::total(state, "energy")],
        ["fuel", "stock/fuel", "fuel", "Fuel available", "fuel", chain::total(state, "fuel")],
        ["buyer", "stock/delivered", "delivered", "Delivered", "delivered", chain::total(state, "delivered")],
        ["processing", "stock/labor", "labor", "Labor available", "labor", chain::total(state, "labor")],
    ] {
        let drawn = entity(
            stock[1], stock[3], "material", stock[0], "state", "neutral", (),
            if stock[0] == "buyer" { "buyer" } else { "world" },
        );
        drawn.metrics = [asset_metric(stock[2], stock[3], stock[4], stock[5])];
        entities.push(drawn);
    }

    let day = val(state, "world", "day");
    let scene = scene::entities(
        scene::graph("Thermal history, physical capacity and funded deliveries", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        metric("day", "Day", day + 1, ()),
        metric(
            "grid", "Grid supply",
            if val(state, "world", `outage/${day}`) > 0 { "outage" } else { "available" }, (),
        ),
        metric(
            "cooling", "Refrigeration",
            if chain::total(state, "cooling-on") > 0 { "powered" } else { "unpowered" }, (),
        ),
        asset_metric("fresh", "Fresh stock", "fresh", chain::total(state, "fresh")),
        asset_metric("preserved", "Preserved stock", "preserved", chain::total(state, "preserved")),
        metric(
            "spoiled", "Spoiled or discarded",
            chain::total(state, "spoiled") + chain::total(state, "disposed"), "crates",
        ),
        asset_metric("delivered", "Food delivered", "delivered", chain::total(state, "delivered")),
        asset_metric("missed", "Unmet obligations", "missed-demand", chain::total(state, "missed-demand")),
        asset_metric("energy", "Electricity left", "energy", chain::total(state, "energy")),
        asset_metric("fuel", "Fuel left", "fuel", chain::total(state, "fuel")),
    ];
    scene
}
