// Rescue's incident. Nature loses, worsens, discovers and reports patients;
// the responder sees itself, the world and the patients someone knows about,
// and triages: unload or carry home a patient on board, stabilize or load the
// most fragile patient found here, search here, resupply or scan at base,
// drive to a found patient, or search the most promising unsearched place.

fn locations() { ["base", "north", "south", "east", "harbor", "west"] }

fn roads() {
    [["base", "north"], ["base", "south"], ["base", "harbor"], ["north", "east"],
     ["east", "harbor"], ["south", "west"], ["west", "harbor"]]
}

fn distance(from, to) { distances(roads(), from)[to] }

fn value(state, account, asset) { state.balance(account, asset).to_int() }

// The exchange of `rate`, with the accounts its roles bind.
fn action(rate) {
    let bindings = #{ world: "world", responder: "responder" };
    let parts = rate.split("/");
    let kind = parts[0];
    if kind == "discover" {
        bindings.patient = `patient/${parts[1]}`;
        bindings.nature = "nature";
    } else if kind == "report" {
        bindings.nature = "nature";
    } else if kind in ["deteriorate", "stabilize", "load", "unload", "lose"] {
        bindings.patient = `patient/${parts[1]}`;
    }
    exchange(rate, bindings, 1)
}

// What happens without anyone choosing: the first that applies.
fn nature(state) {
    let count = value(state, "world", "patient-count");
    for id in 0..count {
        for cause in ["deterioration", "deadline", "horizon"] {
            for phase in ["hidden", "found", "onboard"] {
                let lost = action(`lose/${id}/${phase}/${cause}`);
                if state.applicable(lost) { return lost; }
            }
        }
        let worse = action(`deteriorate/${id}`);
        if state.applicable(worse) { return worse; }
    }
    for id in 0..count {
        for location in locations() {
            let discovered = action(`discover/${id}/${location}`);
            if state.applicable(discovered) { return discovered; }
        }
    }
    for round in 0..2 {
        for location in locations() {
            let reported = action(`report/${round}/${location}`);
            if state.applicable(reported) { return reported; }
        }
    }
    ()
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
    let fast = (urgent || rapid) && value(seen, "responder", "fuel") >= distance(from, to) * 2;
    action(`drive/${from}/${to}/${fast}`)
}

// The responder's choice from what it sees, under `policy`: `triage`,
// `rapid-north` or `stabilize-first`.
fn decide(seen, policy) {
    let count = value(seen, "world", "patient-count");
    if value(seen, "world", "resolved") == count { return action("close"); }
    let from = ();
    for location in locations() {
        if value(seen, "responder", `at/${location}`) > 0 { from = location; break; }
    }
    if from == () { return (); }
    let now = value(seen, "world", "time");
    let rapid = policy == "rapid-north";

    for id in 0..count {
        if value(seen, "responder", `carrying/${id}`) > 0 {
            if from == "base" { return action(`unload/${id}`); }
            let patient = `patient/${id}`;
            let urgent = value(seen, patient, "vitality") <= 1
                || value(seen, patient, "next-decline") <= now + distance(from, "base") * 2 + 1;
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
        let arrival = now + 2 + distance(from, "base") * if rapid { 1 } else { 2 };
        let decline = value(seen, patient, "next-decline");
        let projected = if arrival >= decline { (arrival - decline) / 3 + 1 } else { 0 };
        if !rapid
            && value(seen, "responder", "medical-kit") > 0
            && value(seen, patient, "treatable") > 0
            && (policy == "stabilize-first" || vitality <= projected)
        {
            return action(`stabilize/${id}/${from}/${value(seen, patient, "harm") > 0}`);
        }
        return action(`load/${id}/${from}`);
    }

    if value(seen, "responder", `unsearched/${from}`) > 0 { return action(`search/${from}`); }
    if from == "base" {
        if value(seen, "world", "supply-available") > 0
            && (value(seen, "responder", "fuel") < 4
                || value(seen, "responder", "energy") < 4
                || value(seen, "responder", "medical-kit") == 0)
        {
            return action("resupply");
        }
        if !rapid
            && value(seen, "responder", "sensor") > 0
            && value(seen, "responder", "used-sensor") <= value(seen, "world", "saved-count")
        {
            return action("scan");
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
        let nearer = distance(from, a) - distance(from, b);
        if nearer != 0 { return nearer; }
        if a < b { -1 } else if a > b { 1 } else { 0 }
    });
    if !destinations.is_empty() {
        let to = destinations[0];
        let needed = distance(from, to) + distance(to, "base");
        if from != "base"
            && (value(seen, "responder", "fuel") < needed || value(seen, "responder", "energy") < 3)
        {
            return drive(seen, from, "base", false, rapid);
        }
        return drive(seen, from, to, value(seen, "responder", `hint/${to}`) > 0, rapid);
    }
    action("wait")
}

// One turn of an incident: Nature first, then the responder's decision if it
// applies, and otherwise waiting.
fn turn(state, policy) {
    let response = nature(state);
    if type_of(response) != "()" { return response; }
    let decided = decide(state.view(visible(state)), policy);
    if type_of(decided) != "()" && state.applicable(decided) { return decided; }
    action("wait")
}

// The decision from what the responder may see of `state`.
fn decide_seen(state, policy) {
    decide(state.view(visible(state)), policy)
}
