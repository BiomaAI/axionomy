// Mission's Micro: a Scout and a Medic at base, and a victim north or
// south. Nature draws the hidden truth, a sensor seed and the site's hazard
// from its weighted prior. A scan reports the truth to the Scout, except
// that seed 0 reports the other site; sharing a report gives it to the
// Medic, and the team moves together only on shared intelligence, or both
// go somewhere directly before any scan. At the true site Nature resolves
// the hazard, safe or an injury the Medic must treat, and the team rescues
// the victim and finishes. The Scout sees only its own account, the
// mission's and the goal's: never Nature's, nor the Medic's.
//
// The planner is the Scout choosing every public action by information-set
// tree search over what it sees, each iteration in a world drawn from those
// its observations leave possible; the fixed policies play turn by turn.

fn sites() { ["north", "south"] }

// The exchange of `rate`, with the accounts its roles bind.
fn action(rate) {
    let kind = rate.split("/")[0];
    let bindings = if kind == "instantiate" {
        #{ nature: "nature" }
    } else if kind == "begin-scan" {
        #{ scout: "agent/scout", mission: "mission" }
    } else if kind == "resolve-scan" {
        #{ scout: "agent/scout", nature: "nature", mission: "mission" }
    } else if kind == "encounter" || kind == "rescue" {
        #{ scout: "agent/scout", medic: "agent/medic", nature: "nature", mission: "mission" }
    } else if kind == "finish" {
        #{ mission: "mission", goal: "success" }
    } else {
        #{ scout: "agent/scout", medic: "agent/medic", mission: "mission" }
    };
    exchange(rate, bindings, 1)
}

// Nature's prior: every scenario it encodes that applies, in rate order,
// weighted by what Nature holds of it.
fn scenarios(state) {
    let weighted = [];
    for rate in state.rates("instantiate/") {
        let parts = rate.split("/");
        let scenario = action(rate);
        if state.applicable(scenario) {
            let weight = state.balance("nature", `scenario-weight/${parts[1]}/${parts[2]}/${parts[3]}`);
            weighted.push([scenario, weight]);
        }
    }
    weighted
}

// What Nature answers once the scenario is drawn, in rate order: the
// report a scan awaits, which the truth and the sensor's seed fix, or the
// hazard at the site the team reached, if the victim is there. At most one
// applies.
fn responses(state) {
    let found = [];
    for prefix in ["encounter/", "resolve-scan/"] {
        for rate in state.rates(prefix) {
            let response = action(rate);
            if state.applicable(response) { found.push(response); }
        }
    }
    found
}

// Nature's draws, as `[exchange, weight]` pairs: the prior while the
// scenario is unresolved, and otherwise its response, of weight one. The
// planner's tree draws them, and its beliefs draw the history's again.
fn draws(state) {
    if state.balance("nature", "unresolved") > 0 { return scenarios(state); }
    responses(state).map(|response| [response, 1])
}

// Nature's turn: the scenario while unresolved, drawn from the prior by the
// event `scenario`, which a run meets once, as it begins; and otherwise the
// response the team awaits, if any.
fn nature(state, random) {
    if state.balance("nature", "unresolved") > 0 {
        let weighted = scenarios(state);
        if weighted.is_empty() { return (); }
        return weighted[random.sample("scenario", weighted.map(|outcome| outcome[1]))][0];
    }
    let found = responses(state);
    if found.is_empty() { () } else { found[0] }
}

// The Scout plans every public action.
fn planner(state) { 0 }

// What the Scout sees: its own account, the mission's and the goal's.
fn scout_view(state, player) { state.view(["agent/scout", "mission", "success"]) }

fn holds(seen, account, asset) { seen.balance(account, asset) > 0 }

// The public actions the Scout's view offers, in the order the Rust
// decision source emits them: the treatment an injury needs; the rescue
// where the team stands once the area is safe; the finish once rescued;
// and while planning, a scan with the sensor unused, then for each site
// sharing its report, moving together on shared intelligence, and going
// there directly with the sensor unused. Whether they apply is the
// search's to find in each world.
fn decisions(seen) {
    if holds(seen, "mission", "needs-treatment") { return [action("treat")]; }
    if holds(seen, "mission", "area-safe") {
        let found = [];
        for site in sites() {
            if holds(seen, "agent/scout", `at/${site}`) { found.push(action(`rescue/${site}`)); }
        }
        return found;
    }
    if holds(seen, "mission", "rescued") { return [action("finish")]; }
    if !holds(seen, "mission", "planning") { return []; }
    let found = [];
    let sensing = holds(seen, "agent/scout", "sensor");
    if sensing { found.push(action("begin-scan")); }
    for site in sites() {
        if holds(seen, "agent/scout", `intel/${site}`) { found.push(action(`share/${site}`)); }
        if holds(seen, "agent/scout", `shared-intel/${site}`) {
            found.push(action(`move-together/${site}`));
        }
        if sensing { found.push(action(`move-direct/${site}`)); }
    }
    found
}

