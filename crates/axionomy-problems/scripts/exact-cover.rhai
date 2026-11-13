// Exact cover: which subsets to select so that every element is covered
// exactly once. Breadth-first search tries every rate that applies; Algorithm
// X plans a whole cover, which the kernel still validates step by step.

// Every rate bound to the problem, in order, that applies.
fn candidates(state) {
    let rates = state.rates("");
    rates
        .map(|rate| exchange(rate, #{ problem: "problem" }, 1))
        .filter(|proposal| state.applicable(proposal))
}

// The elements of the universe, covered or not, in order.
fn universe(state) {
    let elements = [];
    for asset in state.balances("problem").keys() {
        let parts = asset.split("/");
        if (parts[0] == "uncovered" || parts[0] == "covered") && !elements.contains(parts[1]) {
            elements.push(parts[1]);
        }
    }
    elements.sort();
    elements
}

// The subsets, each with its elements, read back from what selecting it
// consumes.
fn memberships(state) {
    let sets = #{};
    for rate in state.rates("select/") {
        let set = rate.split("/")[1];
        if set in sets { continue; }
        let members = [];
        for asset in state.consumed(`select/${set}/0`, "problem").keys() {
            if asset.starts_with("uncovered/") { members.push(asset.sub_string(10)); }
        }
        sets[set] = members;
    }
    sets
}

// The subsets that cover `remaining`, each disjoint from the others, chosen
// by Algorithm X: branch on the element the fewest subsets cover, trying its
// subsets in order. Nothing when there is no cover.
fn cover(sets, remaining, available) {
    if remaining.is_empty() { return []; }
    let pivot = ();
    let fewest = ();
    for element in remaining {
        let count = available.filter(|set| sets[set].contains(element)).len();
        if fewest == () || count < fewest {
            fewest = count;
            pivot = element;
        }
    }
    for set in available {
        let members = sets[set];
        if !members.contains(pivot) || !members.all(|element| remaining.contains(element)) {
            continue;
        }
        let next_remaining = remaining.filter(|element| !members.contains(element));
        let next_available = available.filter(|candidate| {
            !sets[candidate].some(|element| members.contains(element))
        });
        let suffix = cover(sets, next_remaining, next_available);
        if suffix != () {
            return [set] + suffix;
        }
    }
    ()
}

// Algorithm X's plan: the subsets it selects, each at the progress made
// before it, then finishing; nothing when no cover exists.
fn algorithm_x(state) {
    let held = state.balances("problem");
    if "solved" in held { return []; }
    let sets = memberships(state);
    let elements = universe(state);
    let remaining = elements.filter(|element| `uncovered/${element}` in held);
    let names = sets.keys();
    names.sort();
    let available = names.filter(|set| `available/${set}` in held);
    let selected = cover(sets, remaining, available);
    if selected == () { return (); }
    let before = elements.len() - remaining.len();
    let plan = [];
    for set in selected {
        plan.push(exchange(`select/${set}/${before}`, #{ problem: "problem" }, 1));
        before += sets[set].len();
    }
    plan.push(exchange("finish", #{ problem: "problem" }, 1));
    plan
}
