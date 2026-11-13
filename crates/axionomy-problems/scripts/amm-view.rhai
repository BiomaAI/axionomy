// The Living Market: how people read the closed endogenous market. Labels
// and formats name its identifiers; the scene draws the energy / credit pool
// and the actors trading against it; and the leaderboard ranks each actor's
// direct contribution to the price, carried over the trace. The actors, the
// price and the pool's tallies are the market's economy script's.

import "amm" as amm;

fn actor_label(actor) {
    switch actor {
        "founder" => "Founding LP",
        "generator" => "Solar generator",
        "factory" => "Factory",
        "household" => "Household",
        "speculator" => "Informed speculator",
        "adaptive-lp" => "Adaptive LP",
        "arbitrageur" => "Internal arbitrageur",
        "whale" => "Demand whale",
    }
}

// A day as people read it, counting from one.
fn ordinal(day) { parse_int(day) + 1 }

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return switch name {
            "pool" => "Energy / credit AMM",
            "treasury" => "Closed-economy treasury",
            "information" => "Scarcity information",
            "actor" => actor_label(parts[2]),
            "world" => "Market calendar and production conditions",
            _ => (),
        };
    }
    if category == "role" {
        return switch name {
            "pool" => "Pool",
            "trader" => "Trader",
            "liquidity-provider" => "Liquidity provider",
            "producer" => "Producer",
            "consumer" => "Consumer",
            "treasury" => "Treasury",
            "information" => "Information source",
            "observer" => "Observer",
            "world" => "Market calendar",
            "participant" => actor_label(parts[2]),
            _ => (),
        };
    }
    if category == "asset" {
        return switch name {
            "actor-identity" => `Identity: ${actor_label(parts[2])}`,
            "pool-identity" => "Identity: energy pool",
            "treasury-identity" => "Identity: procurement treasury",
            "world-identity" => "Identity: market calendar",
            "market-active" => "Market season active",
            "market-ended" => "Market season closed",
            "day" => "Time elapsed",
            "at-day" => `Market day ${ordinal(parts[2])}`,
            "days" => "Season length",
            "scale" => "Production scale",
            "market-open" => "Market access open",
            "trading" => "Still trading today",
            "done" => "Trading finished for today",
            "decisions" => "Decisions remaining today",
            "storage-capacity" => "Energy storage capacity",
            "production-capacity" => "Generating capacity",
            "production-allowance" => "Remaining production today",
            "future-capacity" => "Capacity left to build",
            "pending-capacity" => "Capacity under construction",
            "construction-due" => "Construction completes at",
            "reference-price" => "Smoothed prior closing price",
            "sunlight" => `Day ${ordinal(parts[2])} sunlight`,
            "current-sunlight" => "Current sunlight",
            "liquidity-decision" => "Liquidity adjustment available",
            "need" => `${actor_label(parts[2])} need due today`,
            "future-need" => `${actor_label(parts[2])} needs still to come`,
            "satisfied-need" => `${actor_label(parts[2])} needs met`,
            "missed-need" => `${actor_label(parts[2])} needs unmet`,
            "obligation" => `${actor_label(parts[2])} obligation due today`,
            "future-obligation" => `${actor_label(parts[2])} obligations still to come`,
            "settled-obligation" => `${actor_label(parts[2])} obligations settled`,
            "defaulted-obligation" => `${actor_label(parts[2])} obligations missed`,
            "unmade-goods" => "Goods still to make",
            "goods" => "Manufactured goods awaiting sale",
            "delivered-goods" => "Goods delivered to procurement",
            "income-due" => "Income payment due today",
            "future-income" => "Income payments still to come",
            "received-income" => "Income payments received",
            "missed-income" => "Income payments missed",
            "energy" => "Usable energy",
            "credit" => "Settlement credit",
            "solar-potential" => "Unharvested solar potential",
            "spent-energy" => "Energy consumed toward goals",
            "lp-share" => "AMM liquidity share",
            "unissued-lp-share" => "Unissued liquidity share",
            "uninformed" => `${actor_label(parts[2])} has no shortage signal`,
            "informed" => `${actor_label(parts[2])} knows about the shortage`,
            "shortage-signal" => "Verified energy shortage signal",
            "utility" => "Realized utility",
            _ => (),
        };
    }
    switch name {
        "end-trading" => `${actor_label(parts[2])} finishes trading today`,
        "advance-day" => `Close day ${ordinal(parts[2])} and open day ${ordinal(parts[2]) + 1}`,
        "close-market" => "Close the market season and record unmet needs",
        "invest-production" => "Fund new generating capacity",
        "commission-production" => "Bring completed generating capacity online",
        "deliver-goods" => "Factory sells manufactured goods",
        "collect-income" => "Household collects its daily income",
        "fund-liquidity" => "Add liquidity at current reserves with bounded rounding",
        "buy-energy" => "Buy energy from the AMM",
        "sell-energy" => "Sell energy into the AMM",
        "add-liquidity" => "Deepen AMM liquidity",
        "remove-liquidity" => "Withdraw AMM liquidity",
        "produce-energy" => "Harvest solar energy",
        "use-energy" => `${actor_label(parts[2])} uses energy`,
        "settle-obligation" => `${actor_label(parts[2])} settles a credit obligation`,
        "learn-shortage" => `${actor_label(parts[2])} receives the shortage signal`,
        _ => (),
    }
}

