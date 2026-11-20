let model = #{ontology: "mission", laws: ["mission/common"]};
let accounts = [];
accounts.push(opening(account("agent/medic"), basket([
    amount(asset("at/base"), 1),
    amount(asset("energy"), 2),
    amount(asset("medical-kit"), 1)
])));
accounts.push(opening(account("agent/scout"), basket([
    amount(asset("at/base"), 1),
    amount(asset("energy"), 2),
    amount(asset("sensor"), 1)
])));
accounts.push(opening(account("mission"), basket([amount(asset("planning"), 1), amount(asset("time-remaining"), 10)])));
accounts.push(opening(account("nature"), basket([amount(asset("unresolved"), 1)])));
accounts.push(opening(account("success"), basket([])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("two-agent-positions");
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
    let rule = invariant("time-accounting");
    rule.weight(asset("time-remaining"), 1);
    rule.weight(asset("elapsed-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("sensor-accounting");
    rule.weight(asset("sensor"), 1);
    rule.weight(asset("used-sensor"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("medical-kit-accounting");
    rule.weight(asset("medical-kit"), 1);
    rule.weight(asset("used-medical-kit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("mission-lifecycle");
    for name in ["planning", "awaiting-scan", "awaiting-encounter", "needs-treatment", "area-safe", "rescued", "solved"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-hidden-truth");
    for name in ["unresolved", "truth/north", "truth/south"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-hidden-seed");
    for name in ["unresolved", "seed/0", "seed/1", "seed/2", "seed/3"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-hidden-hazard");
    for name in ["unresolved", "hazard-resolved", "hazard/safe", "hazard/injury"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
