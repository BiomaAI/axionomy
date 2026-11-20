let model = #{
    ontology: "exact-cover",
    laws: [
        "exact-cover/common",
        "exact-cover/micro-and-micro-unsatisfiable",
        "exact-cover/micro-and-micro-unsatisfiable-and-showcase-unsatisfiable-and-stress-unsatisfiable"
    ]
};
let accounts = [];
accounts.push(opening(account("problem"), basket([
    amount(asset("active"), 1),
    amount(asset("available/ab"), 1),
    amount(asset("available/ac"), 1),
    amount(asset("progress/0"), 1),
    amount(asset("uncovered/a"), 1),
    amount(asset("uncovered/b"), 1),
    amount(asset("uncovered/c"), 1),
    amount(asset("uncovered/d"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("each-universe-element-persists");
    for name in [
        "uncovered/a",
        "covered/a",
        "uncovered/b",
        "covered/b",
        "uncovered/c",
        "covered/c",
        "uncovered/d",
        "covered/d"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("set-choice-is-single-use");
    for name in ["available/ab", "selected/ab", "available/ac", "selected/ac"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-progress-token");
    for name in ["progress/0", "progress/1", "progress/2", "progress/3", "progress/4"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("exact-cover-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
