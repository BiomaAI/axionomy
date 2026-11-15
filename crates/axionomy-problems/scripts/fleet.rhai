// Logistics' fleet: a refrigerated van and a freight van serve customers
// north, south and east of a depot, over roads through a hub, one trunk
// road the vans share, and an express road that can break a van down.
// Orders arrive on a clock, cold loads expire after their window and late
// freight earns half. Nature releases and expires orders, resolves each
// trip from the ticket it holds for that van, trip and road, and ends
// refuelling, repairs and tows when their time is up; otherwise the season
// closes when it can, the first van with a decision acts, and else the
// clock ticks.

fn vans() { ["cold", "freight"] }
fn locations() { ["depot", "hub", "north", "south", "east"] }

// The roads, each once, as the world's map gives them: the cold van's trips
// from each place to a later one, with the fuel a trip burns and whether it
// takes the trunk road's one lane.
fn roads(state) {
    let places = locations();
    let roads = [];
    for trip in state.instances("depart") {
        let parts = trip.split("/");
        if parts[1] == "cold" && places.index_of(parts[2]) < places.index_of(parts[3]) {
            let values = state.values(trip);
            roads.push(#{
                from: values.from, to: values.to, fuel: values.fuel,
                trunk: values["trunk-lane"] > 0,
            });
        }
    }
    roads
}

fn value(state, account, asset) { state.balance(account, asset).to_int() }

// Where `account` stands, if anywhere.
fn whereabouts(state, account) {
    for at in locations() {
        if value(state, account, `at/${at}`) > 0 { return at; }
    }
    ()
}

// What an order is: one in three needs the refrigerated slot, one in three
// is three slots of heavy freight, and where it goes.
fn cold(id) { id % 3 == 0 }
fn heavy(id) { id % 3 == 1 }

fn destination(id) {
    if id == 1 || id == 2 { "south" } else if id == 3 || id == 4 { "east" } else { "north" }
}

fn other_van(van) { if van == "cold" { "freight" } else { "cold" } }

// The exchange of `rate` if it applies.
fn applying(state, rate) {
    let proposal = state.exchange_of(rate);
    if state.applicable(proposal) { proposal } else { () }
}

// The first road from `from` toward `to` on a shortest path over `roads`,
// avoiding the express road, which burns extra fuel, when `reliable`;
// nothing at `to` or when no road leads there.
fn first_road(roads, from, to, reliable) {
    if from == to { return (); }
    let queue = [[from, ()]];
    let seen = [from];
    let head = 0;
    while head < queue.len() {
        let at = queue[head][0];
        let first = queue[head][1];
        head += 1;
        for road in roads {
            let next = ();
            if at == road.from {
                next = road.to;
            } else if at == road.to {
                next = road.from;
            } else {
                continue;
            }
            if reliable && road.fuel > 1 { continue; }
            let hop = if first == () { next } else { first };
            if next == to { return hop; }
            if !(next in seen) {
                seen.push(next);
                queue.push([next, hop]);
            }
        }
    }
    ()
}

// What happens without anyone choosing, the first that is due: order by
// order, its arrival or expiry; then van by van, its trip resolved from its
// ticket or arriving, or its service ending.
fn nature_response(state) {
    let orders = state.moves("release") + state.moves("expire");
    for id in 0..value(state, "world", "count") {
        for due in orders {
            if due.account("order") == `order/${id}` { return due; }
        }
    }
    let trips = state.moves("resolve") + state.moves("arrive") + state.moves("end-refuel")
        + state.moves("end-tow") + state.moves("end-repair");
    for van in vans() {
        for due in trips {
            if due.account("vehicle") == `vehicle/${van}` { return due; }
        }
    }
    ()
}

// Order ids by `first`, a preference, then by deadline, then by id.
fn by_deadline(state, ids, cold_first) {
    let keyed = [];
    for id in ids {
        let late = value(state, `order/${id}`, "deadline");
        let preferred = if cold_first { if cold(id) { 0 } else { 1 } } else { if heavy(id) { 0 } else { 1 } };
        keyed.push([preferred, late, id]);
    }
    keyed.sort(|a, b| {
        if a[0] != b[0] { return a[0] - b[0]; }
        if a[1] != b[1] { return a[1] - b[1]; }
        a[2] - b[2]
    });
    let sorted = [];
    for entry in keyed { sorted.push(entry[2]); }
    sorted
}

