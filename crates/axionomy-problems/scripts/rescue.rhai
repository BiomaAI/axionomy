// Rescue's Micro: one survivor hidden north or south, and a sensor whose
// reading is wrong for one seed in four. Nature draws the hidden truth and
// seed from its weighted prior and answers the observation the agent asks
// for; the agent sees only its own account. It senses and follows the
// reading, or goes north without looking, then rescues where it stands and
// finishes.

fn sites() { ["north", "south", "east", "west", "harbor", "hills"] }

// The first exchange of a rate under `prefix` that applies, in rate order.
fn first_applicable(state, prefix) {
    let found = state.moves(prefix);
    if found.is_empty() { () } else { found[0] }
}

// Nature's draw from its prior: every scenario it encodes, in rate order,
// weighted by what Nature holds of it, chosen by the event `scenario`,
// which a run meets once, as it begins.
fn instantiation(state, random) {
    let scenarios = state.moves("instantiate");
    let weights = [];
    for scenario in scenarios {
        let parts = scenario.rate().split("/");
        weights.push(state.balance("nature", `scenario-weight/${parts[1]}/${parts[2]}`));
    }
    if scenarios.is_empty() { return (); }
    scenarios[random.sample("scenario", weights)]
}

fn holds(seen, asset) { seen.balance("agent", asset) > 0 }

// The agent's choice from what it sees, under `policy`:
// `observe-then-follow` or `direct-north`.
fn decide(seen, policy) {
    if holds(seen, "evacuated") || holds(seen, "rescued") { return seen.exchange_of("finish"); }
    for location in sites() {
        if holds(seen, `at/${location}`) { return seen.exchange_of(`rescue/${location}`); }
    }
    if !holds(seen, "at/base") { return (); }
    if policy == "observe-then-follow" && holds(seen, "sensor") {
        return seen.exchange_of("begin-observe");
    }
    let destination = ();
    if policy == "direct-north" {
        destination = "north";
    } else {
        for location in sites() {
            if holds(seen, `belief/${location}`) { destination = location; break; }
        }
    }
    if destination == () { return (); }
    // A reading may point where the agent has no road, as seed 0 does of a
    // survivor south in Micro: the move it proposes then names no rate, and
    // the economy refuses it.
    let rate = `move/base/${destination}`;
    try {
        return seen.exchange_of(rate);
    } catch {
        return exchange(rate, #{}, 1);
    }
}

// One turn: Nature draws the scenario while it is unresolved and answers the
// observation the agent awaits; an agent holding a rescued survivor
// evacuates them where it can; otherwise the agent decides from its view.
fn turn(state, random, policy) {
    if state.balance("nature", "unresolved") > 0 { return instantiation(state, random); }
    if state.balance("agent", "awaiting-observation") > 0 {
        return first_applicable(state, "resolve-observation");
    }
    if state.balance("agent", "rescued") > 0 {
        let evacuation = first_applicable(state, "evacuate");
        if type_of(evacuation) != "()" { return evacuation; }
    }
    decide(state.view(["agent"]), policy)
}

// The policies, as a strategy calls a policy.
fn observe_then_follow(state, random) { turn(state, random, "observe-then-follow") }
fn direct_north(state, random) { turn(state, random, "direct-north") }

// Whether the survivor was rescued, whether the sensor was spent, and the
// energy spent.
fn succeeded(state) { if state.balance("agent", "solved") > 0 { 1 } else { 0 } }
fn sensed(state) { if state.balance("agent", "used-sensor") > 0 { 1 } else { 0 } }
fn spent_energy(state) { state.balance("agent", "spent-energy") }
