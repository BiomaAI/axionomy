// Scheduling: how people read the job shop. Labels and formats name its
// identifiers; the scene draws the machines' timeline: the slots each
// operation reserves and the queue still waiting, or, on the executed shop,
// the work done and underway, each operation's state, the crew, Machine
// Two's condition and each job's completion. The operations and the
// makespan are the shop's economy script's.

import "scheduling" as scheduling;

// A name as people read it: `one` is `One`.
fn words(name) {
    let parts = [];
    for part in name.split("-") {
        parts.push(part.sub_string(0, 1).to_upper() + part.sub_string(1));
    }
    parts.reduce(|sum, part| if sum == () { part } else { sum + " " + part })
}

fn machines() {
    ["one", "two", "three"]
}

fn job_name(job) {
    `Job ${words(job)}`
}

// The job an operation belongs to: `one-b` is Job One's.
fn job_of(operation) {
    operation.split("-")[0]
}

// An operation as its job's step: `Job One step A`.
fn operation_name(operation) {
    `${job_name(job_of(operation))} step ${operation.split("-")[1].to_upper()}`
}

fn machine_name(machine) {
    `Machine ${words(machine)}`
}

fn asset_label(name, parts) {
    switch name {
        "job-identity" => `This is ${job_name(parts[2])}`,
        "slot-identity" => `This is ${machine_name(parts[2])} at slot ${parts[3]}`,
        "ready-at" => `${operation_name(parts[2])} ready at slot ${parts[3]}`,
        "completed-at" => `${operation_name(parts[2])} completed at slot ${parts[3]}`,
        "available" => "Machine slot available",
        "reserved" => `Reserved for ${operation_name(parts[2])}`,
        "makespan" => `Overall finish at slot ${parts[2]}`,
        "active" => "Schedule in progress",
        "solved" => "Schedule complete",
        "machine-identity" => `This is ${machine_name(parts[2])}`,
        "ready" => `${operation_name(parts[2])} ready for routing`,
        "queued" => `${operation_name(parts[2])} queued on ${machine_name(parts[3])}`,
        "running" => `${operation_name(parts[2])} running on ${machine_name(parts[3])}`,
        "paused" => `${operation_name(parts[2])} paused on ${machine_name(parts[3])}`,
        "queue-free" => "Queue space",
        "queue-ticket" => `Queue reservation for ${operation_name(parts[2])}`,
        "idle" => "Machine idle",
        "busy" => "Machine occupied",
        "online" => "Machine online",
        "broken" => "Machine broken",
        "repairing" => "Repair underway",
        "configured" => `Set up for ${job_name(parts[2])}`,
        "labor" => "Available crew",
        "busy-labor" => "Occupied crew",
        "repair-kit" => "Repair kit",
        "used-repair-kit" => "Repair kit used",
        "arrival" => "Job release time",
        "due" => "Activity completes at",
        "work-remaining" => "Interrupted work remaining",
        "done" => "Job completed",
        "completion-time" => "Actual completion time",
        "now" => "World time",
        "time-remaining" => "Workday remaining",
        "fault-at" => "Scheduled disruption time",
        "fault-pending" => "Disruption pending",
        "fault-occurred" => "Disruption occurred",
        "repair-duration" => "Repair duration",
        "not-started" => `${operation_name(parts[2])} has not started`,
        "started" => `${operation_name(parts[2])} started`,
        "start-time" => `${operation_name(parts[2])} start time`,
        "end-time" => `${operation_name(parts[2])} completion time`,
        "executed-on" => `${operation_name(parts[2])} uses ${machine_name(parts[3])}`,
        "interrupted" => `${operation_name(parts[2])} was interrupted`,
        _ => (),
    }
}

