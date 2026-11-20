let model = #{
    ontology: "amm",
    laws: ["amm/showcase", "amm/showcase-and-stress"],
    tables: #{
        "advance-day/{day}": #{
            variables: [#{name: "day", range: ["0", "6"]}],
            relations: [
                #{
                    gives: ["sunlight", "shortage"],
                    rows: [
                        ["0", "40", "0"],
                        ["1", "100", "0"],
                        ["2", "100", "1"],
                        ["3", "40", "0"],
                        ["4", "100", "1"],
                        ["5", "40", "0"],
                        ["6", "100", "0"]
                    ],
                    variables: ["day"]
                }
            ]
        }
    },
    constants: #{
        batch: "100",
        "daily-decisions": "6",
        "fee-denominator": "1000",
        "fee-numerator": "997",
        "project-capacity": "400",
        scale: "1",
        "season-length": "8",
        "share-limit": "1000000"
    }
};
let accounts = [];
accounts.push(opening(account("actor/adaptive-lp"), basket([
    amount(asset("credit"), 20000),
    amount(asset("decisions"), 6),
    amount(asset("energy"), 2000),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 3000),
    amount(asset("trading"), 1)
])));
accounts.push(opening(account("actor/arbitrageur"), basket([
    amount(asset("credit"), 10000),
    amount(asset("decisions"), 6),
    amount(asset("energy"), 2500),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 3000),
    amount(asset("trading"), 1)
])));
accounts.push(opening(account("actor/factory"), basket([
    amount(asset("credit"), 40000),
    amount(asset("decisions"), 6),
    amount(asset("future-need/factory"), 7),
    amount(asset("future-obligation/factory"), 7),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("need/factory"), 1),
    amount(asset("obligation/factory"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 2500),
    amount(asset("trading"), 1),
    amount(asset("unmade-goods"), 800)
])));
accounts.push(opening(account("actor/founder"), basket([
    amount(asset("credit"), 5000),
    amount(asset("decisions"), 6),
    amount(asset("liquidity-decision"), 1),
    amount(asset("lp-share"), 10000),
    amount(asset("market-open"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 20000),
    amount(asset("trading"), 1)
])));
accounts.push(opening(account("actor/generator"), basket([
    amount(asset("credit"), 5000),
    amount(asset("decisions"), 6),
    amount(asset("energy"), 3000),
    amount(asset("future-capacity"), 800),
    amount(asset("future-obligation/generator"), 7),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("obligation/generator"), 1),
    amount(asset("production-allowance"), 320),
    amount(asset("production-capacity"), 800),
    amount(asset("reference-price"), 10000),
    amount(asset("solar-potential"), 24000),
    amount(asset("storage-capacity"), 3500),
    amount(asset("trading"), 1)
])));
accounts.push(opening(account("actor/household"), basket([
    amount(asset("credit"), 20000),
    amount(asset("decisions"), 6),
    amount(asset("future-income"), 7),
    amount(asset("future-need/household"), 7),
    amount(asset("income-due"), 1),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("need/household"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 1200),
    amount(asset("trading"), 1)
])));
accounts.push(opening(account("actor/speculator"), basket([
    amount(asset("credit"), 25000),
    amount(asset("decisions"), 6),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 3000),
    amount(asset("trading"), 1),
    amount(asset("uninformed/speculator"), 1)
])));
accounts.push(opening(account("actor/whale"), basket([
    amount(asset("credit"), 80000),
    amount(asset("decisions"), 6),
    amount(asset("liquidity-decision"), 1),
    amount(asset("market-open"), 1),
    amount(asset("reference-price"), 10000),
    amount(asset("storage-capacity"), 8000),
    amount(asset("trading"), 1)
])));
accounts.push(opening(account("information"), basket([amount(asset("shortage-signal"), 1)])));
accounts.push(opening(account("pool"), basket([
    amount(asset("credit"), 100000),
    amount(asset("energy"), 10000),
    amount(asset("market-open"), 1),
    amount(asset("unissued-lp-share"), 990000)
])));
accounts.push(opening(account("treasury"), basket([amount(asset("credit"), 100000)])));
accounts.push(opening(account("world"), basket([
    amount(asset("at-day/0"), 1),
    amount(asset("current-sunlight"), 40),
    amount(asset("market-active"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("closed-energy-accounting");
    for name in ["energy", "solar-potential", "spent-energy"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("closed-credit-accounting");
    rule.weight(asset("credit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("liquidity-share-accounting");
    rule.weight(asset("lp-share"), 1);
    rule.weight(asset("unissued-lp-share"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("actor-goal-lifecycle");
    for name in [
        "need/factory",
        "satisfied-need/factory",
        "need/household",
        "satisfied-need/household",
        "obligation/generator",
        "settled-obligation/generator",
        "obligation/factory",
        "settled-obligation/factory",
        "future-need/factory",
        "missed-need/factory",
        "future-need/household",
        "missed-need/household",
        "future-obligation/factory",
        "defaulted-obligation/factory",
        "future-obligation/generator",
        "defaulted-obligation/generator"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("shortage-knowledge-lifecycle");
    rule.weight(asset("uninformed/speculator"), 1);
    rule.weight(asset("informed/speculator"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("market-lifecycle");
    rule.weight(asset("market-active"), 1);
    rule.weight(asset("market-ended"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("production-capacity-lifecycle");
    for name in ["production-capacity", "future-capacity", "pending-capacity"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("manufactured-goods-lifecycle");
    for name in ["unmade-goods", "goods", "delivered-goods"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("income-entitlement-lifecycle");
    for name in ["future-income", "income-due", "received-income", "missed-income"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