// What `van` does under `policy`, `fastest`, `reliable` or `relay`: deliver
// what it carries, stage freight at the hub for the other van to collect
// when relaying, hand its cargo over or call the tow truck when damaged or
// out of fuel, repair and refuel at the depot, load waiting orders, drive
// toward a delivery, the hub or the depot, and call the tow truck when worn
// out away from the depot.
fn decision(state, van, policy) {
    let count = value(state, "world", "count");
    if value(state, "world", "resolved") == count { return (); }
    let own = `vehicle/${van}`;
    if value(state, own, "ready") == 0 || value(state, "world", `permit/${van}`) == 0 {
        return ();
    }
    let from = whereabouts(state, own);
    if from == () { return (); }
    let relay = policy == "relay";
    let reliable = policy != "fastest";
    let held = [];
    for id in 0..count {
        if value(state, own, `cargo/${id}`) > 0 { held.push(id); }
    }
    let cargo = by_deadline(state, held, true);
    let carries_cold = false;
    for id in cargo {
        if cold(id) { carries_cold = true; }
    }
    // The first delivery of what it carries that applies, most urgent first,
    // within the window or late: the rates deliver a cold load only on time.
    if !cargo.is_empty() {
        let deliveries = state.moves("deliver");
        for id in cargo {
            for delivery in deliveries {
                if delivery.account("vehicle") == own && delivery.account("order") == `order/${id}` {
                    return delivery;
                }
            }
        }
    }
    if relay && from == "hub" && van == "cold" && carries_cold {
        for id in cargo {
            if cold(id) { continue; }
            let staging = applying(state, `stage/${van}/${id}`);
            if staging != () { return staging; }
        }
    }
    // A working teammate can recover a compatible load from a stranded van.
    if value(state, own, "damage") > 0 || value(state, own, "fuel") == 0 {
        for id in cargo {
            let handoff = applying(state, `transfer/${van}/${other_van(van)}/${id}/${from}`);
            if handoff != () { return handoff; }
        }
        if from != "depot" { return applying(state, `tow/${van}/${from}`); }
    }
    if from == "depot" {
        let damaged = value(state, own, "damage") > 0;
        if damaged || value(state, own, "wear") >= 4 {
            let repair = applying(state, `repair/${van}/${damaged}`);
            if repair != () { return repair; }
        }
        if value(state, own, "fuel") < 4 {
            let refuel = applying(state, `refuel/${van}`);
            if refuel != () { return refuel; }
        }
    }
    // Relay packs the cold van's spare slot for hub staging. Other policies
    // keep one destination per trip. Heavy freight never fits the cold van.
    let may_load = cargo.is_empty()
        || (relay && van == "cold" && from == "depot" && carries_cold);
    if may_load {
        let ready = [];
        for id in 0..count {
            if value(state, `order/${id}`, "waiting") > 0 && value(state, `order/${id}`, `at/${from}`) > 0 {
                ready.push(id);
            }
        }
        for id in by_deadline(state, ready, van == "cold") {
            if relay && from == "hub" && van == "cold" && !cold(id) { continue; }
            let load = applying(state, `load/${van}/${id}/${from}`);
            if load != () { return load; }
        }
    }
    if !cargo.is_empty() {
        let target = if relay && van == "cold" && from == "depot" && cargo.len() > 1 {
            "hub"
        } else {
            destination(cargo[0])
        };
        let next = first_road(roads(state), from, target, reliable);
        if next != () {
            let departure = applying(state, `depart/${van}/${from}/${next}`);
            if departure != () { return departure; }
        }
    } else {
        if relay && van == "freight" {
            let staged = false;
            for id in 0..count {
                if value(state, `order/${id}`, "waiting") > 0
                    && value(state, `order/${id}`, "at/hub") > 0
                    && !cold(id)
                {
                    staged = true;
                }
            }
            if staged {
                let next = first_road(roads(state), from, "hub", reliable);
                if next != () {
                    let departure = applying(state, `depart/${van}/${from}/${next}`);
                    if departure != () { return departure; }
                }
            }
        }
        if from != "depot" {
            let next = first_road(roads(state), from, "depot", reliable);
            if next != () {
                let departure = applying(state, `depart/${van}/${from}/${next}`);
                if departure != () { return departure; }
            }
        }
    }
    // Recovery from exhausted fuel or wear can use the paid shared tow truck.
    if from != "depot" && (value(state, own, "wear") >= 6 || value(state, own, "fuel") == 0) {
        return applying(state, `tow/${van}/${from}`);
    }
    ()
}

// One turn of `policy`: Nature first, then the season's close, then the
// first van with a decision, and otherwise the clock's tick.
fn turn(state, policy) {
    let response = nature_response(state);
    if response != () { return response; }
    let close = applying(state, "close");
    if close != () { return close; }
    for van in vans() {
        let decided = decision(state, van, policy);
        if decided != () { return decided; }
    }
    state.exchange_of("tick")
}

// The fleet's policies, as a strategy calls a policy.
fn fastest(state, random) { turn(state, "fastest") }
fn reliable(state, random) { turn(state, "reliable") }
fn relay(state, random) { turn(state, "relay") }

// What a season's outcome is measured by.
fn delivered(state) { state.balance("world", "delivered-count") }
fn on_time(state) { state.balance("world", "on-time-count") }
fn expired(state) { state.balance("world", "expired-count") }
fn cash(state) { state.balance("world", "money") }
fn spent_fuel(state) {
    state.balance("vehicle/cold", "spent-fuel") + state.balance("vehicle/freight", "spent-fuel")
}
fn elapsed(state) { state.balance("world", "time") }
