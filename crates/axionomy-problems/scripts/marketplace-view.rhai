// Marketplace: how people read the multi-party marketplace. Labels and
// formats name its identifiers; the scene draws buyers, sellers, carriers,
// the platform and the tax authority around the orders, each binding the
// kernel would settle now, each contract booked and each settlement made,
// with the funds, deadlines and deliveries of a market that runs on a
// clock.

// A letter as people read it: `a` is `A`.
fn letter(name) { name.to_upper() }

// How a contract's price was agreed: in full, or with the 10-credit
// discount.
fn priced(discounted) { if discounted == "true" { "discounted" } else { "full-price" } }

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return switch name {
            "buyer" => `Buyer ${letter(parts[2])}`,
            "seller" => `Seller ${letter(parts[2])}`,
            "platform" => "Marketplace platform",
            "tax-authority" => "Tax authority",
            "carrier" => `Carrier ${letter(parts[2])}`,
            "order" => `Order ${letter(parts[2])}`,
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "buyer" => "Buyer",
            "seller" => "Seller",
            "platform" => "Platform",
            "tax-authority" => "Tax authority",
            "carrier" => "Carrier",
            "order" => "Order",
            _ => (),
        };
    }
    if category == "asset" {
        return switch name {
            "money" => "Money",
            "item" => `${parts[2].sub_string(0, 1).to_upper()}${parts[2].sub_string(1)} inventory`,
            "purchase-intent" => `Wants Order ${letter(parts[2])}`,
            "purchase-receipt" => `Bought Order ${letter(parts[2])}`,
            "sale-offer" => `Offers one ${parts[2]}`,
            "completed-sale" => `Sold Order ${letter(parts[2])}`,
            "marketplace-license" => "Marketplace licence",
            "tax-policy" => "Tax policy",
            "shipping-capacity" => "Shipping capacity left",
            "used-shipping-capacity" => "Shipping capacity used",
            "open-order" => `Order ${letter(parts[2])} is open`,
            "settled-order" => `Order ${letter(parts[2])} settled`,
            "settled-value" => `Settled for ${parts[2]} credits`,
            "utility" => "Participant benefit",
            "buyer-license" => "Buyer identity",
            "seller-license" => "Seller identity",
            "carrier-license" => "Carrier identity",
            "contract" => `Order ${letter(parts[2])} reserved`,
            "in-shipment" => `Order ${letter(parts[2])} in transit`,
            "purchase-locked" => `Buyer committed to Order ${letter(parts[2])}`,
            "sale-reserved" => `Seller committed to Order ${letter(parts[2])}`,
            "booking" => `Carrier booked for Order ${letter(parts[2])}`,
            "delivered-by" => `Delivered Order ${letter(parts[2])}`,
            "agreed-price" => `Agreed price of ${parts[2]} credits`,
            "arrival" => "Order release time",
            "deadline" => "Delivery deadline",
            "lease-until" => "Dispatch reservation expires at",
            "ready-at" => "Delivery available at",
            "now" => "World time",
            "time-remaining" => "Time remaining",
            "transit-time" => "Base shipping duration",
            "delay-risk" => "Exposure to route delays",
            "route-delay" => "Route delay",
            "on-time" => "On-time deliveries",
            "late" => "Late deliveries",
            "refunded" => "Expired reservations refunded",
            _ => (),
        };
    }
    switch name {
        "settle-order" => `Agree Order ${letter(parts[2])} at full price`,
        "discounted-contract" => `Agree Order ${letter(parts[2])} at a 10-credit discount`,
        "dispatch" => `Dispatch Order ${letter(parts[2])}`,
        "deliver" => `Deliver ${priced(parts[3])} Order ${letter(parts[2])} ${
            if parts[4] == "true" { "late with a rebate" } else { "on time" }
        }`,
        "cancel" => `Refund the expired ${priced(parts[3])} reservation for Order ${letter(parts[2])}`,
        "advance" => `Advance market time to ${parts[2]}`,
        _ => (),
    }
}

// The unit quantities of `asset` are read in, or nothing for a count:
// money in credits, and the platform's clock and every time it keeps in
// ticks.
fn unit(asset) {
    switch asset {
        "money" => "credits",
        "arrival" | "deadline" | "lease-until" | "ready-at" | "now" | "time-remaining"
            | "transit-time" | "route-delay" => "ticks",
        _ => (),
    }
}

fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

fn held(state, account, asset) { state.balance(account, asset).to_int() }

// The market's orders, one for each settlement rate, in identifier order.
fn order_ids(state) {
    state.rates("settle-order/").map(|rate| rate.sub_string(13))
}

// Whether `order` has settled.
fn settled(state, order) { held(state, `order/${order}`, `settled-order/${order}`) > 0 }

// The first of `ids` whose `prefix` account holds some of `asset`, or nothing.
fn holder(state, prefix, ids, asset) {
    for id in ids {
        if held(state, `${prefix}/${id}`, asset) > 0 { return id; }
    }
    ()
}

// The settlement of `order` binding a buyer, a seller and a carrier.
fn settlement(order, buyer, seller, carrier) {
    let bindings = #{
        buyer: `buyer/${buyer}`,
        seller: `seller/${seller}`,
        platform: "platform",
        carrier: `carrier/${carrier}`,
        order: `order/${order}`,
    };
    bindings["tax-authority"] = "tax-authority";
    exchange(`settle-order/${order}`, bindings, 1)
}

// The id of each account under `prefix`, in identifier order.
fn ids(state, prefix) {
    state.accounts(`${prefix}/`).map(|account| account.sub_string(prefix.len() + 1))
}

fn node(key, label, class, x, y) {
    #{ id: #{ key: key, label: label }, classes: [class], x: x, y: y }
}

fn edge(id, source, target, label, classes) {
    #{ id: id, source: source, target: target, label: label, classes: classes }
}

fn entity(key, label, glyph, node, tone, status, account) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph, anchor: #{ kind: "graph_node", node: node }, role: "state",
        tone: tone, status: status,
        account: `marketplace/account/${account}`,
        evidence: [#{ kind: "account", account: `marketplace/account/${account}` }],
        metrics: [],
    }
}