// The unit quantities of `asset` are read in, or nothing for a count.
fn unit(asset) {
    switch asset.split("/")[0] {
        "energy" | "spent-energy" | "solar-potential" | "storage-capacity" | "production-allowance" => "units",
        "production-capacity" | "future-capacity" | "pending-capacity" => "units / day",
        "credit" => "credits",
        "reference-price" => "credit / energy",
        "day" | "days" | "construction-due" => "days",
        "sunlight" | "current-sunlight" => "%",
        _ => (),
    }
}

// The reference price is kept in thousandths of a credit per unit of energy.
fn format(asset) {
    let unit = unit(asset);
    if unit == () { () }
    else if asset == "reference-price" { #{ scale: 3, unit: unit } }
    else { #{ scale: 0, unit: unit } }
}

// A metric showing `value`, in `unit` if it has one.
fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

// A metric showing a quantity of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    metric(key, label, quantity, unit(asset))
}

// Thousandths written with their three digits, signed when `signed` and
// not zero: `12345` is `12.345`, or `+12.345`.
fn milli(value, signed) {
    let sign = if value > 0 && signed { "+" } else if value < 0 { "-" } else { "" };
    let magnitude = value.abs();
    let fraction = `${magnitude % 1000}`;
    while fraction.len() < 3 { fraction = "0" + fraction; }
    `${sign}${magnitude / 1000}.${fraction}`
}

fn read(state, account, asset) { state.balance(account, asset).to_int() }

// The actor whose trade or liquidity an exchange is, if any: the trader
// of a swap, the provider of a liquidity change.
fn price_actor(exchange) {
    let rate = exchange.rate();
    let role = if rate == "buy-energy" || rate == "sell-energy" { "trader" }
        else if rate == "add-liquidity" || rate == "remove-liquidity" || rate == "fund-liquidity" { "liquidity-provider" }
        else { return (); };
    let account = exchange.account(role);
    if account == () || !account.starts_with("actor/") { return (); }
    account.sub_string(6)
}

// Each actor's cumulative change in the price, carried from frame to frame.
fn carry(carried, before, exchange, after) {
    let contributions = carried;
    if contributions == () {
        contributions = #{};
        for actor in amm::actors() { contributions[actor] = 0; }
    }
    let actor = price_actor(exchange);
    if actor != () {
        contributions[actor] += amm::price_milli(after).to_int() - amm::price_milli(before).to_int();
    }
    contributions
}

fn leaderboards(state, frame) {
    let contributions = frame.carried;
    // Ties keep the actors' identifier order.
    let ordered = amm::actors();
    ordered.sort();
    let entries = ordered.map(|actor| {
        let value = if contributions == () { 0 } else { contributions[actor] };
        #{
            participant: #{ key: `amm/account/actor/${actor}`, label: actor_label(actor) },
            score: value,
            value: milli(value, true),
            unit: "Δ credit / energy",
            eligible: true,
            components: [],
        }
    });
    [#{
        key: "price-contribution",
        label: "Direct price contribution",
        description: "Each actor's cumulative change in the replayed reserve price. Contributions sum exactly to total price movement because proportional liquidity does not move price.",
        direction: "maximize",
        entries: entries,
    }]
}

fn glyph(actor) {
    switch actor {
        "founder" | "adaptive-lp" => "organization",
        "generator" => "energy",
        "factory" => "machine",
        "household" => "person",
        "speculator" => "information",
        "arbitrageur" => "move",
        "whale" => "money",
    }
}