fn rate_label(name, parts) {
    switch name {
        "schedule" => `Run ${operation_name(parts[2])} from slot ${parts[4]}, ready at slot ${parts[3]}`,
        "finish" => `Finish with Job One done at slot ${parts[2]}, Job Two at slot ${parts[3]}, makespan ${parts[4]}`,
        "queue" => `Queue ${operation_name(parts[2])} on ${machine_name(parts[3])}`,
        "cancel-queue" => `Withdraw ${operation_name(parts[2])} from the ${machine_name(parts[3])} queue`,
        "start" => if parts[4] == job_of(parts[2]) {
            `Start ${operation_name(parts[2])} on ${machine_name(parts[3])}, already set up`
        } else {
            `Start ${operation_name(parts[2])} on ${machine_name(parts[3])}, switching setup from ${job_name(parts[4])}`
        },
        "complete" => `Complete ${operation_name(parts[2])} on ${machine_name(parts[3])}`,
        "resume" => `Resume ${operation_name(parts[2])} on ${machine_name(parts[3])}`,
        "fault" => if parts[2] == "some" {
            `Machine Two fails during ${operation_name(parts[3])}`
        } else {
            "Machine Two fails"
        },
        "repair" => "Begin Machine Two repair",
        "complete-repair" => "Complete Machine Two repair",
        "advance" => `Advance the workday to tick ${parts[2]}`,
        "close" => "Close the completed workday",
        _ => (),
    }
}

fn role_label(name) {
    switch name {
        "primary-job" => "Job",
        "secondary-job" => "Second job",
        "slot0" => "First time slot",
        "slot1" => "Second time slot",
        "schedule" => "Schedule",
        "machine" => "Machine",
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "job" => job_name(parts[2]),
            "slot" => `${machine_name(parts[2])} at slot ${parts[3]}`,
            "machine" => machine_name(parts[2]),
            _ => "Schedule",
        },
        "asset" => asset_label(parts[1], parts),
        "rate" => rate_label(parts[1], parts),
        _ => role_label(parts[1]),
    }
}

fn format(asset) {
    switch asset.split("/")[0] {
        "arrival" | "due" | "work-remaining" | "completion-time" | "now" | "time-remaining"
            | "fault-at" | "repair-duration" | "start-time" | "end-time" => #{ scale: 0, unit: "ticks" },
        _ => (),
    }
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

fn scene(state, frame) {
    if scheduling::executing(state) {
        return execution_scene(state);
    }
    let lanes = machines().map(|machine| #{
        id: #{ key: `machine/${machine}`, label: machine_name(machine) },
        classes: [],
    });
    let spans = [];
    let scheduled = [];
    for machine in machines() {
        for operation in scheduling::operations() {
            let first = ();
            let last = ();
            for time in 0..12 {
                if !state.balance(`slot/${machine}/${time}`, `reserved/${operation}`).is_zero() {
                    if first == () {
                        first = time;
                    }
                    last = time;
                }
            }
            if first != () {
                spans.push(#{
                    id: `${machine}/${operation}`,
                    lane: `machine/${machine}`,
                    start: first,
                    end: last + 1,
                    label: operation_name(operation),
                    classes: [`job/${job_of(operation)}`],
                });
                scheduled.push(operation);
            }
        }
    }
    let pending = scheduling::operations().filter(|operation| !scheduled.contains(operation));
    let entities = pending.map(|operation| #{
        id: #{ key: `pending/${operation}`, label: operation_name(operation) },
        glyph: "task",
        anchor: #{ kind: "unanchored" },
        role: "attachment",
        tone: "warning",
        status: "ready queue",
    });
    let finish = scheduling::makespan(state);
    let scene = scene::entities(scene::timeline("Machine timeline", lanes, spans, finish), entities);
    scene.metrics = [
        metric("pending", "Pending operations", pending.len(), ()),
        metric("scheduled", "Scheduled operations", scheduling::operations().len() - pending.len(), ()),
        metric("makespan", "Current makespan", finish, "ticks"),
    ];
    scene
}

