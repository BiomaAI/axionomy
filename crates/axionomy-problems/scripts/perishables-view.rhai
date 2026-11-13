// Perishables' Micro: how people read two batches of interchangeable units
// on the spoilage clock. Labels and formats name its identifiers; the scene
// draws a timeline of the clock, each batch's condition and each store's
// claims, the cursor at the moment the clock has reached.

// A name as people read it: `fridge` is `Fridge`.
fn capital(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

// Where a batch's stock stays fresh.
fn exposure(exposure) {
    switch exposure {
        "ambient" => "in ambient storage",
        "cold" => "in cold storage",
        "warmed-after-outage" => "after warming in the outage",
    }
}

// The moments of the clock, harvest first.
fn moments() { ["harvest", "ambient-expiry", "warmed-expiry", "cold-expiry"] }

fn moment(moment) {
    switch moment {
        "harvest" => "harvest",
        "ambient-expiry" => "ambient expiry",
        "warmed-expiry" => "warmed expiry",
        "cold-expiry" => "cold expiry",
    }
}

fn asset_label(parts) {
    switch parts[1] {
        "world-identity" => "Clock identity",
        "location-identity" => `${capital(parts[2])} identity`,
        "cohort-identity" => `${capital(parts[2])} batch identity`,
        "claim" => `Units in the ${parts[2]} batch`,
        "consumed" => `Used units from the ${parts[2]} batch`,
        "fresh" => `${capital(parts[2])} batch fresh ${exposure(parts[3])}, until ${moment(parts[4])}`,
        "rotten" => `Spoiled units from the ${parts[2]} batch`,
        "ambient" => "Ambient storage",
        "cold" => "Cold storage",
        "powered" => "Refrigeration running",
        "unpowered" => "Refrigeration down",
        "cooling-energy" => "Cooling energy left",
        "spent-cooling-energy" => "Cooling energy spent",
        "now" => `Clock at ${moment(parts[2])}`,
        "before" => `Before ${moment(parts[2])}`,
        "reached" => `${capital(moment(parts[2]))} reached`,
        "active" => "Inventory simulation running",
        "solved" => "Inventory simulation complete",
        "planning" => "Storage plan open",
        "plan-solved" => "Storage plan complete",
        _ => (),
    }
}

fn rate_label(parts) {
    switch parts[1] {
        "move-to-fridge" => "Move ambient batch units into the fridge",
        "advance" => `Advance time: ${moment(parts[2])} → ${moment(parts[3])}`,
        "lose-power" => "Refrigeration loses power",
        "eat" => `Use fresh ${parts[2]} units ${exposure(parts[3])}`,
        "spoil" => `Spoil the ${parts[2]} batch ${exposure(parts[3])}`,
        "finish" => "Finish the spoilage simulation",
        "seal-storage-plan" => "Commit the storage plan",
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "world" => "Clock and power",
            "storage" => capital(parts[2]),
            "cohort" => `${capital(parts[2])} batch condition`,
            _ => (),
        },
        "asset" => asset_label(parts),
        "rate" => rate_label(parts),
        "role" => switch parts[1] {
            "world" => "World",
            "holder" => "Holder",
            "storage" => "Storage",
            "cohort" => "Batch",
            "source-storage" => "Source storage",
            "destination-storage" => "Destination storage",
            "source-cohort" => "Source batch",
            "destination-cohort" => "Destination batch",
            _ => (),
        },
        _ => (),
    }
}

// Cooling energy reads in units; every other quantity is a count.
fn format(asset) {
    switch asset {
        "cooling-energy" | "spent-cooling-energy" => #{ scale: 0, unit: "units" },
        _ => (),
    }
}

fn span(id, lane, end, label, classes) {
    #{ id: id, lane: lane, start: 0, end: end, label: label, classes: classes }
}

fn scene(state, frame) {
    let lanes = [
        ["world", "World clock"],
        ["ambient", "Ambient cohort"],
        ["refrigerated", "Refrigerated cohort"],
        ["warehouse", "Warehouse claims"],
        ["fridge", "Fridge claims"],
    ].map(|lane| #{ id: #{ key: lane[0], label: lane[1] }, classes: [] });
    let cursor = 0;
    for (at, index) in moments() {
        if state.balance("world", `now/${at}`) > 0 {
            cursor = index;
            break;
        }
    }
    let ambient_rotten = state.balance("cohort/ambient", "rotten/ambient") > 0;
    let cold_rotten = state.balance("cohort/refrigerated", "rotten/refrigerated") > 0;
    let powered = state.balance("storage/fridge", "powered") > 0;
    let warehouse = state.balance("storage/warehouse", "claim/ambient");
    let fridge = state.balance("storage/fridge", "claim/refrigerated");
    let spans = [
        span(
            "ambient-condition", "ambient", 1,
            if ambient_rotten { "Rotten" } else { "Fresh until ambient expiry" },
            if ambient_rotten { ["failed"] } else { [] },
        ),
        span(
            "cold-condition", "refrigerated", if powered { 3 } else { 2 },
            if cold_rotten { "Rotten after outage" }
                else if powered { "Cold + powered" }
                else { "Warming after outage" },
            if powered { ["selected"] } else { ["uncertain"] },
        ),
        span("warehouse-claims", "warehouse", 3, `${warehouse} fungible claims`, []),
        span("fridge-claims", "fridge", 3, `${fridge} fungible claims`, []),
    ];
    scene::timeline("Batches, storage, and the spoilage clock", lanes, spans, cursor)
}
