let model = #{ontology: "pantry", laws: ["delivery"]};
let accounts = [];
accounts.push(opening(account("stockroom"), basket([amount(asset("meal"), 3)])));
accounts.push(opening(account("counter"), basket([])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("meals");
    rule.weight(asset("meal"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
