// Logistics' Micro: one vehicle carries packages from the depot to a
// customer, by the direct road or the reliable one through a junction.
// Every leg it departs on waits for Nature to draw its weather from the
// weights Nature holds, clear, delayed or a breakdown; a clear or delayed
// leg then arrives, and a breakdown is repaired before Nature draws again.
// Each draw is the event of its leg at the vehicle's elapsed time, so any
// run that travels that leg then meets the same weather.
// The fixed policies drive one route; the planner searches its choices with
// Monte Carlo tree search, Nature's draws as chance in its tree.

fn orders() { ["a", "b", "c", "d"] }

fn routes() {
    ["direct-out", "direct-back", "safe-out-first", "safe-out-second", "safe-back-first",
     "safe-back-second"]
}

fn outcomes() { ["clear", "delayed", "breakdown"] }

// What a refuel buys, and the fuel below which the vehicle refuels.
fn refuel_amount() { 4 }

// The exchange of `rate`, with the accounts its roles bind.
fn action(rate) {
    let parts = rate.split("/");
    let kind = parts[0];
    let bindings = #{ vehicle: "vehicle" };
    if kind == "load" || kind == "deliver" {
        bindings.order = `order/${parts[1]}`;
    } else if kind == "refuel" {
        bindings["fuel-station"] = "fuel-station";
    } else if kind != "depart" {
        bindings.nature = "nature";
    }
    exchange(rate, bindings, 1)
}

// The route the vehicle is on, if it is travelling.
fn active_route(state) {
    for route in routes() {
        if state.balance("vehicle", `traveling/${route}`) > 0 { return route; }
    }
    ()
}

// Where the vehicle stands, if it is not travelling.
fn location(state) {
    for place in ["depot", "junction", "customer"] {
        if state.balance("vehicle", `at/${place}`) > 0 { return place; }
    }
    ()
}

// The order whose package the vehicle carries, if any.
fn carried(state) {
    for order in orders() {
        if state.balance("vehicle", `package/${order}`) > 0 { return order; }
    }
    ()
}

// Nature's draw for the leg under way while its weather is ready: each
// resolution that applies, with the weight Nature holds of it, as
// `[exchange, weight]` pairs in outcome order; none otherwise.
fn weather(state) {
    let route = active_route(state);
    if route == () || state.balance("nature", "weather-ready") == 0 { return []; }
    let weighted = [];
    for outcome in outcomes() {
        let resolution = action(`resolve/${route}/${outcome}`);
        if state.applicable(resolution) {
            weighted.push([resolution, state.balance("nature", `outcome-weight/${route}/${outcome}`)]);
        }
    }
    weighted
}

// The weather of the leg under way, one of `weighted` outcomes: the event
// `weather/<leg>/<elapsed time>`, the leg's weather met at that time. A
// leg's weather is drawn as it begins and again after each repair, which
// takes time, so each draw names an occurrence of its own.
fn drawn(state, weighted, random) {
    if weighted.is_empty() { return (); }
    let weights = [];
    for outcome in weighted { weights.push(outcome[1]); }
    let event = `weather/${active_route(state)}/${state.balance("vehicle", "elapsed-time")}`;
    weighted[random.sample(event, weights)][0]
}

// What `policy`, `direct` or `reliable`, does once Nature has drawn: repair
// a breakdown, arrive, refuel or load at the depot and depart on its route,
// deliver at the customer and head back, or go on from the junction.
fn decide(state, policy) {
    let route = active_route(state);
    if route != () {
        if state.balance("nature", "weather-ready") > 0 { return (); }
        if state.balance("nature", `outcome/${route}/breakdown`) > 0 {
            return action(`repair/${route}`);
        }
        for outcome in ["clear", "delayed"] {
            if state.balance("nature", `outcome/${route}/${outcome}`) > 0 {
                return action(`arrive/${route}/${outcome}`);
            }
        }
        return ();
    }
    let carrying = carried(state);
    let at = location(state);
    if at == "depot" {
        if carrying != () {
            return action(if policy == "direct" { "depart/direct-out" } else { "depart/safe-out-first" });
        }
        if state.balance("vehicle", "fuel") < refuel_amount() { return action("refuel"); }
        for order in orders() {
            if state.balance(`order/${order}`, "waiting") > 0 { return action(`load/${order}`); }
        }
        return ();
    }
    if at == "customer" {
        if carrying != () { return action(`deliver/${carrying}`); }
        return action(if policy == "direct" { "depart/direct-back" } else { "depart/safe-back-first" });
    }
    if at == "junction" {
        return action(if carrying != () { "depart/safe-out-second" } else { "depart/safe-back-second" });
    }
    ()
}

