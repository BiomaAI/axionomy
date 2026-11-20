let model = #{
    ontology: "exact-cover",
    laws: [
        "exact-cover/common",
        "exact-cover/micro-and-showcase-and-stress",
        "exact-cover/showcase-and-showcase-unsatisfiable-and-stress-and-stress-unsatisfiable",
        "exact-cover/showcase-and-stress",
        "exact-cover/stress",
        "exact-cover/stress-and-stress-unsatisfiable"
    ]
};
let accounts = [];
accounts.push(opening(account("problem"), basket([
    amount(asset("active"), 1),
    amount(asset("available/ab"), 1),
    amount(asset("available/abef"), 1),
    amount(asset("available/abij"), 1),
    amount(asset("available/ace"), 1),
    amount(asset("available/adg"), 1),
    amount(asset("available/ae"), 1),
    amount(asset("available/aei"), 1),
    amount(asset("available/ai"), 1),
    amount(asset("available/al"), 1),
    amount(asset("available/beh"), 1),
    amount(asset("available/bfg"), 1),
    amount(asset("available/bfj"), 1),
    amount(asset("available/cd"), 1),
    amount(asset("available/cdgh"), 1),
    amount(asset("available/cdk"), 1),
    amount(asset("available/cf"), 1),
    amount(asset("available/cgk"), 1),
    amount(asset("available/dh"), 1),
    amount(asset("available/dhl"), 1),
    amount(asset("available/ef"), 1),
    amount(asset("available/efl"), 1),
    amount(asset("available/fgh"), 1),
    amount(asset("available/gh"), 1),
    amount(asset("available/ij"), 1),
    amount(asset("available/kl"), 1),
    amount(asset("progress/0"), 1),
    amount(asset("uncovered/a"), 1),
    amount(asset("uncovered/b"), 1),
    amount(asset("uncovered/c"), 1),
    amount(asset("uncovered/d"), 1),
    amount(asset("uncovered/e"), 1),
    amount(asset("uncovered/f"), 1),
    amount(asset("uncovered/g"), 1),
    amount(asset("uncovered/h"), 1),
    amount(asset("uncovered/i"), 1),
    amount(asset("uncovered/j"), 1),
    amount(asset("uncovered/k"), 1),
    amount(asset("uncovered/l"), 1)
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
        "covered/d",
        "uncovered/e",
        "covered/e",
        "uncovered/f",
        "covered/f",
        "uncovered/g",
        "covered/g",
        "uncovered/h",
        "covered/h",
        "uncovered/i",
        "covered/i",
        "uncovered/j",
        "covered/j",
        "uncovered/k",
        "covered/k",
        "uncovered/l",
        "covered/l"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("set-choice-is-single-use");
    for name in [
        "available/ab",
        "selected/ab",
        "available/cd",
        "selected/cd",
        "available/ef",
        "selected/ef",
        "available/gh",
        "selected/gh",
        "available/ij",
        "selected/ij",
        "available/kl",
        "selected/kl",
        "available/ace",
        "selected/ace",
        "available/bfg",
        "selected/bfg",
        "available/dh",
        "selected/dh",
        "available/adg",
        "selected/adg",
        "available/beh",
        "selected/beh",
        "available/cf",
        "selected/cf",
        "available/abef",
        "selected/abef",
        "available/cdgh",
        "selected/cdgh",
        "available/ae",
        "selected/ae",
        "available/fgh",
        "selected/fgh",
        "available/aei",
        "selected/aei",
        "available/bfj",
        "selected/bfj",
        "available/cgk",
        "selected/cgk",
        "available/dhl",
        "selected/dhl",
        "available/abij",
        "selected/abij",
        "available/cdk",
        "selected/cdk",
        "available/efl",
        "selected/efl",
        "available/ai",
        "selected/ai",
        "available/al",
        "selected/al"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-progress-token");
    for name in [
        "progress/0",
        "progress/1",
        "progress/2",
        "progress/3",
        "progress/4",
        "progress/5",
        "progress/6",
        "progress/7",
        "progress/8",
        "progress/9",
        "progress/10",
        "progress/11",
        "progress/12"
    ] {
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
