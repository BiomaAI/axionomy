// Workshop: how people read the stoichiometric workshop. Labels and formats
// name its accounts, stocks, recipes and roles; the scene draws the
// materials and products as a graph of recipes, the workshop's stock of each
// and every customer's contract, with the shop's progress as metrics.

fn ordinal(text) {
    parse_int(text) + 1
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return if name == "workshop" { "Workshop" } else { `Customer ${ordinal(parts[2])}` };
    }
    if category == "role" {
        return if name == "shop" { "Shop" } else { "Customer" };
    }
    if category == "asset" {
        return switch name {
            "customer-identity" => `Customer ${ordinal(parts[2])} identity`,
            "credit" => "Credit",
            "frame" => "Prepared frames",
            "panel" => "Prepared panel material",
            "offcut" => "Recyclable offcuts",
            "table" => "Finished tables",
            "durability" => "Tool durability",
            "wear" => "Tool wear",
            "tool-potential" => "Upgrade capacity",
            "repair-kit" => "Repair kits",
            "used-repair-kit" => "Used repair kits",
            "upgrade-kit" => "Available upgrade",
            "upgraded" => "Precision tool upgrade",
            "idle" => "Workbench available",
            "contract" => `Contract ${ordinal(parts[2])} available`,
            "raw" => `Contract ${ordinal(parts[2])} awaiting components`,
            "prepared" => `Contract ${ordinal(parts[2])} components prepared`,
            "assembled" => `Contract ${ordinal(parts[2])} ready to deliver`,
            "fulfilled" => `Contract ${ordinal(parts[2])} fulfilled`,
            "completed" => "Contracts delivered",
            "wood" => "Wood left",
            "labor" => "Labor left",
            "spent-labor" => "Labor used",
            "tool" => "Reusable tool",
            "chair" => "Finished chairs",
            "waste" => "Scrap",
            "time" => "Time left",
            "spent-time" => "Time spent",
            "active" => "Order in progress",
            "solved" => "Order complete",
            _ => (),
        };
    }
    switch name {
        "accept" => `Accept contract ${ordinal(parts[2])}`,
        "prepare" => `Prepare components for contract ${ordinal(parts[2])} with ${
            if parts[4] == "true" { "the upgraded tool" }
            else if parts[3] == "true" { "a precision cut" }
            else { "a fast cut" }
        }`,
        "assemble" => `Assemble contract ${ordinal(parts[2])}`,
        "deliver" => `Deliver contract ${ordinal(parts[2])} and collect payment`,
        "repair" => "Restore worn tool with a repair kit",
        "upgrade" => "Install the precision tool upgrade",
        "recycle" => "Recover wood from two offcuts",
        "basic-chair" => "Build one chair, fast but with more scrap",
        "efficient-batch" => "Build two chairs, slower but with less scrap",
        "counterfeit-chair" => "Counterfeit chair proposal",
        "finish" => "Complete the order",
        _ => (),
    }
}

// The unit quantities of `asset` are read in, or nothing for a count.
fn unit(asset) {
    switch asset {
        "wood" | "panel" | "waste" | "labor" | "spent-labor" => "units",
        "time" | "spent-time" => "ticks",
        "credit" => "credits",
        _ => (),
    }
}

fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

// A quantity of `asset` as people read it, unit included.
fn written(asset, quantity) {
    let unit = unit(asset);
    if unit == () { quantity.to_string() } else { `${quantity} ${unit}` }
}

// A metric showing a quantity of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    #{ key: key, label: label, value: quantity.to_string(), unit: unit(asset) }
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
    entity.account = `workshop/account/${account}`;
    entity.evidence = [#{
        kind: "balance", account: `workshop/account/${account}`, asset: `workshop/asset/${asset}`,
    }];
    entity
}

fn at(node) {
    #{ kind: "graph_node", node: node }
}

