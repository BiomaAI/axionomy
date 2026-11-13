// The Autonomous Work League: workers contend for a finite pool of jobs.
// Each worker holds its policy, so a lineup is an economy of its own and
// these functions play whichever field its workers hold.
//
// Micro is the compact serial fixture: each worker in turn claims the open
// job its policy prefers and works it until it succeeds, repairing after
// each failure and retrying safely, and a steward then recycles a unit of
// its waste. Nature's outcome for each attempt is the ticket keyed by the
// job, the worker, the mode and the attempt, a counter the economy does not
// hold, so `serial` plans the match whole.
//
// Showcase and Stress are seasons on a shared clock, played turn by turn by
// `season`: the events that are due, contract by contract (an expiry,
// Nature settling an attempt from the ticket keyed by the job and its
// attempt, a claim whose lease lapsed), then services ending, then the
// season's end once it may come; otherwise each worker with a decision left
// decides, the first one rotating with the clock, and then the clock ticks.

fn workers() { ["atlas", "bolt", "coda", "delta"] }
fn policies() { ["sprinter", "steward", "value-hunter", "resilient"] }
fn modes() { ["rush", "lean", "safe"] }
fn facilities() { ["workshop", "charger", "recycler"] }
fn locations() { ["depot", "north", "east", "south", "west", "workshop", "charger", "recycler"] }

// The places a season's roads lead from `at`, in the order of locations:
// the depot's spokes, and the workshop, charger and recycler beyond them.
fn neighbours(at) {
    switch at {
        "depot" => ["north", "east", "south", "west"],
        "north" => ["depot", "recycler"],
        "east" => ["depot", "charger"],
        "south" => ["depot", "workshop", "charger"],
        "west" => ["depot", "workshop"],
        "workshop" => ["south", "west"],
        "charger" => ["east", "south"],
        _ => ["north"],
    }
}

fn smaller(a, b) { if b < a { b } else { a } }
fn larger(a, b) { if b > a { b } else { a } }

// Whether `a` comes before `b`, compared element by element.
fn before(a, b) {
    for index in 0..a.len() {
        if a[index] < b[index] { return true; }
        if a[index] > b[index] { return false; }
    }
    false
}

// The workers the economy holds, in the league's order.
fn competitors(state) {
    let found = [];
    for worker in workers() {
        if state.has_account(`agent/${worker}`) { found.push(worker); }
    }
    found
}

// What `balances`, an account's, holds of `asset`.
fn held(balances, asset) {
    let quantity = balances[asset];
    if quantity == () { 0 } else { quantity.to_int() }
}

// The first of `names` that `balances` holds under `kind`.
fn first_held(balances, kind, names) {
    for name in names {
        if held(balances, `${kind}/${name}`) > 0 { return name; }
    }
    ()
}

// What a job is: where, what it pays and what it costs, and its risk.
fn job_spec(id) {
    let index = id - 1;
    #{
        id: id,
        location: ["north", "east", "south", "west"][index % 4],
        value: 8 + index * 7 % 17,
        energy: 3 + index * 3 % 5,
        time: 3 + index * 3 % 6,
        material: 2 + index * 2 % 4,
        risk: 1 + index * 3 % 4,
    }
}

// What working a job in `mode` costs and leaves as waste.
fn work_cost(spec, mode) {
    switch mode {
        "rush" => #{
            energy: spec.energy + 2,
            time: larger(spec.time - 1, 1),
            material: spec.material + 1,
            waste: 2,
        },
        "lean" => #{
            energy: spec.energy,
            time: spec.time + 2,
            material: spec.material,
            waste: spec.id % 2,
        },
        _ => #{
            energy: spec.energy + 1,
            time: spec.time + 1,
            material: spec.material,
            waste: 1,
        },
    }
}

// The chances in ten that working a job of `risk` in `mode` fails.
fn failure_weight(risk, mode) {
    switch mode {
        "rush" => smaller(risk + 2, 8),
        "lean" => smaller(risk, 7),
        _ => smaller(larger(risk - 1, 0), 4),
    }
}

// The exchange of `rate`, once, with the accounts its roles bind.
fn once(rate, bindings) { exchange(rate, bindings, 1) }

// `proposal`, or `otherwise` when there is none.
fn or_else(proposal, otherwise) { if proposal == () { otherwise } else { proposal } }

// --- Micro: the serial match ------------------------------------------------

fn first_mode(policy) {
    switch policy {
        "steward" => "lean",
        "resilient" => "safe",
        _ => "rush",
    }
}

