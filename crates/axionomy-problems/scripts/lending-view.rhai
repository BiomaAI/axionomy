// Lending: how people read the lending market. Labels and formats name its
// identifiers; the scene draws who lends, borrows and is paid: the market's
// cash, the borrower's collateral and loan, what a keeper seized, and the
// market's terms at this state, each computed as the market computes it.

// A name as people read it: `keeper` is `Keeper`.
fn words(name) {
    let parts = [];
    for part in name.split("-") {
        parts.push(part.sub_string(0, 1).to_upper() + part.sub_string(1));
    }
    parts.reduce(|sum, part| if sum == () { part } else { sum + " " + part })
}

// The fixed-point scale of the borrow index and every market term (10^18).
fn wad() {
    quantity(1_000_000_000_000_000_000)
}

// A numbered identifier counted from one: `depositor/0` is Depositor 1.
fn ordinal(number) {
    parse_int(number) + 1
}

fn asset_label(name, parts) {
    switch name {
        "cash" => "Cash",
        "share" => "Vault share",
        "locked-share" => "Locked genesis share",
        "share-supply" => "Vault share supply",
        "debt-share" => "Debt share",
        "debt-share-supply" => "Debt share supply",
        "borrow-index" => "Borrow index",
        "reserves" => "Reserves",
        "collateral" => "Collateral",
        "posted-collateral" => "Posted collateral",
        "collateral-price" => "Collateral price",
        "base-rate" => "Base rate",
        "kink" => "Utilization kink",
        "slope-below-kink" => "Rate slope below the kink",
        "slope-above-kink" => "Rate slope above the kink",
        "reserve-factor" => "Reserve factor",
        "loan-to-value" => "Loan-to-value limit",
        "liquidation-threshold" => "Liquidation threshold",
        "liquidation-bonus" => "Liquidation bonus",
        "credit-line" => "Credit line",
        "invoice" => "Supplier invoice",
        "bill-paid" => "Bill paid",
        "purchase-order" => `Purchase order ${ordinal(parts[2])}`,
        "market-identity" => "Market identity",
        "clock-identity" => "Clock identity",
        "oracle-identity" => "Oracle identity",
        "period" => "Time remaining",
        "elapsed-period" => "Time elapsed",
        "pending-shock" => "Pending price drop",
        "shock-after" => "Price drop after",
        "price-drop" => "Price drop",
        _ => (),
    }
}

fn rate_label(name, parts) {
    switch name {
        "deposit" => "Deposit cash for vault shares",
        "redeem" => "Redeem vault shares for cash",
        "donate" => "Donate cash to the market",
        "post-collateral" => "Post collateral",
        "withdraw-collateral" => "Withdraw collateral",
        "borrow" => "Borrow cash",
        "repay" => "Repay cash",
        "repay-shares" => "Repay the whole loan",
        "pay-bill" => "Pay the supplier's bill",
        "collect-revenue" => `Collect installment ${ordinal(parts[2])}`,
        "accrue" => "Close the period and accrue interest",
        "reprice" => "Apply the scheduled price drop",
        "liquidate" => "Liquidate the loan",
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "market" => "Lending market",
            "oracle" => "Price oracle",
            "depositor" => `Depositor ${ordinal(parts[2])}`,
            _ => words(parts[1]),
        },
        "asset" => asset_label(parts[1], parts),
        "rate" => rate_label(parts[1], parts),
        _ => words(parts[1]),
    }
}

// The market's terms are basis points scaled by WAD, so 10^16 atoms are one
// percent; the borrow index is a WAD-scaled multiplier. Values the market
// keeps in cash read as cash, and the clock counts periods.
fn format(asset) {
    switch asset.split("/")[0] {
        "borrow-index" => #{ scale: 18 },
        "base-rate" | "kink" | "slope-below-kink" | "slope-above-kink" | "reserve-factor"
            | "loan-to-value" | "liquidation-threshold" | "liquidation-bonus" => #{ scale: 16, unit: "%" },
        "cash" | "reserves" | "collateral-price" | "price-drop" => #{ scale: 0, unit: "cash" },
        "period" | "elapsed-period" | "shock-after" => #{ scale: 0, unit: "periods" },
        _ => (),
    }
}

fn market_balance(state, asset) {
    state.balance("market", asset)
}

// Outstanding debt valued rounded down.
fn debt_value(state) {
    mul_div(market_balance(state, "debt-share-supply"), market_balance(state, "borrow-index"), wad(), "floor")
}

// Market cash plus outstanding debt, less reserves.
fn total_assets(state) {
    market_balance(state, "cash") + debt_value(state) - market_balance(state, "reserves")
}

// Cash the borrower owes now, rounded up.
fn debt(state) {
    mul_div(state.balance("borrower", "debt-share"), market_balance(state, "borrow-index"), wad(), "ceil")
}

// Borrowed share of cash plus debt, scaled by WAD, rounded up.
fn utilization(state) {
    let lent = debt_value(state);
    let gross = max(market_balance(state, "cash") + lent, quantity(1));
    mul_div(lent, wad(), gross, "ceil")
}

// Interest per period at the current utilization, scaled by WAD.
fn interest(state) {
    let used = utilization(state);
    let kink = market_balance(state, "kink");
    market_balance(state, "base-rate")
        + mul_div(min(used, kink), market_balance(state, "slope-below-kink"), wad(), "ceil")
        + mul_div(max(used, kink) - kink, market_balance(state, "slope-above-kink"), wad(), "ceil")
}

