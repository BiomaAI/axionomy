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
// The fewest roads from `from` to `to`, as the world's map gives it.
fn distance(state, from, to) {
    if from == to { 0 } else { state.values(`depart/scout/${from}/${to}`).distance.to_int() }
}

// The roads between the expedition's places, each once: the journeys the
// map gives one road long, from the earlier place to the later.
fn roads(state) {
    let places = locations();
    let roads = [];
    for journey in state.instances("depart/scout") {
        let parts = journey.split("/");
        if places.index_of(parts[2]) < places.index_of(parts[3])
            && state.values(journey).distance == 1 {
            roads.push([parts[2], parts[3]]);
        }
    }
    roads
}

// Even objectives are casualties, odd ones archives.
fn patient(id) { id % 2 == 0 }

fn now(state) { state.balance("world", "time").to_int() }

// What Nature and the clock's events settle, in the order they settle: the
// weather and the end of a clinical pulse; then objective by objective, its
// decline, failure, finished work and discovery; then teammate by teammate,
// its arrival; then a scan's report. The first that applies, or nothing.
fn nature_response(state) {
    let settling = state.moves("weather") + state.moves("finish-clinical");
    if !settling.is_empty() { return settling[0]; }
    let kinds = [
        state.moves("clinical"),
        state.moves("fail"),
        state.moves("finish-extract") + state.moves("finish-collect"),
        state.moves("discover"),
    ];
    for id in 0..state.balance("world", "count").to_int() {
        let objective = `objective/${id}`;
        for moves in kinds {
            for due in moves {
                if due.account("objective") == objective { return due; }
            }
        }
    }
    let arrivals = state.moves("arrive");
    for agent in agents() {
        for due in arrivals {
            if due.rate().split("/")[1] == agent { return due; }
        }
    }
    let reports = state.moves("report");
    if reports.is_empty() { () } else { reports[0] }
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
        && val(seen, own, "energy") >= distance(seen, from, to) + val(seen, own, "harm") {
        return seen.exchange_of(`depart/${agent}/${from}/${to}`);
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
            if from == "base" { return seen.exchange_of(`deliver/${id}`); }
            return move_to(seen, agent, from, "base");
        }
    }
    if from == "base" && val(seen, "world", "supply") > 0
        && (val(seen, own, "energy") < 7 || (agent == "medic" && val(seen, own, "medical-kit") == 0)) {
        return seen.exchange_of(`resupply/${agent}`);
    }
    if agent == "medic" {
        if val(seen, own, "medical-kit") > 1 && val(seen, own, "harm") >= 2 {
            return seen.exchange_of(`treat-agent/medic/${from}`);
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
                return seen.exchange_of(`treat-patient/${id}/${site}/${restore}`);
            }
            return ();
        }
        if from != "base" { return move_to(seen, agent, from, "base"); }
        return ();
    }
    // The Scout learns a sector before acting on a fallible hint.
    if val(seen, own, `unsearched/${from}`) > 0 { return seen.exchange_of(`survey/${from}`); }
    for id in 0..count {
        if !patient(id) { continue; }
        let site = known(seen, own, id);
        if site == () { continue; }
        let objective = `objective/${id}`;
        if val(seen, objective, "located") == 0 { continue; }
        if val(seen, own, `shared/${id}/${site}`) == 0 {
            if val(seen, "world", `team-at/medic/${from}`) > 0 {
                return seen.exchange_of(`share/${id}/${site}/false`);
            }
            if policy == "radio-coordination" && val(seen, own, "radio") > 0 {
                return seen.exchange_of(`share/${id}/${site}/true`);
            }
            return move_to(seen, agent, from, "base");
        }
        if from == site && val(seen, "world", `team-at/medic/${site}`) > 0
            && val(seen, "world", "permit/medic") > 0 && val(seen, objective, "stable") > 0 {
            return seen.exchange_of(`extract/${id}/${site}`);
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
            if from == site { return seen.exchange_of(`collect/${id}/${site}`); }
            let moved = move_to(seen, agent, from, site);
            if moved != () { return moved; }
        }
    }
    if policy != "north-first" && from == "base" && val(seen, own, "sensor") > 0
        && (val(seen, own, "used-sensor") == 0 || val(seen, "world", "completed") > 0) {
        return seen.exchange_of("scan");
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
            sites.push([if preferred { 0 } else { 1 }, distance(seen, from, site), site]);
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
    let close = state.exchange_of("close");
    if state.applicable(close) { return close; }
    for agent in agents() {
        let decided = decision(agent_view(state, agent), agent, policy);
        if decided != () && state.applicable(decided) { return decided; }
    }
    state.exchange_of(`tick/${now(state)}`)
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