// The job of `open`, in job order, that `policy` takes first.
fn chosen_job(open, policy) {
    let best = ();
    let best_key = ();
    for id in open {
        let spec = job_spec(id);
        let key = switch policy {
            "sprinter" => [spec.time, -spec.value, spec.risk],
            "steward" => [spec.material, spec.energy, spec.risk],
            "value-hunter" => [-spec.value, spec.risk, spec.time],
            _ => [spec.risk, spec.energy, -spec.value],
        };
        if best == () || before(key, best_key) {
            best = id;
            best_key = key;
        }
    }
    best
}

// Nature's outcome for `worker`'s `attempt` at a job in `mode`.
fn serial_outcome(random, worker, spec, mode, attempt) {
    let key = (spec.id << 17) ^ (workers().index_of(worker) << 9)
        ^ (modes().index_of(mode) << 5) ^ attempt;
    if random.keyed(key, 10) < failure_weight(spec.risk, mode) { "failure" } else { "success" }
}

// `worker`'s walk from `from` to `to`: one move, or none when there.
fn walk(worker, from, to) {
    if from == to { return []; }
    [once(`move/${worker}/${from}/${to}`, #{ agent: `agent/${worker}` })]
}

// The serial match whole: turn by turn until no job is left to claim.
fn serial(state, random) {
    let crew = competitors(state);
    let open = [];
    for id in 1..=state.accounts("job/").len() { open.push(id); }
    // What the plan follows of each worker: its policy, where it is and
    // the waste it holds.
    let policy = #{};
    let at = #{};
    let waste = #{};
    for worker in crew {
        let balances = state.balances(`agent/${worker}`);
        policy[worker] = first_held(balances, "policy", policies());
        at[worker] = first_held(balances, "at", locations());
        waste[worker] = held(balances, "waste");
    }
    let plan = [];
    let turn = 0;
    while open.len() > 0 {
        let worker = crew[turn % crew.len()];
        let agent = `agent/${worker}`;
        let id = chosen_job(open, policy[worker]);
        let still_open = [];
        for other in open {
            if other != id { still_open.push(other); }
        }
        open = still_open;
        let spec = job_spec(id);
        let job = `job/${id}`;
        plan.push(once(`claim/${worker}/${id}`, #{ agent: agent, job: job }));
        plan += walk(worker, at[worker], spec.location);
        at[worker] = spec.location;
        let mode = first_mode(policy[worker]);
        let attempt = 0;
        loop {
            attempt += 1;
            plan.push(once(`begin/${worker}/${id}/${mode}`, #{ agent: agent, job: job }));
            waste[worker] += work_cost(spec, mode).waste;
            let outcome = serial_outcome(random, worker, spec, mode, attempt);
            plan.push(once(`resolve/${worker}/${id}/${mode}/${outcome}`, #{ agent: agent, nature: "nature" }));
            plan.push(once(`finish/${worker}/${id}/${mode}/${outcome}`, #{ agent: agent, job: job }));
            if outcome == "success" { break; }
            plan += walk(worker, spec.location, "workshop");
            plan.push(once(`repair/${worker}`, #{ agent: agent, facility: "facility/workshop" }));
            plan += walk(worker, "workshop", spec.location);
            mode = "safe";
        }
        if policy[worker] == "steward" && waste[worker] > 0 {
            plan += walk(worker, at[worker], "recycler");
            at[worker] = "recycler";
            plan.push(once(`recycle/${worker}`, #{ agent: agent, facility: "facility/recycler" }));
            waste[worker] -= 1;
        }
        turn += 1;
    }
    plan
}

// --- Showcase and Stress: the season ------------------------------------------

fn preferred_modes(policy) {
    switch policy {
        "steward" => ["lean", "safe", "rush"],
        "resilient" => ["safe", "lean", "rush"],
        _ => ["rush", "safe", "lean"],
    }
}

// The first step and the number of moves on a shortest road from `from` to
// each place, found breadth first in the order of locations.
fn routes_from(from) {
    let found = #{};
    found[from] = [from, 0];
    let queue = [from];
    let head = 0;
    while head < queue.len() {
        let at = queue[head];
        head += 1;
        let first = found[at][0];
        let distance = found[at][1];
        for next in neighbours(at) {
            if !found.contains(next) {
                found[next] = [if distance == 0 { next } else { first }, distance + 1];
                queue.push(next);
            }
        }
    }
    found
}

// What a decision reads of `worker`.
fn worker_in(state, worker) {
    let account = `agent/${worker}`;
    let balances = state.balances(account);
    let at = first_held(balances, "at", locations());
    #{
        name: worker,
        account: account,
        balances: balances,
        policy: first_held(balances, "policy", policies()),
        at: at,
        routes: routes_from(at),
        energy: held(balances, "energy"),
        material: held(balances, "material"),
    }
}

// Whether `who` can still reach a job in `mode` and finish it by its
// deadline, `overhead` ticks from now, with the material and energy it holds.
fn can_finish(who, spec, mode, clock, deadline, overhead) {
    let cost = work_cost(spec, mode);
    clock + overhead + cost.time <= deadline
        && who.material >= cost.material
        && who.energy > cost.energy + who.routes[spec.location][1]
}

// `who`'s first move toward `to`, if it can make one.
fn travel(state, who, to) {
    let first = who.routes[to][0];
    if first == who.at { return (); }
    let move = once(`move/${who.name}/${who.at}/${first}`, #{ world: "world", agent: who.account });
    if state.applicable(move) { move } else { () }
}

// `who` heading for `facility`, or starting its service there.
fn service(state, who, facility) {
    if who.at != facility { return travel(state, who, facility); }
    let maintenance = facility == "workshop" && held(who.balances, "damage") == 0;
    let begin = once(
        `begin-service/${who.name}/${facility}/${maintenance}`,
        #{ world: "world", agent: who.account, facility: `facility/${facility}` },
    );
    if state.applicable(begin) { begin } else { () }
}

// Nature settling the `attempt`th attempt at job `id` in `mode`: the
// ticket keyed by the job and the attempt, against Nature's weights.
fn settled(state, random, id, mode, attempt) {
    let failure = state.balance("nature", `outcome-weight/${id}/${mode}/failure`).to_int();
    let success = state.balance("nature", `outcome-weight/${id}/${mode}/success`).to_int();
    if random.keyed((id << 32) ^ attempt, failure + success) < failure { "failure" } else { "success" }
}

// The event due for job `id`, if any: its expiry once its deadline has
// passed, the end of the attempt at it once that is due, or the release of
// a claim whose lease has lapsed.
fn due(state, random, clock, id) {
    let job = `job/${id}`;
    let contract = state.balances(job);
    if held(contract, "completed") > 0 || held(contract, "expired") > 0 { return (); }
    let owner = first_held(contract, "assigned", workers());
    let mode = ();
    let owned = #{};
    if owner != () {
        owned = state.balances(`agent/${owner}`);
        mode = first_held(owned, `awaiting/${id}`, modes());
    }
    if clock > held(contract, "deadline") {
        let bindings = #{ world: "world", job: job };
        let carrier = "none";
        if owner != () {
            bindings.agent = `agent/${owner}`;
            carrier = `some/${owner}`;
        }
        let activity = if mode == () { "none" } else { `some/${mode}` };
        return once(`expire/${id}/${carrier}/${activity}`, bindings);
    }
    if owner != () && mode != () {
        if clock < held(owned, "ready-at") { return (); }
        let outcome = settled(state, random, id, mode, held(contract, "attempts"));
        return once(
            `finish/${owner}/${id}/${mode}/${outcome}`,
            #{ world: "world", agent: `agent/${owner}`, job: job, nature: "nature" },
        );
    }
    if owner != () {
        let release = once(
            `release-claim/${owner}/${id}/true`,
            #{ world: "world", agent: `agent/${owner}`, job: job },
        );
        if state.applicable(release) { return release; }
    }
    ()
}

// What `worker`, holding a decision, decides.
fn decision(state, worker, clock, count, world) {
    let me = worker_in(state, worker);
    let wait = once(`wait/${worker}`, #{ world: "world", agent: me.account });
    if held(me.balances, "busy") > 0 { return wait; }
    let policy = me.policy;
    let claimed = ();
    for id in 1..=count {
        if held(me.balances, `claimed/${id}`) > 0 {
            claimed = id;
            break;
        }
    }
    // Damaged or worn: hand the claim to a worker who can finish it, or go
    // for repair.
    let wear_limit = if policy == "resilient" { 4 } else { 6 };
    if held(me.balances, "damage") > 0 || held(me.balances, "wear") >= wear_limit {
        if claimed != () {
            let spec = job_spec(claimed);
            let deadline = state.balance(`job/${claimed}`, "deadline").to_int();
            for recipient in workers() {
                if recipient == worker { continue; }
                let handoff = once(
                    `handoff/${worker}/${recipient}/${claimed}`,
                    #{ world: "world", agent: me.account, partner: `agent/${recipient}`, job: `job/${claimed}` },
                );
                if !state.applicable(handoff) { continue; }
                let other = worker_in(state, recipient);
                let overhead = 1 + other.routes[spec.location][1];
                for mode in preferred_modes(other.policy) {
                    if can_finish(other, spec, mode, clock, deadline, overhead) { return handoff; }
                }
            }
        }
        return or_else(service(state, me, "workshop"), wait);
    }
    // Holding a claim: release an impossible promise, recharge or reclaim
    // material for it, head for it, or begin it.
    if claimed != () {
        let spec = job_spec(claimed);
        let job = `job/${claimed}`;
        let deadline = state.balance(job, "deadline").to_int();
        let distance = me.routes[spec.location][1];
        let preferred = preferred_modes(policy);
        let release = once(
            `release-claim/${worker}/${claimed}/false`,
            #{ world: "world", agent: me.account, job: job },
        );
        let possible = false;
        let minimum_energy = ();
        let minimum_material = ();
        for mode in preferred {
            let cost = work_cost(spec, mode);
            if clock + distance + cost.time <= deadline { possible = true; }
            minimum_energy = if minimum_energy == () { cost.energy } else { smaller(minimum_energy, cost.energy) };
            minimum_material = if minimum_material == () { cost.material } else { smaller(minimum_material, cost.material) };
        }
        if !possible { return release; }
        if me.energy < minimum_energy + distance + 1 {
            return or_else(service(state, me, "charger"), wait);
        }
        if me.material < minimum_material {
            return or_else(service(state, me, "recycler"), release);
        }
        let heading = travel(state, me, spec.location);
        if heading != () { return heading; }
        for mode in preferred {
            let begin = once(`begin/${worker}/${claimed}/${mode}`, #{ world: "world", agent: me.account, job: job });
            if state.applicable(begin) { return begin; }
        }
        return wait;
    }
    if held(world, "contracts-open") == 0 { return wait; }
    if me.energy < 9 { return or_else(service(state, me, "charger"), wait); }
    if held(me.balances, "waste") >= 2 && (me.material < 5 || policy == "steward") {
        return or_else(service(state, me, "recycler"), wait);
    }
    // Claim the open contract the policy scores best that it can finish.
    let best = ();
    let best_score = ();
    for id in 1..=count {
        let contract = state.balances(`job/${id}`);
        // What a claim needs: an open contract that has arrived.
        if held(contract, "available") == 0 || clock < held(contract, "release-at") { continue; }
        let spec = job_spec(id);
        let deadline = held(contract, "deadline");
        let distance = me.routes[spec.location][1];
        let mode = ();
        for each in preferred_modes(policy) {
            if can_finish(me, spec, each, clock, deadline, 1 + distance) {
                mode = each;
                break;
            }
        }
        if mode == () { continue; }
        let claim = once(`claim/${worker}/${id}`, #{ world: "world", agent: me.account, job: `job/${id}` });
        if !state.applicable(claim) { continue; }
        let cost = work_cost(spec, mode);
        let urgency = deadline - clock;
        let score = switch policy {
            "sprinter" => [cost.time + distance, urgency, -spec.value],
            "value-hunter" => [-(spec.value * 100 / (cost.time + distance + 1)), urgency, spec.risk],
            "steward" => [cost.material + cost.waste, distance, urgency],
            _ => [failure_weight(spec.risk, mode), urgency, distance],
        };
        if best == () || before(score, best_score) {
            best = claim;
            best_score = score;
        }
    }
    or_else(best, wait)
}

// The season's next exchange.
fn season(state, random) {
    let clock = state.balance("world", "tick").to_int();
    let count = state.accounts("job/").len();
    for id in 1..=count {
        let event = due(state, random, clock, id);
        if event != () { return event; }
    }
    for worker in workers() {
        let balances = state.balances(`agent/${worker}`);
        for facility in facilities() {
            if held(balances, `servicing/${facility}`) == 0 { continue; }
            let ending = once(
                `end-service/${worker}/${facility}`,
                #{ world: "world", agent: `agent/${worker}`, facility: `facility/${facility}` },
            );
            if state.applicable(ending) { return ending; }
        }
    }
    let world = state.balances("world");
    if held(world, "contracts-open") == 0 && held(world, "activities") == 0 {
        let end = once("end-season", #{ world: "world" });
        if state.applicable(end) { return end; }
    }
    let first = clock % 4;
    for offset in 0..4 {
        let worker = workers()[(first + offset) % 4];
        if held(world, `permit/${worker}`) > 0 {
            return decision(state, worker, clock, count, world);
        }
    }
    let bindings = #{ world: "world" };
    for worker in workers() { bindings[`worker/${worker}`] = `agent/${worker}`; }
    once("tick", bindings)
}

// --- Objectives -------------------------------------------------------------

// What every worker holds of `asset`.
fn total(state, asset) {
    let sum = quantity(0);
    for account in state.accounts("agent/") { sum = sum + state.balance(account, asset); }
    sum
}

fn contract_value(state) { total(state, "value") }
fn completed(state) { total(state, "completed") }
fn residual_waste(state) { total(state, "waste") }
