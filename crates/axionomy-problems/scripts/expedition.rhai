// Mission's Showcase and Stress: the expedition. A Scout and a Medic move
// and work on one clock; the Scout surveys sectors and scans for fallible
// hints, discovers an archive and casualties, and briefs the Medic by radio
// or in person; casualties need stabilizing and both teammates to extract,
// and everything carried must reach base before its deadline. Weather and
// clinical decline are Nature's, and settle before anyone works.
//
// Every turn Nature responds first, then the expedition closes when it can,
// then the first teammate whose own view gives a decision that applies
// acts, and otherwise the clock ticks. A teammate's view is its own
// account, the world's public state and the objectives it has learned of.

fn locations() { ["base", "north", "east", "south", "west"] }
fn agents() { ["scout", "medic"] }
fn roads() {
    [["base", "north"], ["base", "south"], ["north", "east"], ["east", "south"],
     ["south", "west"], ["west", "north"]]
}

// The fewest roads from `from` to `to`.
fn distance(from, to) { distances(roads(), from)[to] }

// Even objectives are casualties, odd ones archives.
fn patient(id) { id % 2 == 0 }

// The exchange of `rate`, with the accounts its roles bind: the world and
// both teammates always, the objective a rate names, and Nature where it
// acts.
fn action(rate) {
    let parts = rate.split("/");
    let kind = parts[0];
    let bindings = #{ world: "world", scout: "agent/scout", medic: "agent/medic" };
    if kind == "discover" {
        bindings.objective = `objective/${parts[1]}`;
        bindings.nature = "nature";
    } else if ["collect", "treat-patient", "extract", "finish-collect", "finish-extract", "deliver",
               "clinical", "fail"].contains(kind) {
        bindings.objective = `objective/${parts[1]}`;
    } else if kind == "report" || kind == "weather" {
        bindings.nature = "nature";
    }
    exchange(rate, bindings, 1)
}

fn now(state) { state.balance("world", "time").to_int() }

// What Nature and the clock's events settle, in the order they settle: the
// weather, the clinical pulses and their end, each objective's decline,
// failure, finished work and discovery, each teammate's arrival, and the
// scans' reports; the first that applies, or nothing.
fn nature_response(state) {
    let count = state.balance("world", "count").to_int();
    let rates = [];
    for location in locations() {
        for opening in [false, true] { rates.push(`weather/${location}/${opening}`); }
    }
    for day in [8, 16, 24] { rates.push(`finish-clinical/${day}`); }
    for id in 0..count {
        if patient(id) {
            for day in [8, 16, 24] {
                for active in [true, false] { rates.push(`clinical/${id}/${day}/${active}`); }
            }
        }
        for phase in ["hidden", "located", "carried"] {
            for dead in [true, false] { rates.push(`fail/${id}/${phase}/${dead}`); }
        }
        rates.push(if patient(id) { `finish-extract/${id}` } else { `finish-collect/${id}` });
        for location in locations() { rates.push(`discover/${id}/${location}`); }
    }
    for agent in agents() {
        for to in locations() {
            for hazardous in [false, true] { rates.push(`arrive/${agent}/${to}/${hazardous}`); }
        }
    }
    for round in 0..2 {
        for location in locations() { rates.push(`report/${round}/${location}`); }
    }
    for rate in rates {
        let response = action(rate);
        if state.applicable(response) { return response; }
    }
    ()
}

// What `agent` sees: its own account, the world's, and every objective it
// knows a site of.
fn agent_view(state, agent) {
    let own = `agent/${agent}`;
    let accounts = ["world", own];
    let count = state.balance("world", "count").to_int();
    for id in 0..count {
        for location in locations() {
            if state.balance(own, `known/${id}/${location}`) > 0 {
                accounts.push(`objective/${id}`);
                break;
            }
        }
    }
    state.view(accounts)
}

// A balance `seen` shows, as an integer; nothing it cannot see.
fn val(seen, account, asset) { seen.balance(account, asset).to_int() }

// The first site `own` knows objective `id` to be at, or nothing.
fn known(seen, own, id) {
    for location in locations() {
        if val(seen, own, `known/${id}/${location}`) > 0 { return location; }
    }
    ()
}

// `agent` departing `from` for `to`, if the weather there is open and its
// energy covers the journey, slowed by its harm; nothing otherwise.
fn move_to(seen, agent, from, to) {
    let own = `agent/${agent}`;
    if to != from && val(seen, "world", `weather-open/${to}`) > 0
        && val(seen, own, "energy") >= distance(from, to) + val(seen, own, "harm") {
        return action(`depart/${agent}/${from}/${to}`);
    }
    ()
}

// Unsearched sites ranked for a search: first those `policy` prefers,
// north for `north-first` and hinted sites otherwise, then the nearer,
// then by name.
fn ranked(a, b) {
    if a[0] != b[0] { return a[0] - b[0]; }
    if a[1] != b[1] { return a[1] - b[1]; }
    if a[2] < b[2] { -1 } else if a[2] > b[2] { 1 } else { 0 }
}

