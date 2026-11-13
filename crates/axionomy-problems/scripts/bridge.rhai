// Bridge: two travellers and one lane. Micro is one crossing round settled
// directly; Showcase and Stress add a shared clock, arrivals, deadlines,
// reservation leases, occupied crossings and a paid detour. Every policy
// here proposes one exchange a turn; the kernel applies it or refuses it.

// Whether the economy keeps time: timed traffic can advance its clock.
fn timed(state) { "advance/1" in state.rates("advance/") }

// The shared clock, and what an agent, `a` or `b`, holds of `asset`.
fn clock(state) { state.balance("bridge", "now").to_int() }
fn held(state, agent, asset) { state.balance(`agent/${agent}`, asset).to_int() }
fn other(agent) { if agent == "a" { "b" } else { "a" } }

// How many ticks an agent's crossing occupies the lane.
fn duration(agent) { if agent == "a" { 3 } else { 2 } }

// A rate's kind, its first segment, such as `cross` or `submit-bid`.
fn kind(rate) { rate.split("/")[0] }

// `rate` bound to the accounts its roles name.
fn action(rate) {
    let parts = rate.split("/");
    switch parts[0] {
        "resolve" => exchange(rate, #{
            winner: `agent/${parts[1]}`,
            loser: `agent/${other(parts[1])}`,
            bridge: "bridge",
        }, 1),
        "reset-round" | "finish" => exchange(rate, #{
            "agent-a": "agent/a",
            "agent-b": "agent/b",
            bridge: "bridge",
        }, 1),
        "advance" => exchange(rate, #{ bridge: "bridge" }, 1),
        _ => exchange(rate, #{ traveler: `agent/${parts[1]}`, bridge: "bridge" }, 1),
    }
}

// The next time something happens: an arrival, a crossing or detour that
// completes, or an unused reservation that expires. Nothing when none is
// due before the clock runs out.
fn next_time(state) {
    let now = clock(state);
    let next = ();
    for agent in ["a", "b"] {
        let events = [];
        if held(state, agent, "at/west") > 0 {
            events.push(held(state, agent, "arrival"));
        }
        if held(state, agent, "in-transit") + held(state, agent, "on-detour") > 0 {
            events.push(held(state, agent, "ready-at"));
        }
        if held(state, agent, "crossing-right") > 0 && held(state, agent, "at/west") > 0 {
            events.push(held(state, agent, "lease-until") + 1);
        }
        for time in events {
            if time > now && time <= 64 && (next == () || time < next) { next = time; }
        }
    }
    next
}

// Every rate, in order, that applies; timed traffic only advances its clock
// to the next time something happens.
fn candidates(state) {
    let rates = state.rates("");
    if timed(state) {
        let next = next_time(state);
        let due = if next == () { "" } else { `advance/${next}` };
        rates = rates.filter(|rate| !rate.starts_with("advance/") || rate == due);
    }
    rates.map(|rate| action(rate)).filter(|proposal| state.applicable(proposal))
}

// What plain search routes over: in timed traffic, a ready crossing,
// completion, round reset or finish alone, and otherwise every candidate but
// bids, withdrawals, auctions and deliberately expired reservations.
fn routes(state) {
    let choices = candidates(state);
    if !timed(state) { return choices; }
    for choice in choices {
        if ["complete", "reset-round", "finish", "cross"].contains(kind(choice.rate())) {
            return [choice];
        }
    }
    choices.filter(|choice| {
        !["submit-bid", "withdraw-bid", "resolve", "expire-reservation"].contains(kind(choice.rate()))
    })
}

// Fixed priority: `first` claims the first turn, and whoever is left takes
// the second.
fn priority_claim(state, choices, first) {
    let first_turn = state.balance("bridge", "first-turn") > 0;
    for choice in choices {
        let parts = choice.rate().split("/");
        if (parts[0] == "claim-first" && parts[1] == first && first_turn)
            || parts[0] == "claim-second" || parts[0] == "yield-to-waiting" {
            return choice;
        }
    }
    ()
}

