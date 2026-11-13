// Job-shop scheduling: which operation to schedule, queue, start, finish or
// resume next. The micro instance plans whole slots ahead; the showcase and
// stress instances execute the shop tick by tick, with queues, setups, shared
// labor and a machine that breaks down.

// The operations, in order.
fn operations() { ["one-a", "one-b", "one-c", "two-a", "two-b", "two-c"] }

// An operation's job, the machine a planned schedule runs it on, and how many
// slots it takes there.
fn spec(operation) {
    switch operation {
        "one-a" => #{ job: "one", machine: "one", duration: 2 },
        "one-b" => #{ job: "one", machine: "two", duration: 1 },
        "one-c" => #{ job: "one", machine: "three", duration: 1 },
        "two-a" => #{ job: "two", machine: "one", duration: 2 },
        "two-b" => #{ job: "two", machine: "two", duration: 1 },
        "two-c" => #{ job: "two", machine: "three", duration: 1 },
    }
}

// The machines an executed operation may run on, in the order the shop
// considers them.
fn alternatives(operation) {
    switch operation {
        "one-a" | "two-a" => ["one", "two"],
        "one-b" | "two-b" => ["two", "three"],
        "one-c" | "two-c" => ["three", "one"],
    }
}

// Whether the shop executes its schedule, rather than planning slots: only an
// executed shop closes.
fn executing(state) { !state.rates("close").is_empty() }

// The exchange of `rate`, bound to the schedule and to the jobs, slots and
// machine the rate names.
fn action(rate) {
    let parts = rate.split("/");
    let bindings = #{ schedule: "success" };
    switch parts[0] {
        "schedule" => {
            let operation = spec(parts[1]);
            let start = parse_int(parts[3]);
            bindings["primary-job"] = `job/${operation.job}`;
            bindings.slot0 = `slot/${operation.machine}/${start}`;
            if operation.duration == 2 {
                bindings.slot1 = `slot/${operation.machine}/${start + 1}`;
            }
        }
        "finish" | "close" => {
            bindings["primary-job"] = "job/one";
            bindings["secondary-job"] = "job/two";
        }
        "queue" | "cancel-queue" | "start" | "complete" | "resume" => {
            bindings["primary-job"] = `job/${spec(parts[1]).job}`;
            bindings.machine = `machine/${parts[2]}`;
        }
        "fault" => {
            bindings.machine = "machine/two";
            if parts[1] == "some" {
                bindings["primary-job"] = `job/${spec(parts[2]).job}`;
            }
        }
        "repair" | "complete-repair" => {
            bindings.machine = "machine/two";
        }
    }
    exchange(rate, bindings, 1)
}

// What may happen next. A planned shop tries every rate that applies, in
// order; an executed one decides as `dispatch` does.
fn candidates(state) {
    if executing(state) { return dispatch(state); }
    let rates = state.rates("");
    rates.map(|rate| action(rate)).filter(|proposal| state.applicable(proposal))
}

// An executed shop's next events, each kind in the economy's order of rates.
// A completion, a finished repair or closing the day comes first, alone,
// since it commutes with dispatching; then a disruption that is due.
// Queueing costs neither time nor work, so an operation waits to be queued
// until it can start, and then starts at once: of the queued operations that
// can start, the last in operation and alternative order is the only choice.
// Otherwise advancing the clock to the next arrival, completion, repair or
// disruption, queueing, repairing and resuming, whichever apply.
fn dispatch(state) {
    let schedule = state.balances("success");
    if !("active" in schedule) { return []; }
    let now = state.balance("success", "now");
    let times = [];
    for job in ["one", "two"] {
        times.push(state.balance(`job/${job}`, "arrival"));
        times.push(state.balance(`job/${job}`, "due"));
    }
    times.push(state.balance("machine/two", "due"));
    let fault_pending = "fault-pending" in schedule;
    if fault_pending { times.push(state.balance("success", "fault-at")); }
    let upcoming = ();
    for time in times {
        if time > now && (upcoming == () || time < upcoming) { upcoming = time; }
    }

    let machines = #{
        one: state.balances("machine/one"),
        two: state.balances("machine/two"),
        three: state.balances("machine/three"),
    };
    let labor = "labor" in schedule;
    // The rates of a kind, keyed by their paths, which a map keeps in order.
    let completions = #{};
    let faults = #{};
    let queues = #{};
    let resumptions = #{};
    let queued_start = ();
    let queued_rank = -1;
    // Where each job's operations stand, read from the job's tokens.
    for job in ["one", "two"] {
        for token in state.balances(`job/${job}`).keys() {
            let parts = token.split("/");
            switch parts[0] {
                "ready" => {
                    let operation = parts[1];
                    for machine in alternatives(operation) {
                        let held = machines[machine];
                        if labor && "idle" in held && "online" in held {
                            queues[`queue/${operation}/${machine}`] = ();
                        }
                    }
                }
                "queued" => {
                    let operation = parts[1];
                    let machine = parts[2];
                    let configured = if "configured/one" in machines[machine] { "one" } else { "two" };
                    let start = `start/${operation}/${machine}/${configured}`;
                    let rank = operations().index_of(operation) * 2
                        + alternatives(operation).index_of(machine);
                    if rank > queued_rank && state.applicable(action(start)) {
                        queued_start = start;
                        queued_rank = rank;
                    }
                }
                "running" => {
                    completions[`complete/${parts[1]}/${parts[2]}`] = ();
                    if parts[2] == "two" { faults[`fault/some/${parts[1]}`] = (); }
                }
                "paused" => {
                    resumptions[`resume/${parts[1]}/${parts[2]}`] = ();
                }
            }
        }
    }

    let first = ["close"] + completions.keys();
    if "repairing" in machines.two { first.push("complete-repair"); }
    for rate in first {
        let proposal = action(rate);
        if state.applicable(proposal) { return [proposal]; }
    }
    if fault_pending && now == state.balance("success", "fault-at") {
        for rate in ["fault/none"] + faults.keys() {
            let proposal = action(rate);
            if state.applicable(proposal) { return [proposal]; }
        }
    }
    if queued_start != () { return [action(queued_start)]; }
    let rest = [];
    if upcoming != () { rest.push(`advance/${upcoming.to_int()}`); }
    rest += queues.keys();
    if "broken" in machines.two { rest.push("repair"); }
    rest += resumptions.keys();
    rest.map(|rate| action(rate)).filter(|proposal| state.applicable(proposal))
}

// When the schedule finishes: the clock of an executed shop, the makespan a
// planned one records when it finishes, and nothing before.
fn makespan(state) {
    if executing(state) { return state.balance("success", "now"); }
    let schedule = state.balances("success");
    for time in 0..=16 {
        if `makespan/${time}` in schedule { return time; }
    }
    0
}

// When `job` completes: as an executed shop records it, or when a planned
// schedule completes the job's last operation; nothing before.
fn completion(state, job) {
    let account = `job/${job}`;
    if executing(state) { return state.balance(account, "completion-time"); }
    let held = state.balances(account);
    for operation in [`${job}-c`, `${job}-b`] {
        for time in 0..=16 {
            if `completed-at/${operation}/${time}` in held { return time; }
        }
    }
    0
}

fn job_one(state) { completion(state, "one") }
fn job_two(state) { completion(state, "two") }
