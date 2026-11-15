// Rescue's incident. Nature loses, worsens, discovers and reports patients;
// the responder sees itself, the world and the patients someone knows about,
// and triages: unload or carry home a patient on board, stabilize or load the
// most fragile patient found here, search here, resupply or scan at base,
// drive to a found patient, or search the most promising unsearched place.

fn locations() { ["base", "north", "south", "east", "harbor", "west"] }

fn value(state, account, asset) { state.balance(account, asset).to_int() }

// How many roads apart two places are, as the world's map records it.
fn distance(state, from, to) {
    if from == to { 0 } else { value(state, "world", `distance/${from}/${to}`) }
}

// The roads between the places the incident takes place at, each once: the
// places the map records one road apart.
fn roads(state) {
    let places = locations();
    let roads = [];
    for (from, index) in places {
        for to in places.extract(index + 1) {
            if distance(state, from, to) == 1 { roads.push([from, to]); }
        }
    }
    roads
}

// Where `account` stands, if anywhere.
fn whereabouts(seen, account) {
    for location in locations() {
        if value(seen, account, `at/${location}`) > 0 { return location; }
    }
    ()
}

// What happens without anyone choosing, the first that is due: patient by
// patient, a loss before a decline; then a discovery; then a report.
fn nature(state) {
    let declines = state.moves("lose") + state.moves("deteriorate");
    for id in 0..value(state, "world", "patient-count") {
        let patient = `patient/${id}`;
        for due in declines {
            if due.account("patient") == patient { return due; }
        }
    }
    let news = state.moves("discover") + state.moves("report");
    if news.is_empty() { () } else { news[0] }
}

// The accounts the responder may see.
fn visible(state) {
    let visible = ["responder", "world"];
    let count = value(state, "world", "patient-count");
    for id in 0..count {
        if locations().some(|location| value(state, "world", `known/${id}/${location}`) > 0) {
            visible.push(`patient/${id}`);
        }
    }
    visible
}

fn drive(seen, from, to, urgent, rapid) {
    let fast = (urgent || rapid) && value(seen, "responder", "fuel") >= distance(seen, from, to) * 2;
    seen.exchange_of(`drive/${from}/${to}/${fast}`)
}

// The responder's choice from what it sees, under `policy`: `triage`,
// `rapid-north` or `stabilize-first`.
fn decide(seen, policy) {
    let count = value(seen, "world", "patient-count");
    if value(seen, "world", "resolved") == count { return seen.exchange_of("close"); }
    let from = whereabouts(seen, "responder");
    if from == () { return (); }
    let now = value(seen, "world", "time");
    let rapid = policy == "rapid-north";

    for id in 0..count {
        if value(seen, "responder", `carrying/${id}`) > 0 {
            if from == "base" { return seen.exchange_of(`unload/${id}`); }
            let patient = `patient/${id}`;
            let urgent = value(seen, patient, "vitality") <= 1
                || value(seen, patient, "next-decline") <= now + distance(seen, from, "base") * 2 + 1;
            return drive(seen, from, "base", urgent, rapid);
        }
    }

    // Found and alive, the most fragile first.
    let found = [];
    for id in 0..count {
        let patient = `patient/${id}`;
        if value(seen, patient, "found") > 0 && value(seen, patient, "alive") > 0 { found.push(id); }
    }
    found.sort(|a, b| {
        let vitality = value(seen, `patient/${a}`, "vitality") - value(seen, `patient/${b}`, "vitality");
        if vitality != 0 { return vitality; }
        value(seen, `patient/${a}`, "next-decline") - value(seen, `patient/${b}`, "next-decline")
    });
    for id in found {
        let patient = `patient/${id}`;
        if value(seen, patient, `at/${from}`) == 0 { continue; }
        let vitality = value(seen, patient, "vitality");
        let arrival = now + 2 + distance(seen, from, "base") * if rapid { 1 } else { 2 };
        let decline = value(seen, patient, "next-decline");
        let projected = if arrival >= decline { (arrival - decline) / 3 + 1 } else { 0 };
        if !rapid
            && value(seen, "responder", "medical-kit") > 0
            && value(seen, patient, "treatable") > 0
            && (policy == "stabilize-first" || vitality <= projected)
        {
            return seen.exchange_of(`stabilize/${id}/${from}/${value(seen, patient, "harm") > 0}`);
        }
        return seen.exchange_of(`load/${id}/${from}`);
    }

    if value(seen, "responder", `unsearched/${from}`) > 0 { return seen.exchange_of(`search/${from}`); }
    if from == "base" {
        if value(seen, "world", "supply-available") > 0
            && (value(seen, "responder", "fuel") < 4
                || value(seen, "responder", "energy") < 4
                || value(seen, "responder", "medical-kit") == 0)
        {
            return seen.exchange_of("resupply");
        }
        if !rapid
            && value(seen, "responder", "sensor") > 0
            && value(seen, "responder", "used-sensor") <= value(seen, "world", "saved-count")
        {
            return seen.exchange_of("scan");
        }
    }
    if !found.is_empty() {
        for location in locations() {
            if value(seen, `patient/${found[0]}`, `at/${location}`) > 0 {
                return drive(seen, from, location, true, rapid);
            }
        }
    }

    // Unsearched places: hinted ones first (north first when rapid), then the
    // nearest, then by name.
    let destinations = locations().filter(|location| value(seen, "responder", `unsearched/${location}`) > 0);
    destinations.sort(|a, b| {
        let first = |location| if rapid {
            if location != "north" { 1 } else { 0 }
        } else {
            if value(seen, "responder", `hint/${location}`) == 0 { 1 } else { 0 }
        };
        let preference = first.call(a) - first.call(b);
        if preference != 0 { return preference; }
        let nearer = distance(seen, from, a) - distance(seen, from, b);
        if nearer != 0 { return nearer; }
        if a < b { -1 } else if a > b { 1 } else { 0 }
    });
    if !destinations.is_empty() {
        let to = destinations[0];
        let needed = distance(seen, from, to) + distance(seen, to, "base");
        if from != "base"
            && (value(seen, "responder", "fuel") < needed || value(seen, "responder", "energy") < 3)
        {
            return drive(seen, from, "base", false, rapid);
        }
        return drive(seen, from, to, value(seen, "responder", `hint/${to}`) > 0, rapid);
    }
    seen.exchange_of("wait")
}

// One turn of an incident: Nature first, then the responder's decision if it
// applies, and otherwise waiting.
fn turn(state, policy) {
    let response = nature(state);
    if type_of(response) != "()" { return response; }
    let decided = decide(state.view(visible(state)), policy);
    if type_of(decided) != "()" && state.applicable(decided) { return decided; }
    state.exchange_of("wait")
}

// The decision from what the responder may see of `state`.
fn decide_seen(state, policy) {
    decide(state.view(visible(state)), policy)
}

// The incident's policies, as a strategy calls a policy.
fn triage(state, random) { turn(state, "triage") }
fn rapid_north(state, random) { turn(state, "rapid-north") }
fn stabilize_first(state, random) { turn(state, "stabilize-first") }

// What an incident's outcome is measured by.
fn saved(state) { state.balance("world", "saved-count") }
fn saved_vitality(state) { state.balance("world", "saved-vitality") }
fn deaths(state) { state.balance("world", "death-count") }
fn spent_fuel(state) { state.balance("responder", "spent-fuel") }
fn kits_used(state) { state.balance("responder", "used-medical-kit") }
fn elapsed(state) { state.balance("world", "time") }