// What an agent bids. Micro's one round settles A's 2 against B's 1; timed
// traffic bids 2 when the deadline leaves two ticks of slack or fewer, else
// 1, never more than the agent's credit.
fn bid(state, agent) {
    if !timed(state) { return if agent == "a" { 2 } else { 1 }; }
    let slack = held(state, agent, "deadline") - (clock(state) + duration(agent));
    let amount = if slack <= 2 { 2 } else { 1 };
    let credit = held(state, agent, "credit");
    if credit < amount { credit } else { amount }
}

// The auction: once both travellers have arrived, each bids for the first
// turn, A first; with no bid left to make, the first claim that applies.
// After the first turn, the loser or whoever is left takes the second.
fn auction_claim(state, choices) {
    if state.balance("bridge", "first-turn") > 0 {
        let now = clock(state);
        if now < held(state, "a", "arrival") || now < held(state, "b", "arrival") { return (); }
        for agent in ["a", "b"] {
            let offer = action(`submit-bid/${agent}/${bid(state, agent)}`);
            if state.applicable(offer) { return offer; }
        }
        for choice in choices {
            if kind(choice.rate()) == "claim-first" { return choice; }
        }
        return ();
    }
    for choice in choices {
        if ["yield-to-waiting", "claim-second"].contains(kind(choice.rate())) { return choice; }
    }
    ()
}

// Deadline-responsive routing: pay for the detour when the lane stays
// occupied past a deadline the detour still meets; otherwise the claim of
// the traveller whose deadline comes first, the earlier offered on a tie.
fn adaptive_claim(state, choices) {
    let now = clock(state);
    for choice in choices {
        let parts = choice.rate().split("/");
        if parts[0] != "detour" { continue; }
        let agent = parts[1];
        let deadline = held(state, agent, "deadline");
        let occupied = held(state, other(agent), "ready-at");
        if occupied < now { occupied = now; }
        if now + 3 <= deadline && occupied + duration(agent) > deadline { return choice; }
    }
    let earliest = ();
    let chosen = ();
    for choice in choices {
        let parts = choice.rate().split("/");
        if !["claim-first", "claim-second", "yield-to-waiting"].contains(parts[0]) { continue; }
        let deadline = held(state, parts[1], "deadline");
        if earliest == () || deadline < earliest {
            earliest = deadline;
            chosen = choice;
        }
    }
    chosen
}

// One turn of a policy: whatever is ready (a completion, round reset,
// finish, crossing or auction settlement) first, then the policy's own
// choice, then the clock's advance; nothing when none applies.
fn turn(state, policy) {
    let choices = candidates(state);
    for choice in choices {
        if ["complete", "reset-round", "finish", "cross", "resolve"].contains(kind(choice.rate())) {
            return choice;
        }
    }
    let chosen = switch policy {
        "first-come-a" => priority_claim(state, choices, "a"),
        "first-come-b" => priority_claim(state, choices, "b"),
        "auction" => auction_claim(state, choices),
        "adaptive" => adaptive_claim(state, choices),
    };
    if chosen != () { return chosen; }
    for choice in choices {
        if kind(choice.rate()) == "advance" { return choice; }
    }
    ()
}

// The policies an experiment compares, turn by turn.
fn first_come_a(state, random) { turn(state, "first-come-a") }
fn first_come_b(state, random) { turn(state, "first-come-b") }
fn auction(state, random) { turn(state, "auction") }
fn adaptive(state, random) { turn(state, "adaptive") }

// What each traveller keeps, as the objectives measure it: priority won,
// credit retained, trips completed on time; and the time traffic took.
fn priority_a(state) { state.balance("agent/a", "priority-benefit") }
fn priority_b(state) { state.balance("agent/b", "priority-benefit") }
fn credit_a(state) { state.balance("agent/a", "credit") }
fn credit_b(state) { state.balance("agent/b", "credit") }
fn on_time_a(state) { state.balance("agent/a", "on-time") }
fn on_time_b(state) { state.balance("agent/b", "on-time") }
fn elapsed(state) { state.balance("bridge", "now") }