fn status(state, actor) {
    let account = `actor/${actor}`;
    if state.has_account("world") {
        return switch actor {
            "generator" => `${read(state, account, "production-capacity")} capacity · ${read(state, account, "pending-capacity")} building`,
            "factory" | "household" => `${read(state, account, `satisfied-need/${actor}`)} needs met · ${read(state, account, `missed-need/${actor}`)} missed`,
            "founder" | "adaptive-lp" => `${read(state, account, "lp-share")} liquidity shares`,
            "speculator" if read(state, account, `informed/${actor}`) > 0 => "next-day shortage observed",
            _ => `${read(state, account, "energy")} stored energy`,
        };
    }
    if read(state, account, `satisfied-need/${actor}`) > 0 { "need satisfied" }
    else if read(state, account, `informed/${actor}`) > 0 { "shortage informed" }
    else if read(state, account, `settled-obligation/${actor}`) > 0 { "obligation settled" }
    else if read(state, account, "lp-share") > 0 { "liquidity provider" }
    else { () }
}

fn tone(state, actor, status) {
    let account = `actor/${actor}`;
    if read(state, account, `missed-need/${actor}`) > 0 || read(state, account, `defaulted-obligation/${actor}`) > 0 {
        "warning"
    } else if status != () {
        "success"
    } else if actor == "whale" || actor == "speculator" {
        "warning"
    } else {
        "neutral"
    }
}

fn scene(state, frame) {
    let energy = state.balance("pool", "energy");
    let credit = state.balance("pool", "credit");
    let issued = amm::issued(state);
    let price = amm::price_milli(state).to_int();
    let positions = [[12.0, 17.0], [11.0, 44.0], [13.0, 72.0], [36.0, 87.0], [64.0, 87.0], [87.0, 72.0], [89.0, 44.0], [88.0, 17.0]];
    let participants = [];
    let entities = [];
    for (actor, index) in amm::actors() {
        let account = `amm/account/actor/${actor}`;
        let id = #{ key: `amm/actor/${actor}`, label: actor_label(actor) };
        let status = status(state, actor);
        let tone = tone(state, actor, status);
        let held_energy = state.balance(`actor/${actor}`, "energy");
        let held_credit = state.balance(`actor/${actor}`, "credit");
        participants.push(#{
            id: id, account: account, glyph: glyph(actor), tone: tone,
            x: positions[index][0], y: positions[index][1],
            base: held_energy.to_string(), quote: held_credit.to_string(), status: status,
        });
        entities.push(#{
            id: id, glyph: glyph(actor), anchor: #{ kind: "unanchored" }, role: "occupant",
            tone: tone, status: status, account: account,
            evidence: [#{ kind: "account", account: account }],
            metrics: [
                asset_metric("energy", "Energy", "energy", held_energy),
                asset_metric("credit", "Credit", "credit", held_credit),
            ],
        });
    }
    let metrics = [
        metric("price", "Discovered price", milli(price, false), "credit / energy"),
        asset_metric("energy-reserve", "Energy reserve", "energy", energy),
        asset_metric("liquidity", "Liquidity shares issued", "lp-share", issued),
    ];
    if state.has_account("world") {
        metrics.insert(0, metric("day", "Market day", read(state, "world", "day") + 1, "day"));
        metrics += [
            metric("unmet", "Unmet daily needs", amm::unmet(state), "needs"),
            metric("defaults", "Missed obligations", amm::defaulted(state), "payments"),
            asset_metric("sunlight", "Sunlight", "current-sunlight", state.balance("world", "current-sunlight")),
            asset_metric("capacity", "Generating capacity", "production-capacity", state.balance("actor/generator", "production-capacity")),
            asset_metric("goods", "Goods delivered", "delivered-goods", state.balance("treasury", "delivered-goods")),
        ];
    }
    // The pool of energy and credit the actors trade against: its reserve
    // ratio is the market's only public price, and its reserves stay on the
    // constant-product curve.
    #{
        title: "The Living Market",
        surface: #{
            kind: "reserves",
            pool: #{
                id: #{ key: "amm/pool/energy-credit", label: "Energy / Credit AMM" },
                account: "amm/account/pool",
                caption: "AMM pool · x · y = k",
                base: #{ asset: #{ key: "amm/asset/energy", label: "Energy" }, quantity: energy.to_string(), symbol: "E" },
                quote: #{ asset: #{ key: "amm/asset/credit", label: "Credit" }, quantity: credit.to_string(), symbol: "C" },
                ratio: #{
                    label: "Discovered price",
                    value: `${price}`,
                    format: #{ scale: 3, digits: 3, unit: "credit / energy" },
                },
                curve: #{ kind: "constant_product", label: "Reserve curve · xy = k", product: (energy * credit).to_string() },
            },
            participants: participants,
        },
        entities: entities,
        paths: [],
        annotations: [],
        metrics: metrics,
        legend: [],
    }
}
