let model = #{
    ontology: "perishables/cold-chain",
    laws: ["perishables/cold-chain/common"],
    tables: #{
        "open-day/{day}": #{
            variables: [#{name: "day", range: ["1", "7"]}],
            relations: [
                #{
                    gives: ["grid-supply"],
                    rows: [["1", "3"], ["2", "3"], ["3", "0"], ["4", "0"], ["5", "0"], ["6", "3"], ["7", "3"]],
                    variables: ["day"]
                }
            ]
        }
    },
    constants: #{battery: "6", days: "8"}
};
let accounts = [];
accounts.push(opening(account("batch/0"), basket([
    amount(asset("active"), 1),
    amount(asset("at/warehouse"), 1),
    amount(asset("fresh"), 9),
    amount(asset("origin/0"), 1)
])));
accounts.push(opening(account("batch/1"), basket([
    amount(asset("active"), 1),
    amount(asset("at/fridge"), 1),
    amount(asset("cold-occupancy"), 3),
    amount(asset("fresh"), 3),
    amount(asset("origin/1"), 1)
])));
accounts.push(opening(account("batch/2"), basket([amount(asset("dormant"), 1)])));
accounts.push(opening(account("batch/3"), basket([amount(asset("dormant"), 1)])));
accounts.push(opening(account("batch/4"), basket([amount(asset("dormant"), 1)])));
accounts.push(opening(account("batch/5"), basket([amount(asset("dormant"), 1)])));
accounts.push(opening(account("buyer"), basket([amount(asset("money"), 48)])));
accounts.push(opening(account("world"), basket([
    amount(asset("cooling-off"), 1),
    amount(asset("decisions"), 1),
    amount(asset("demand"), 2),
    amount(asset("energy"), 3),
    amount(asset("freight"), 2),
    amount(asset("fuel"), 5),
    amount(asset("future-demand"), 14),
    amount(asset("future-freight"), 14),
    amount(asset("future-labor"), 14),
    amount(asset("grid-potential"), 12),
    amount(asset("labor"), 2)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("food-mass");
    for name in ["fresh", "preserved", "spoiled", "disposed", "delivered"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("batch-activation");
    rule.weight(asset("active"), 1);
    rule.weight(asset("dormant"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("physical-cold-storage");
    rule.weight(asset("cold-space"), 1);
    rule.weight(asset("cold-occupancy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("closed-electricity-and-fuel-potential");
    for name in ["energy", "spent-energy", "grid-potential", "spilled-energy"] {
        rule.weight(asset(name), 1);
    }
    rule.weight(asset("fuel"), 3);
    rule.weight(asset("transport-fuel"), 3);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-fuel");
    for name in ["fuel", "generator-fuel", "transport-fuel"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("cooling-switch");
    rule.weight(asset("cooling-on"), 1);
    rule.weight(asset("cooling-off"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-labor");
    for name in ["labor", "spent-labor", "future-labor", "unused-labor"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("daily-transport-capacity");
    for name in ["freight", "used-freight", "future-freight", "unused-freight"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("daily-food-obligations");
    for name in ["demand", "future-demand", "satisfied-demand", "missed-demand"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("funded-food-purchases");
    rule.weight(asset("money"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("cold-chain-lifecycle");
    for name in ["decisions", "aging", "solved"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
