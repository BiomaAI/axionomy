let model = #{
    ontology: "amm",
    laws: ["amm/micro"],
    constants: #{"fee-denominator": "1000", "fee-numerator": "997", scale: "1", "share-limit": "1000000"}
};
let accounts = [];
accounts.push(opening(account("actor/adaptive-lp"), basket([amount(asset("credit"), 20000), amount(asset("energy"), 2000)])));
accounts.push(opening(account("actor/arbitrageur"), basket([amount(asset("credit"), 10000), amount(asset("energy"), 2500)])));
accounts.push(opening(account("actor/factory"), basket([
    amount(asset("credit"), 40000),
    amount(asset("need/factory"), 1),
    amount(asset("obligation/factory"), 1)
])));
accounts.push(opening(account("actor/founder"), basket([amount(asset("credit"), 5000), amount(asset("lp-share"), 4000)])));
accounts.push(opening(account("actor/generator"), basket([
    amount(asset("credit"), 5000),
    amount(asset("energy"), 3000),
    amount(asset("obligation/generator"), 1),
    amount(asset("solar-potential"), 6000)
])));
accounts.push(opening(account("actor/household"), basket([amount(asset("credit"), 20000), amount(asset("need/household"), 1)])));
accounts.push(opening(account("actor/speculator"), basket([amount(asset("credit"), 25000), amount(asset("uninformed/speculator"), 1)])));
accounts.push(opening(account("actor/whale"), basket([amount(asset("credit"), 80000)])));
accounts.push(opening(account("information"), basket([amount(asset("shortage-signal"), 1)])));
accounts.push(opening(account("pool"), basket([
    amount(asset("credit"), 40000),
    amount(asset("energy"), 4000),
    amount(asset("unissued-lp-share"), 996000)
])));
accounts.push(opening(account("treasury"), basket([amount(asset("credit"), 20000)])));
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
        "settled-obligation/factory"
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
model.invariants = rules;
model
