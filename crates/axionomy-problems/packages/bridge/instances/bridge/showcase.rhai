let model = #{ontology: "bridge", laws: ["bridge/showcase", "bridge/showcase-and-stress"], constants: #{"detour-time": "3"}};
let accounts = [];
accounts.push(opening(account("agent/a"), basket([
    amount(asset("at/west"), 1),
    amount(asset("can-bid"), 1),
    amount(asset("credit"), 4),
    amount(asset("deadline"), 7),
    amount(asset("energy"), 1)
])));
accounts.push(opening(account("agent/b"), basket([
    amount(asset("arrival"), 1),
    amount(asset("at/west"), 1),
    amount(asset("can-bid"), 1),
    amount(asset("credit"), 4),
    amount(asset("deadline"), 4),
    amount(asset("energy"), 1)
])));
accounts.push(opening(account("bridge"), basket([
    amount(asset("active"), 1),
    amount(asset("capacity-free"), 1),
    amount(asset("first-turn"), 1),
    amount(asset("rounds-remaining"), 1),
    amount(asset("time-remaining"), 64)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("two-agent-positions");
    for name in ["at/west", "at/east", "in-transit", "on-detour"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("two-agent-status-tokens");
    for name in ["can-bid", "submitted", "crossing-right", "waiting", "crossed", "detouring"] {
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
{
    let rule = invariant("bounded-shared-clock");
    rule.weight(asset("now"), 1);
    rule.weight(asset("time-remaining"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
