let model = #{ontology: "perishables", laws: ["perishables/common"]};
let accounts = [];
accounts.push(opening(account("cohort/ambient"), basket([
    amount(asset("cohort-identity/ambient"), 1),
    amount(asset("fresh/ambient/ambient/ambient-expiry"), 1)
])));
accounts.push(opening(account("cohort/refrigerated"), basket([
    amount(asset("cohort-identity/refrigerated"), 1),
    amount(asset("fresh/refrigerated/cold/cold-expiry"), 1)
])));
accounts.push(opening(account("storage/fridge"), basket([
    amount(asset("claim/refrigerated"), 30),
    amount(asset("cold"), 1),
    amount(asset("cooling-energy"), 70),
    amount(asset("location-identity/fridge"), 1),
    amount(asset("powered"), 1)
])));
accounts.push(opening(account("storage/warehouse"), basket([
    amount(asset("ambient"), 1),
    amount(asset("claim/ambient"), 70),
    amount(asset("location-identity/warehouse"), 1)
])));
accounts.push(opening(account("world"), basket([
    amount(asset("active"), 1),
    amount(asset("before/ambient-expiry"), 1),
    amount(asset("before/cold-expiry"), 1),
    amount(asset("before/warmed-expiry"), 1),
    amount(asset("now/harvest"), 1),
    amount(asset("planning"), 1),
    amount(asset("reached/harvest"), 1),
    amount(asset("world-identity"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("fruit-claims");
    for name in ["claim/ambient", "consumed/ambient", "claim/refrigerated", "consumed/refrigerated"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("cohort-condition/ambient");
    rule.weight(asset("rotten/ambient"), 1);
    rule.weight(asset("fresh/ambient/ambient/ambient-expiry"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("cohort-condition/refrigerated");
    for name in [
        "rotten/refrigerated",
        "fresh/refrigerated/warmed-after-outage/warmed-expiry",
        "fresh/refrigerated/cold/cold-expiry"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("storage-environment");
    rule.weight(asset("ambient"), 1);
    rule.weight(asset("cold"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("fridge-power");
    rule.weight(asset("powered"), 1);
    rule.weight(asset("unpowered"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("cooling-energy");
    rule.weight(asset("cooling-energy"), 1);
    rule.weight(asset("spent-cooling-energy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-current-time");
    for name in ["now/harvest", "now/ambient-expiry", "now/warmed-expiry", "now/cold-expiry"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("deadline-phase/ambient-expiry");
    rule.weight(asset("before/ambient-expiry"), 1);
    rule.weight(asset("reached/ambient-expiry"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("deadline-phase/warmed-expiry");
    rule.weight(asset("before/warmed-expiry"), 1);
    rule.weight(asset("reached/warmed-expiry"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("deadline-phase/cold-expiry");
    rule.weight(asset("before/cold-expiry"), 1);
    rule.weight(asset("reached/cold-expiry"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("scenario-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("storage-planning-lifecycle");
    rule.weight(asset("planning"), 1);
    rule.weight(asset("plan-solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
