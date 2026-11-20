let model = #{ontology: "workshop", laws: ["workshop/micro"]};
let accounts = [];
accounts.push(opening(account("workshop"), basket([
    amount(asset("active"), 1),
    amount(asset("labor"), 4),
    amount(asset("time"), 3),
    amount(asset("tool"), 1),
    amount(asset("wood"), 6)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("material-mass");
    for name in ["wood", "chair", "waste"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("labor-accounting");
    rule.weight(asset("labor"), 1);
    rule.weight(asset("spent-labor"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("time-accounting");
    rule.weight(asset("time"), 1);
    rule.weight(asset("spent-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("workshop-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