// The first of `candidates` whose rate is `wanted`, or nothing.
fn named(candidates, wanted) {
    for candidate in candidates {
        if candidate.rate() == wanted { return candidate; }
    }
    ()
}

// How the planner's simulations move, from the Scout's view: finish,
// treat, rescue, then share or move together toward the site the Scout
// has intelligence of, then scan, go north, go south.
fn informed(seen, candidates, random) {
    let preferred = ();
    for site in sites() {
        if holds(seen, "agent/scout", `intel/${site}`) || holds(seen, "agent/scout", `shared-intel/${site}`) {
            preferred = site;
            break;
        }
    }
    for wanted in ["finish", "treat"] {
        let found = named(candidates, wanted);
        if found != () { return found; }
    }
    for candidate in candidates {
        if candidate.rate().starts_with("rescue/") { return candidate; }
    }
    if preferred != () {
        for wanted in [`share/${preferred}`, `move-together/${preferred}`] {
            let found = named(candidates, wanted);
            if found != () { return found; }
        }
    }
    for wanted in ["begin-scan", "move-direct/north", "move-direct/south"] {
        let found = named(candidates, wanted);
        if found != () { return found; }
    }
    ()
}

fn solved(state) { state.balance("success", "solved") > 0 }

// The planner's payoff where the mission is solved, and its estimate
// wherever a simulation stops short.
fn payoff(state) { if solved(state) { [1.0] } else { () } }
fn unsolved(state) { [0.0] }

// The first exchange of a rate under `prefix` that applies, in rate order.
fn first_applicable(state, prefix) {
    for rate in state.rates(prefix) {
        let proposal = action(rate);
        if state.applicable(proposal) { return proposal; }
    }
    ()
}

// What the team does under `policy`, `north-together` or
// `share-and-coordinate`, once Nature has drawn: Nature's report to a
// scan; finish, treat, or rescue where the Scout stands; Nature's hazard
// where the team arrived; and otherwise both go north, or the Scout scans,
// shares what its view shows, and the team follows the Medic's view.
fn policy_action(state, policy) {
    if state.balance("mission", "awaiting-scan") > 0 { return first_applicable(state, "resolve-scan/"); }
    if state.balance("mission", "rescued") > 0 { return action("finish"); }
    if state.balance("mission", "needs-treatment") > 0 { return action("treat"); }
    if state.balance("mission", "area-safe") > 0 {
        for site in ["base", "north", "south"] {
            if state.balance("agent/scout", `at/${site}`) > 0 { return action(`rescue/${site}`); }
        }
        return ();
    }
    if state.balance("mission", "awaiting-encounter") > 0 { return first_applicable(state, "encounter/"); }
    if policy == "north-together" { return action("move-direct/north"); }
    let scout = state.view(["agent/scout", "mission", "success"]);
    if holds(scout, "agent/scout", "sensor") { return action("begin-scan"); }
    for site in sites() {
        if holds(scout, "agent/scout", `intel/${site}`) { return action(`share/${site}`); }
    }
    let medic = state.view(["agent/medic", "mission", "success"]);
    for site in sites() {
        if holds(medic, "agent/medic", `intel/${site}`) { return action(`move-together/${site}`); }
    }
    ()
}

// A turn of `policy`: Nature draws the scenario while it is unresolved,
// and otherwise the policy acts.
fn turn(state, random, policy) {
    if state.balance("nature", "unresolved") > 0 { return nature(state, random); }
    policy_action(state, policy)
}

// The fixed policies, as a strategy calls a policy.
fn direct_north(state, random) { turn(state, random, "north-together") }
fn share_and_coordinate(state, random) { turn(state, random, "share-and-coordinate") }

// Whether the mission succeeded, the time it took, and whether the Medic's
// kit was used.
fn succeeded(state) { if solved(state) { 1 } else { 0 } }
fn elapsed(state) { state.balance("mission", "elapsed-time") }
fn kit_used(state) { if state.balance("agent/medic", "used-medical-kit") > 0 { 1 } else { 0 } }