// What `agent` decides from its view `seen` under `policy`,
// `radio-coordination`, `rendezvous` or `north-first`, or nothing.
fn decision(seen, agent, policy) {
    let own = `agent/${agent}`;
    if val(seen, own, "ready") == 0 || val(seen, "world", `permit/${agent}`) == 0 { return (); }
    let from = ();
    for location in locations() {
        if val(seen, own, `at/${location}`) > 0 { from = location; break; }
    }
    if from == () { return (); }
    let count = val(seen, "world", "count");
    for id in 0..count {
        if val(seen, own, `cargo/${id}`) > 0 {
            if from == "base" { return action(`deliver/${id}`); }
            return move_to(seen, agent, from, "base");
        }
    }
    if from == "base" && val(seen, "world", "supply") > 0
        && (val(seen, own, "energy") < 7 || (agent == "medic" && val(seen, own, "medical-kit") == 0)) {
        return action(`resupply/${agent}`);
    }
    if agent == "medic" {
        if val(seen, own, "medical-kit") > 1 && val(seen, own, "harm") >= 2 {
            return action(`treat-agent/medic/${from}`);
        }
        for id in 0..count {
            if !patient(id) { continue; }
            let site = known(seen, own, id);
            if site == () { continue; }
            let objective = `objective/${id}`;
            if val(seen, objective, "located") == 0 { continue; }
            if val(seen, own, "medical-kit") == 0 && val(seen, objective, "unstable") > 0
                && val(seen, "world", "supply") > 0 {
                return move_to(seen, agent, from, "base");
            }
            if from != site { return move_to(seen, agent, from, site); }
            if val(seen, objective, "unstable") > 0 && val(seen, own, "medical-kit") > 0 {
                if val(seen, "world", "next-clinical") <= val(seen, "world", "time") + 1 { return (); }
                let restore = val(seen, objective, "harm") > 0;
                return action(`treat-patient/${id}/${site}/${restore}`);
            }
            return ();
        }
        if from != "base" { return move_to(seen, agent, from, "base"); }
        return ();
    }
    // The Scout learns a sector before acting on a fallible hint.
    if val(seen, own, `unsearched/${from}`) > 0 { return action(`survey/${from}`); }
    for id in 0..count {
        if !patient(id) { continue; }
        let site = known(seen, own, id);
        if site == () { continue; }
        let objective = `objective/${id}`;
        if val(seen, objective, "located") == 0 { continue; }
        if val(seen, own, `shared/${id}/${site}`) == 0 {
            if val(seen, "world", `team-at/medic/${from}`) > 0 {
                return action(`share/${id}/${site}/false`);
            }
            if policy == "radio-coordination" && val(seen, own, "radio") > 0 {
                return action(`share/${id}/${site}/true`);
            }
            return move_to(seen, agent, from, "base");
        }
        if from == site && val(seen, "world", `team-at/medic/${site}`) > 0
            && val(seen, "world", "permit/medic") > 0 && val(seen, objective, "stable") > 0 {
            return action(`extract/${id}/${site}`);
        }
        if from != site && val(seen, "world", `weather-open/${site}`) > 0 {
            return move_to(seen, agent, from, site);
        }
        // While the Medic is on the way, hold the rendezvous.
        if from == site { return (); }
    }
    for id in 0..count {
        if patient(id) { continue; }
        if val(seen, `objective/${id}`, "located") == 0 { continue; }
        let site = known(seen, own, id);
        if site != () {
            if from == site { return action(`collect/${id}/${site}`); }
            let moved = move_to(seen, agent, from, site);
            if moved != () { return moved; }
        }
    }
    if policy != "north-first" && from == "base" && val(seen, own, "sensor") > 0
        && (val(seen, own, "used-sensor") == 0 || val(seen, "world", "completed") > 0) {
        return action("scan");
    }
    let sites = [];
    for site in locations() {
        if val(seen, own, `unsearched/${site}`) > 0 && val(seen, "world", `weather-open/${site}`) > 0 {
            let preferred = if policy == "north-first" {
                site == "north"
            } else {
                let hinted = false;
                for id in 0..count {
                    if val(seen, own, `hint/${id}/${site}`) > 0 { hinted = true; }
                }
                hinted
            };
            sites.push([if preferred { 0 } else { 1 }, distance(from, site), site]);
        }
    }
    sites.sort(|a, b| ranked(a, b));
    for site in sites {
        let moved = move_to(seen, agent, from, site[2]);
        if moved != () { return moved; }
    }
    ()
}

// One turn of the expedition under `policy`.
fn turn(state, policy) {
    let response = nature_response(state);
    if response != () { return response; }
    let close = action("close");
    if state.applicable(close) { return close; }
    for agent in agents() {
        let decided = decision(agent_view(state, agent), agent, policy);
        if decided != () && state.applicable(decided) { return decided; }
    }
    action(`tick/${now(state)}`)
}

// The policies, as a strategy calls a policy: brief by radio, brief only
// in person, or search north first without scanning.
fn radio_coordination(state, random) { turn(state, "radio-coordination") }
fn rendezvous(state, random) { turn(state, "rendezvous") }
fn north_first(state, random) { turn(state, "north-first") }

// What an expedition is measured by: objectives completed and lost,
// evacuated vitality, radio charges and team energy spent, and the time.
fn completed(state) { state.balance("world", "completed") }
fn lost(state) { state.balance("world", "failed-count") }
fn vitality(state) { state.balance("world", "saved-vitality") }
fn radio_spent(state) { state.balance("agent/scout", "used-radio") }
fn energy_spent(state) {
    state.balance("agent/scout", "spent-energy") + state.balance("agent/medic", "spent-energy")
}
fn elapsed(state) { state.balance("world", "time") }
