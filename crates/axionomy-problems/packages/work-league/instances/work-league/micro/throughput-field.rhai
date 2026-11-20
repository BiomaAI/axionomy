let model = #{ontology: "work-league", laws: ["work-league/micro"]};
let accounts = [];
accounts.push(opening(account("agent/atlas"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("energy"), 96),
    amount(asset("material"), 64),
    amount(asset("operational"), 1),
    amount(asset("policy/sprinter"), 1),
    amount(asset("time-remaining"), 220)
])));
accounts.push(opening(account("agent/bolt"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("energy"), 96),
    amount(asset("material"), 64),
    amount(asset("operational"), 1),
    amount(asset("policy/sprinter"), 1),
    amount(asset("time-remaining"), 220)
])));
accounts.push(opening(account("facility/charger"), basket([amount(asset("charge-supply"), 80)])));
accounts.push(opening(account("facility/recycler"), basket([])));
accounts.push(opening(account("facility/workshop"), basket([amount(asset("repair-supply"), 8)])));
accounts.push(opening(account("job/1"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("job/2"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("job/3"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("job/4"), basket([amount(asset("available"), 1)])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("league-energy-accounting");
    for name in ["energy", "spent-energy", "charge-supply"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("league-material-accounting");
    rule.weight(asset("material"), 1);
    rule.weight(asset("material-spent"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("league-time-accounting");
    rule.weight(asset("time-remaining"), 1);
    rule.weight(asset("elapsed-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("worker-operational-condition");
    rule.weight(asset("operational"), 1);
    rule.weight(asset("damage"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("repair-supply-accounting");
    rule.weight(asset("repair-supply"), 1);
    rule.weight(asset("spent-repair-supply"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
