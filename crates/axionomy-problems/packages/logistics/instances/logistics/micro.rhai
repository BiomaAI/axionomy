let model = #{ontology: "logistics", laws: ["logistics/common"], constants: #{"refuel-amount": "4"}};
let accounts = [];
accounts.push(opening(account("fuel-station"), basket([amount(asset("fuel"), 12)])));
accounts.push(opening(account("nature"), basket([amount(asset("weather-ready"), 1)])));
accounts.push(opening(account("order/a"), basket([amount(asset("package/a"), 1), amount(asset("waiting"), 1)])));
accounts.push(opening(account("order/b"), basket([amount(asset("delivered"), 1), amount(asset("package/b"), 1)])));
accounts.push(opening(account("order/c"), basket([amount(asset("delivered"), 1), amount(asset("package/c"), 1)])));
accounts.push(opening(account("order/d"), basket([amount(asset("delivered"), 1), amount(asset("package/d"), 1)])));
accounts.push(opening(account("vehicle"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("cargo-space"), 1),
    amount(asset("fuel"), 4),
    amount(asset("money"), 12),
    amount(asset("repair-tool"), 1),
    amount(asset("time-remaining"), 80)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("vehicle-position");
    for name in [
        "traveling/direct-out",
        "traveling/direct-back",
        "traveling/safe-out-first",
        "traveling/safe-out-second",
        "traveling/safe-back-first",
        "traveling/safe-back-second",
        "at/depot",
        "at/junction",
        "at/customer"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("fuel-accounting");
    rule.weight(asset("fuel"), 1);
    rule.weight(asset("spent-fuel"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("money-accounting");
    rule.weight(asset("money"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("time-accounting");
    rule.weight(asset("time-remaining"), 1);
    rule.weight(asset("elapsed-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("cargo-capacity");
    rule.weight(asset("cargo-space"), 1);
    rule.weight(asset("cargo-occupied"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("order-lifecycle");
    for name in ["waiting", "in-transit", "delivered"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
