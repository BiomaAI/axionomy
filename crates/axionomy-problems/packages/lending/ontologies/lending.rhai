let world = ontology();

for name in [
    "borrower",
    "clock",
    "customer",
    "depositor/0",
    "depositor/1",
    "depositor/2",
    "depositor/3",
    "keeper",
    "market",
    "oracle",
    "supplier"
] {
    world.declare(account(name));
}

for name in [
    "bill-paid",
    "borrow-index",
    "cash",
    "collateral",
    "collateral-price",
    "debt-share",
    "debt-share-supply",
    "elapsed-period",
    "invoice",
    "locked-share",
    "pending-shock",
    "period",
    "posted-collateral",
    "purchase-order/0",
    "purchase-order/1",
    "purchase-order/2",
    "reserves",
    "share",
    "share-supply"
] {
    world.declare(asset(name));
}

for name in ["borrower", "clock", "customer", "depositor", "keeper", "market", "oracle", "supplier"] {
    world.declare(role(name));
}

for name in [
    "accrue",
    "borrow",
    "collect-revenue/{installment}",
    "deposit",
    "donate",
    "liquidate",
    "pay-bill",
    "post-collateral",
    "redeem",
    "repay",
    "repay-shares",
    "reprice",
    "withdraw-collateral"
] {
    world.rate_pattern(name);
}

for name in ["min-cash", "min-shares"] {
    world.parameter(name);
}

world