// The first of `machines` holding `asset` of `operation` in `account`.
fn machine_holding(state, account, asset, operation) {
    for machine in machines() {
        if state.balance(account, `${asset}/${operation}/${machine}`) > 0 {
            return machine;
        }
    }
    ()
}

fn execution_scene(state) {
    let now = state.balance("success", "now");
    let lanes = [];
    for machine in machines() {
        lanes.push(#{
            id: #{ key: `machine/${machine}`, label: machine_name(machine) },
            classes: if state.balance(`machine/${machine}`, "broken") > 0 { ["blocked"] } else { [] },
        });
    }
    let spans = [];
    let entities = [];
    for operation in scheduling::operations() {
        let job = job_of(operation);
        let account = `job/${job}`;
        let started = state.balance("success", `started/${operation}`) > 0;
        let completed_at = state.balance("success", `end-time/${operation}`);
        let queued = machine_holding(state, account, "queued", operation);
        let running = machine_holding(state, account, "running", operation);
        let paused = machine_holding(state, account, "paused", operation);
        let executed = machine_holding(state, "success", "executed-on", operation);
        let status = if completed_at > 0 {
            `completed at ${completed_at}`
        } else if paused != () {
            `paused · ${state.balance(account, "work-remaining")} ticks remain`
        } else if running != () {
            `running until ${state.balance(account, "due")}`
        } else if queued != () {
            "reserved in machine queue"
        } else if now < state.balance(account, "arrival") {
            `job arrives at ${state.balance(account, "arrival")}`
        } else if state.balance(account, `ready/${operation}`) > 0 {
            "ready for machine choice"
        } else {
            "predecessor unfinished"
        };
        if executed != () && started {
            let start = state.balance("success", `start-time/${operation}`);
            let end = if completed_at > 0 {
                completed_at
            } else if running != () {
                state.balance(account, "due")
            } else {
                max(now, start + 1)
            };
            let interrupted = state.balance("success", `interrupted/${operation}`) > 0;
            spans.push(#{
                id: `operation/${operation}`,
                lane: `machine/${executed}`,
                start: start.to_int(),
                end: end.to_int(),
                label: operation_name(operation) + if interrupted { " · interrupted" } else { "" },
                classes: [`job/${job}`, if completed_at > 0 { "completed" } else { "active" }],
            });
        }
        let at = [queued, running, paused, executed].filter(|machine| machine != ());
        entities.push(#{
            id: #{ key: `operation/${operation}`, label: operation_name(operation) },
            glyph: "task",
            anchor: if at.is_empty() {
                #{ kind: "unanchored" }
            } else {
                #{ kind: "timeline", lane: `machine/${at[0]}`, at: now.to_int() }
            },
            role: "attachment",
            tone: if completed_at > 0 { "success" } else if paused != () { "warning" } else { "active" },
            status: status,
            account: `scheduling/account/${account}`,
            evidence: [#{ kind: "account", account: `scheduling/account/${account}` }],
        });
    }
    let condition = if state.balance("machine/two", "broken") > 0 {
        "broken"
    } else if state.balance("machine/two", "repairing") > 0 {
        "repair underway"
    } else {
        "online"
    };
    let timeline = scene::timeline(
        "Executed work, setup and machine occupancy", lanes, spans, now.to_int(),
    );
    timeline.entities = [];
    let scene = scene::entities(timeline, entities);
    scene.metrics = [
        metric("world-time", "World time", now, "ticks"),
        metric("labor", "Available crew", state.balance("success", "labor"), ()),
        metric("busy-labor", "Occupied crew", state.balance("success", "busy-labor"), ()),
        metric("machine-two", "Machine Two", condition, ()),
        metric("repair-kits", "Repair kits left", state.balance("success", "repair-kit"), ()),
        metric("one-done", "Job One completion", state.balance("job/one", "completion-time"), "ticks"),
        metric("two-done", "Job Two completion", state.balance("job/two", "completion-time"), "ticks"),
    ];
    scene
}
