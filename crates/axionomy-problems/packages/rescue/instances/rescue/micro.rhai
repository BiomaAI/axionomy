let model = #{ontology: "rescue", laws: ["rescue/common"]};
let accounts = [];
accounts.push(opening(account("agent"), basket([
    amount(asset("at/base"), 1),
    amount(asset("energy"), 2),
    amount(asset("planning"), 1),
    amount(asset("sensor"), 1)
])));
accounts.push(opening(account("nature"), basket([amount(asset("unresolved"), 1)])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("one-agent-position");
    for name in ["at/base", "at/north", "at/south"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("energy-accounting");
    rule.weight(asset("energy"), 1);
    rule.weight(asset("spent-energy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("sensor-accounting");
    rule.weight(asset("sensor"), 1);
    rule.weight(asset("used-sensor"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-nature-seed-state");
    for name in ["unresolved", "seed/0", "seed/1", "seed/2", "seed/3"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-hidden-truth-state");
    for name in ["unresolved", "truth/north", "truth/south"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("rescue-lifecycle");
    for name in ["planning", "awaiting-observation", "committed", "rescued", "evacuated", "solved"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