fn scene(state, frame) {
    let contracts = state.has_account("customer/0");
    let assets = [["Wood", "wood"], ["Labor", "labor"], ["Tool", "tool"], ["Chair", "chair"], ["Waste", "waste"]];
    if contracts {
        assets += [
            ["Frames", "frame"], ["Panels", "panel"], ["Tables", "table"], ["Offcuts", "offcut"],
            ["Durability", "durability"], ["Wear", "wear"], ["Revenue", "credit"],
        ];
    }
    let nodes = [];
    for (pair, index) in assets {
        nodes.push(#{
            id: #{ key: pair[1], label: pair[0] },
            classes: if pair[1] == "chair" { ["goal"] } else { ["resource"] },
            x: (index % 3).to_float() * 220.0 + 80.0,
            y: (index / 3).to_float() * 150.0 + 70.0,
        });
    }
    let routes = if contracts {
        [
            ["wood", "frame", "cut"],
            ["wood", "panel", "cut"],
            ["wood", "offcut", "fast-cut residue"],
            ["frame", "chair", "assemble"],
            ["panel", "chair", "assemble"],
            ["frame", "table", "assemble"],
            ["panel", "table", "assemble"],
            ["offcut", "wood", "recycle with loss"],
            ["offcut", "waste", "recycling loss"],
            ["durability", "wear", "production"],
            ["wear", "durability", "repair"],
            ["chair", "credit", "deliver"],
            ["table", "credit", "deliver"],
        ]
    } else {
        [
            ["wood", "chair", "basic recipe"],
            ["wood", "chair", "batch recipe"],
            ["wood", "waste", "by-product"],
            ["tool", "chair", "preserved catalyst"],
        ]
    };
    let edges = [];
    for (route, index) in routes {
        edges.push(#{ id: `recipe/${index}`, source: route[0], target: route[1], label: route[2], classes: [] });
    }

    let entities = [];
    for pair in assets {
        let asset = pair[1];
        let quantity = state.balance("workshop", asset);
        if quantity.is_zero() { continue; }
        let stock = linked(entity(
            `stock/${asset}`, pair[0],
            switch asset { "tool" => "tool", "chair" => "product", _ => "material" },
            at(asset), "state",
            switch asset { "chair" => "success", "waste" => "warning", _ => "active" },
            `${written(asset, quantity)} available`,
        ), "workshop", asset);
        stock.metrics = [asset_metric(asset, pair[0], asset, quantity)];
        entities.push(stock);
    }
    for account in state.accounts("customer/") {
        let order = account.split("/")[1];
        let status = if !state.balance("workshop", `fulfilled/${order}`).is_zero() { "delivered and paid" }
            else if !state.balance("workshop", `assembled/${order}`).is_zero() { "ready for delivery" }
            else if !state.balance("workshop", `prepared/${order}`).is_zero() { "components ready" }
            else if !state.balance("workshop", `raw/${order}`).is_zero() { "accepted · awaiting components" }
            else { "available" };
        let contract = linked(entity(
            `contract/${order}`, `Contract ${ordinal(order)}`, "task", at("credit"), "attachment",
            if status == "delivered and paid" { "success" } else { "neutral" }, status,
        ), account, `customer-identity/${order}`);
        contract.metrics = [asset_metric("escrow", "Customer funds", "credit", state.balance(account, "credit"))];
        entities.push(contract);
    }

    let scene = scene::entities(
        scene::graph("Materials, tooling and contract production", nodes, edges, ()),
        entities,
    );
    scene.metrics = if contracts {
        [
            asset_metric("completed", "Contracts delivered", "completed", state.balance("workshop", "completed")),
            asset_metric("revenue", "Revenue", "credit", state.balance("workshop", "credit")),
            asset_metric("durability", "Tool durability", "durability", state.balance("workshop", "durability")),
            asset_metric("repairs", "Repair kits remaining", "repair-kit", state.balance("workshop", "repair-kit")),
            #{
                key: "upgrade", label: "Precision upgrade",
                value: if state.balance("workshop", "upgraded").is_zero() { "available" } else { "installed" },
                unit: (),
            },
        ]
    } else {
        [
            asset_metric("waste", "Waste", "waste", state.balance("workshop", "waste")),
            asset_metric("time", "Process time", "spent-time", state.balance("workshop", "spent-time")),
        ]
    };
    scene
}
