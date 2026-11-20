let model = #{ontology: "sokoban", laws: ["sokoban/micro"], constants: #{"board-height": "3", "board-width": "7"}};
let accounts = [];
accounts.push(opening(account("cell/0"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/1"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/2"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/3"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/4"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/5"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/6"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/7"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/8"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("floor"), 1),
    amount(asset("player"), 1)
])));
accounts.push(opening(account("cell/9"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/10"), basket([amount(asset("crate/0"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/11"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/12"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/13"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/14"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/15"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/16"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/17"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/18"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/19"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/20"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("success"), basket([amount(asset("active"), 1)])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("one-crate/0");
    rule.weight(asset("crate/0"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-player");
    rule.weight(asset("player"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-occupant-per-floor");
    for name in ["player", "empty", "crate/0"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("puzzle-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
