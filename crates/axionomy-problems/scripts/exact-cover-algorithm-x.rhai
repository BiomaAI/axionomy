// Exact cover's Algorithm X, which the searches plan with and the view asks
// whether a cover can still be completed: the problem's elements and
// subsets, read back from the economy, and the cover of what remains.

// The elements of the universe, covered or not, in order, from what the
// problem `held`.
fn universe(held) {
    let elements = [];
    for asset in held.keys() {
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
