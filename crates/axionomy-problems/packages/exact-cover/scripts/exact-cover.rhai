// Exact cover: which subsets to select so that every element is covered
// exactly once. Algorithm X plans a whole cover, which the kernel still
// validates step by step.

import "exact-cover/algorithm-x" as algorithm;

// Algorithm X's plan: the subsets it selects, each at the progress made
// before it, then finishing; nothing when no cover exists.
fn algorithm_x(state) {
    let held = state.balances("problem");
    if "solved" in held { return []; }
    let sets = algorithm::memberships(state);
    let elements = algorithm::universe(held);
    let remaining = elements.filter(|element| `uncovered/${element}` in held);
    let names = sets.keys();
    names.sort();
    let available = names.filter(|set| `available/${set}` in held);
    let selected = algorithm::cover(sets, remaining, available);
    if selected == () { return (); }
    let before = elements.len() - remaining.len();
    let plan = [];
    for set in selected {
        plan.push(state.exchange_of(`select/${set}/${before}`));
        before += sets[set].len();
    }
    plan.push(state.exchange_of("finish"));
    plan
}
