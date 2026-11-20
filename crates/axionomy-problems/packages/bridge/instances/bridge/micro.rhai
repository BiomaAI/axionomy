let model = #{ontology: "bridge", laws: ["bridge/micro"]};
let accounts = [];
accounts.push(opening(account("agent/a"), basket([
    amount(asset("at/west"), 1),
    amount(asset("can-bid"), 1),
    amount(asset("credit"), 2),
    amount(asset("energy"), 1)
])));
accounts.push(opening(account("agent/b"), basket([
    amount(asset("at/west"), 1),
    amount(asset("can-bid"), 1),
    amount(asset("credit"), 2),
    amount(asset("energy"), 1)
])));
accounts.push(opening(account("bridge"), basket([
    amount(asset("active"), 1),
    amount(asset("capacity-free"), 1),
    amount(asset("first-turn"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("two-agent-positions");
    rule.weight(asset("at/west"), 1);
    rule.weight(asset("at/east"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("two-agent-status-tokens");
    for name in ["can-bid", "submitted", "crossing-right", "waiting", "crossed"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("bridge-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("bridge-capacity");
    rule.weight(asset("capacity-free"), 1);
    rule.weight(asset("crossing-right"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("crossing-turn");
    for name in ["first-turn", "second-turn", "crossing-right"] {
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
    let rule = invariant("credit-accounting");
    for name in ["credit", "escrow", "spent-credit"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