// A turn of `policy`: Nature's draw while the weather is ready, and the
// policy's move otherwise.
fn turn(state, random, policy) {
    if active_route(state) != () && state.balance("nature", "weather-ready") > 0 {
        return drawn(state, weather(state), random);
    }
    decide(state, policy)
}

// The fixed-route policies, as a strategy calls a policy.
fn direct(state, random) { turn(state, random, "direct") }
fn reliable(state, random) { turn(state, random, "reliable") }

// The moves the planner weighs, in the order it weighs them: once Nature
// has drawn, the repair or the arrival; at the depot, both routes out with
// a package or the waiting orders to load without, and a refuel when fuel
// is short; at the customer the delivery, or both routes back; at the
// junction the leg onward.
fn moves(state) {
    let route = active_route(state);
    let rates = [];
    if route != () {
        if state.balance("nature", "weather-ready") > 0 { return []; }
        if state.balance("nature", `outcome/${route}/breakdown`) > 0 {
            rates.push(`repair/${route}`);
        } else {
            for outcome in ["clear", "delayed"] {
                if state.balance("nature", `outcome/${route}/${outcome}`) > 0 {
                    rates.push(`arrive/${route}/${outcome}`);
                }
            }
        }
    } else {
        let carrying = carried(state);
        let at = location(state);
        if at == "depot" {
            if carrying != () {
                rates.push("depart/direct-out");
                rates.push("depart/safe-out-first");
            } else {
                for order in orders() {
                    if state.balance(`order/${order}`, "waiting") > 0 { rates.push(`load/${order}`); }
                }
            }
            if state.balance("vehicle", "fuel") < refuel_amount() { rates.push("refuel"); }
        } else if at == "customer" {
            if carrying != () {
                rates.push(`deliver/${carrying}`);
            } else {
                rates.push("depart/direct-back");
                rates.push("depart/safe-back-first");
            }
        } else if at == "junction" {
            rates.push(if carrying != () { "depart/safe-out-second" } else { "depart/safe-back-second" });
        }
    }
    let applicable = [];
    for rate in rates {
        let proposal = action(rate);
        if state.applicable(proposal) { applicable.push(proposal); }
    }
    applicable
}

// What happens in the planned run without the planner choosing: Nature's
// draw while the weather is ready, and the one move the vehicle has when it
// has only one. The planner searches only real choices.
fn nature(state, random) {
    let weighted = weather(state);
    if !weighted.is_empty() { return drawn(state, weighted, random); }
    let forced = moves(state);
    if forced.len() == 1 { return forced[0]; }
    ()
}

// The planner is the only player.
fn dispatcher(state) { 0 }

// How the planner's simulations move: the reliable policy's move where it is
// one of the candidates, and otherwise the first.
fn simulated(state, candidates, random) {
    if candidates.is_empty() { return (); }
    let preferred = decide(state, "reliable");
    if preferred != () {
        for candidate in candidates {
            if candidate == preferred { return candidate; }
        }
    }
    candidates[0]
}

fn all_delivered(state) {
    for order in orders() {
        if state.balance(`order/${order}`, "delivered") == 0 { return false; }
    }
    true
}

fn delivered_count(state) {
    let count = 0;
    for order in orders() {
        if state.balance(`order/${order}`, "delivered") > 0 { count += 1; }
    }
    count
}

// The planner's payoff: a thousand for every order delivered, a hundred for
// each delivery, less the time spent.
fn utility(state) {
    let bonus = if all_delivered(state) { 1000 } else { 0 };
    bonus + delivered_count(state) * 100 - state.balance("vehicle", "elapsed-time").to_int()
}

// The planner's payoff where its game ends, every order delivered, and its
// estimate where a simulation stops.
fn payoff(state) { if all_delivered(state) { [utility(state)] } else { () } }
fn estimated(state) { [utility(state)] }

// Whether every order was delivered, how many were, and the time elapsed.
fn completed(state) { if all_delivered(state) { 1 } else { 0 } }
fn delivered(state) { delivered_count(state) }
fn elapsed(state) { state.balance("vehicle", "elapsed-time") }
