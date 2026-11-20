let model = #{ontology: "maze", laws: ["maze/micro"]};
let accounts = [];
accounts.push(opening(account("agent"), basket([
    amount(asset("active"), 1),
    amount(asset("at/start"), 1),
    amount(asset("energy"), 9),
    amount(asset("time"), 6)
])));
accounts.push(opening(account("world"), basket([
    amount(asset("distance/detour"), 5),
    amount(asset("distance/gate"), 2),
    amount(asset("distance/key-room"), 4),
    amount(asset("distance/start"), 6),
    amount(asset("edge/detour/exit"), 1),
    amount(asset("edge/gate/exit"), 1),
    amount(asset("edge/key-room/gate"), 1),
    amount(asset("edge/start/detour"), 1),
    amount(asset("edge/start/key-room"), 1),
    amount(asset("key"), 1),
    amount(asset("locked"), 1),
    amount(asset("target/exit"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("one-agent-position");
    for name in ["at/detour", "at/exit", "at/gate", "at/key-room", "at/start"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("energy-is-only-spent");
    rule.weight(asset("energy"), 1);
    rule.weight(asset("spent-energy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("time-is-only-spent");
    rule.weight(asset("time"), 1);
    rule.weight(asset("spent-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("maze-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
