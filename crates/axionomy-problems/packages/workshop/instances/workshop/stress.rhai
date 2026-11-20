let model = #{ontology: "workshop", laws: ["workshop/showcase-and-stress", "workshop/stress"]};
let accounts = [];
accounts.push(opening(account("customer/0"), basket([amount(asset("credit"), 8)])));
accounts.push(opening(account("customer/1"), basket([amount(asset("credit"), 21)])));
accounts.push(opening(account("customer/2"), basket([amount(asset("credit"), 14)])));
accounts.push(opening(account("customer/3"), basket([amount(asset("credit"), 27)])));
accounts.push(opening(account("customer/4"), basket([amount(asset("credit"), 20)])));
accounts.push(opening(account("workshop"), basket([
    amount(asset("active"), 1),
    amount(asset("contract/0"), 1),
    amount(asset("contract/1"), 1),
    amount(asset("contract/2"), 1),
    amount(asset("contract/3"), 1),
    amount(asset("contract/4"), 1),
    amount(asset("durability"), 3),
    amount(asset("idle"), 1),
    amount(asset("labor"), 14),
    amount(asset("repair-kit"), 2),
    amount(asset("time"), 22),
    amount(asset("tool"), 1),
    amount(asset("tool-potential"), 2),
    amount(asset("upgrade-kit"), 1),
    amount(asset("wood"), 16)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("contract-lifecycle/0");
    for name in ["contract/0", "raw/0", "prepared/0", "assembled/0", "fulfilled/0"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("contract-lifecycle/1");
    for name in ["contract/1", "raw/1", "prepared/1", "assembled/1", "fulfilled/1"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("contract-lifecycle/2");
    for name in ["contract/2", "raw/2", "prepared/2", "assembled/2", "fulfilled/2"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("contract-lifecycle/3");
    for name in ["contract/3", "raw/3", "prepared/3", "assembled/3", "fulfilled/3"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("contract-lifecycle/4");
    for name in ["contract/4", "raw/4", "prepared/4", "assembled/4", "fulfilled/4"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-workbench");
    for name in [
        "idle",
        "solved",
        "raw/0",
        "prepared/0",
        "assembled/0",
        "raw/1",
        "prepared/1",
        "assembled/1",
        "raw/2",
        "prepared/2",
        "assembled/2",
        "raw/3",
        "prepared/3",
        "assembled/3",
        "raw/4",
        "prepared/4",
        "assembled/4"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("material-mass");
    for name in ["wood", "frame", "panel"] {
        rule.weight(asset(name), 1);
    }
    rule.weight(asset("chair"), 3);
    rule.weight(asset("table"), 5);
    rule.weight(asset("offcut"), 1);
    rule.weight(asset("waste"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("tool-capacity");
    for name in ["durability", "wear", "tool-potential"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("repair-supplies");
    rule.weight(asset("repair-kit"), 1);
    rule.weight(asset("used-repair-kit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("upgrade-opportunity");
    rule.weight(asset("upgrade-kit"), 1);
    rule.weight(asset("upgraded"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("closed-customer-funds");
    rule.weight(asset("credit"), 1);
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