fn scene(state, frame) {
    let timed = state.rates("dispatch/").contains("dispatch/a");
    let orders = order_ids(state);
    let nodes = [];
    for (buyer, index) in ["a", "b", "c"] {
        nodes.push(node(`buyer/${buyer}`, `Buyer ${letter(buyer)}`, "buyer", 30.0, 60.0 + index.to_float() * 95.0));
    }
    for (seller, index) in ["a", "b", "c"] {
        nodes.push(node(`seller/${seller}`, `Seller ${letter(seller)}`, "seller", 760.0, 60.0 + index.to_float() * 95.0));
    }
    nodes.push(node("platform", "Platform", "resource", 510.0, 30.0));
    nodes.push(node("tax", "Tax authority", "resource", 650.0, 30.0));
    for (carrier, index) in ["a", "b"] {
        nodes.push(node(`carrier/${carrier}`, `Carrier ${letter(carrier)}`, "carrier", 510.0, 200.0 + index.to_float() * 85.0));
    }
    for (order, index) in orders {
        nodes.push(node(
            `order/${order}`, `Order ${letter(order)}`, if settled(state, order) { "completed" } else { "resource" },
            265.0, 55.0 + index.to_float() * 78.0,
        ));
    }

    // Every binding the kernel would settle now, every contract booked and
    // every settlement made, each once, as `order/buyer/seller/carrier`.
    let buyers = ids(state, "buyer");
    let sellers = ids(state, "seller");
    let carriers = ids(state, "carrier");
    let feasible = 0;
    let matches = [];
    for order in orders {
        for buyer in buyers {
            for seller in sellers {
                for carrier in carriers {
                    if state.applicable(settlement(order, buyer, seller, carrier)) {
                        feasible += 1;
                        matches.push(`${order}/${buyer}/${seller}/${carrier}`);
                    }
                }
            }
        }
    }
    for order in orders {
        let buyer = holder(state, "buyer", ["a", "b", "c"], `purchase-locked/${order}`);
        let seller = holder(state, "seller", ["a", "b", "c"], `sale-reserved/${order}`);
        let carrier = holder(state, "carrier", ["a", "b"], `booking/${order}`);
        if buyer != () && seller != () && carrier != () {
            matches.push(`${order}/${buyer}/${seller}/${carrier}`);
        }
        let buyer = holder(state, "buyer", ["a", "b", "c"], `purchase-receipt/${order}`);
        let seller = holder(state, "seller", ["a", "b", "c"], `completed-sale/${order}`);
        if buyer != () && seller != () {
            let carrier = holder(state, "carrier", ["a", "b"], `delivered-by/${order}`);
            matches.push(`${order}/${buyer}/${seller}/${if carrier == () { "a" } else { carrier }}`);
        }
    }
    matches.sort();
    matches.dedup();
    let edges = [];
    for binding in matches {
        let parts = binding.split("/");
        let order = parts[0];
        let done = settled(state, order);
        let classes = if done { ["completed"] } else { ["uncertain"] };
        edges.push(edge(
            `candidate/${binding}/in`, `buyer/${parts[1]}`, `order/${order}`, `via ${letter(parts[3])}`, classes,
        ));
        edges.push(edge(
            `candidate/${binding}/out`, `order/${order}`, `seller/${parts[2]}`,
            if done { "atomic split" } else { "feasible match" }, classes,
        ));
        if done {
            for part in [
                ["platform", "platform", "commission"],
                ["tax", "tax", "tax"],
                ["carrier", `carrier/${parts[3]}`, "shipping fee"],
            ] {
                edges.push(edge(`settlement/${order}/${part[0]}`, `order/${order}`, part[1], part[2], ["completed"]));
            }
        }
    }

    let entities = [];
    for order in orders {
        let account = `order/${order}`;
        let done = settled(state, order);
        let token = entity(
            `order-token/${order}`, `Order ${letter(order)}`, "package",
            if done && !timed { "platform" } else { `order/${order}` },
            if done { "success" } else { "active" },
            if done { "delivered and settled" }
            else if held(state, account, `in-shipment/${order}`) > 0 { "in transit; funds held" }
            else if held(state, account, `contract/${order}`) > 0 { "reserved; awaiting dispatch" }
            else { "awaiting match" },
            account,
        );
        if timed {
            token.metrics = [
                metric("escrow", "Funds held", state.balance(account, "money"), "credits"),
                metric("deadline", "Deadline", state.balance(account, "deadline"), "ticks"),
            ];
        }
        entities.push(token);
    }
    for buyer in ["a", "b", "c"] {
        entities.push(entity(
            `actor/buyer/${buyer}`, `Buyer ${letter(buyer)}`, "person", `buyer/${buyer}`, "neutral", "demand",
            `buyer/${buyer}`,
        ));
    }
    for seller in ["a", "b", "c"] {
        entities.push(entity(
            `actor/seller/${seller}`, `Seller ${letter(seller)}`, "organization", `seller/${seller}`, "neutral",
            "supply", `seller/${seller}`,
        ));
    }
    for carrier in ["a", "b"] {
        entities.push(entity(
            `actor/carrier/${carrier}`, `Carrier ${letter(carrier)}`, "vehicle", `carrier/${carrier}`, "neutral",
            "capacity", `carrier/${carrier}`,
        ));
    }
    entities.push(entity(
        "actor/platform", "Platform", "organization", "platform", "neutral", "settlement role", "platform",
    ));
    entities.push(entity(
        "actor/tax", "Tax authority", "organization", "tax", "neutral", "settlement role", "tax-authority",
    ));

    let closed = orders.filter(|order| settled(state, order)).len();
    let scene = scene::entities(
        scene::graph("Contracts, shipping and settlement", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        metric("open", "Open orders", orders.len() - closed, "orders"),
        metric("settled", "Settled orders", closed, "orders"),
        metric("feasible", "Feasible bindings", feasible, "matches"),
    ];
    if timed {
        scene.metrics += [
            metric("world-time", "World time", state.balance("platform", "now"), "ticks"),
            metric("escrow", "Funds held in contracts", total(state, orders, "money", false), "credits"),
            metric("in-transit", "Shipments underway", total(state, orders, "in-shipment", true), ()),
            metric("ontime", "On-time deliveries", total(state, orders, "on-time", false), ()),
            metric("late", "Late deliveries", total(state, orders, "late", false), ()),
        ];
    }
    scene
}

// What the orders' accounts hold of `asset` together, or of each order's
// own `asset/<order>` when `own`.
fn total(state, orders, asset, own) {
    let sum = 0;
    for order in orders {
        sum += held(state, `order/${order}`, if own { `${asset}/${order}` } else { asset });
    }
    sum
}