// A share of the posted collateral's value, rounded down.
fn limit(state, term) {
    let value = state.balance("borrower", "posted-collateral") * market_balance(state, "collateral-price");
    mul_div(value, market_balance(state, term), wad(), "floor")
}

// A WAD-scaled share as a percentage with two decimals.
fn percent(value) {
    let hundredths = mul_div(value, quantity(10_000), wad(), "floor").to_int();
    let cents = hundredths % 100;
    `${hundredths / 100}.${if cents < 10 { "0" } else { "" }}${cents}`
}

fn node(key, label, classes, x, y) {
    #{ id: #{ key: key, label: label }, classes: classes, x: x, y: y }
}

fn edge(id, source, target, label, active) {
    #{
        id: id, source: source, target: target, label: label,
        classes: if active { ["active"] } else { [] },
    }
}

fn entity(key, label, glyph, node, tone, status) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph,
        anchor: #{ kind: "graph_node", node: node },
        role: "state",
        tone: tone,
        status: status,
    }
}

// The entity citing the balance of `asset` when `account` holds some,
// otherwise the account that would hold it.
fn evidence(entity, state, account, asset) {
    let key = `lending/account/${account}`;
    entity.account = key;
    entity.evidence = if state.balance(account, asset) > 0 {
        [#{ kind: "balance", account: key, asset: `lending/asset/${asset}` }]
    } else {
        [#{ kind: "account", account: key }]
    };
    entity
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

fn scene(state, frame) {
    let owed = debt(state);
    let liquidation_limit = limit(state, "liquidation-threshold");
    let unsafe_loan = owed > liquidation_limit;
    let open_loan = owed > 0;
    let bill_paid = state.balance("borrower", "bill-paid") > 0;
    let pending_installments = state.balance("customer", "cash");
    let seized = state.balance("keeper", "collateral");
    let borrower_classes = if unsafe_loan { ["danger"] } else if open_loan { ["active"] } else { [] };
    let nodes = [
        node("oracle", "Price oracle", ["information"], 380.0, 0.0),
        node("market", "Lending market", ["active"], 380.0, 200.0),
        node("borrower", "Borrower", borrower_classes, 720.0, 200.0),
        node("supplier", "Supplier", [if bill_paid { "success" } else { "goal" }], 1060.0, 80.0),
        node("customer", "Customer", [], 1060.0, 320.0),
        node("keeper", "Keeper", if seized > 0 { ["warning"] } else { [] }, 380.0, 400.0),
    ];
    let edges = [
        edge("loan", "market", "borrower", "loan", open_loan),
        edge("bill", "borrower", "supplier", "bill", bill_paid),
        edge("revenue", "customer", "borrower", "installments", pending_installments == 0),
        edge("price", "oracle", "market", "collateral price", false),
        edge("liquidation", "keeper", "market", "liquidation", seized > 0),
    ];
    for (account, row) in state.accounts("depositor/") {
        let depositor = account.split("/")[1];
        let key = `depositor/${depositor}`;
        nodes.push(node(key, `Depositor ${ordinal(depositor)}`, ["person"], 0.0, row.to_float() * 130.0 + 40.0));
        edges.push(edge(`vault/${depositor}`, key, "market", "vault shares", state.balance(account, "share") > 0));
    }

    let entities = [
        evidence(entity(
            "market-cash", "Market cash", "money", "market", "active",
            `${market_balance(state, "cash")} cash on hand`,
        ), state, "market", "cash"),
        evidence(entity(
            "posted-collateral", "Posted collateral", "shield", "borrower",
            if unsafe_loan { "danger" } else { "neutral" },
            `${state.balance("borrower", "posted-collateral")} posted, ${state.balance("borrower", "collateral")} free`,
        ), state, "borrower", "posted-collateral"),
    ];
    if open_loan {
        entities.push(evidence(entity(
            "debt", "Loan", "token", "borrower",
            if unsafe_loan { "danger" } else { "warning" },
            `owes ${owed}, liquidated above ${liquidation_limit}`,
        ), state, "borrower", "debt-share"));
    }
    if seized > 0 {
        entities.push(evidence(entity(
            "seized", "Seized collateral", "shield", "keeper", "warning", `${seized} seized`,
        ), state, "keeper", "collateral"));
    }

    let elapsed = state.balance("clock", "elapsed-period");
    let remaining = state.balance("clock", "period");
    let scene = scene::entities(scene::graph("Who lends, borrows and is paid", nodes, edges, ()), entities);
    scene.metrics = [
        metric("period", "Period", `${elapsed + 1} of ${elapsed + remaining}`, ()),
        metric("utilization", "Utilization", percent(utilization(state)), "%"),
        metric("rate", "Interest per period", percent(interest(state)), "%"),
        metric("owed", "Borrower owes", owed, "cash"),
        metric("borrow-limit", "Borrowing limit", limit(state, "loan-to-value"), "cash"),
        metric("liquidation-limit", "Liquidation threshold", liquidation_limit, "cash"),
        metric("price", "Collateral price", market_balance(state, "collateral-price"), "cash"),
        metric("reserves", "Reserves", market_balance(state, "reserves"), "cash"),
        metric("share-assets", "Depositors' assets", total_assets(state), "cash"),
    ];
    scene
}
