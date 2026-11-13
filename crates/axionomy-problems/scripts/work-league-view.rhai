// The Autonomous Work League: how people read workers competing for
// contracts on a shared network. Labels and formats name its identifiers;
// the scene draws the places and the routes between them, each worker where
// it stands, each contract and who holds it, and in a season the service
// bays; the leaderboards rank the workers by contract value, throughput,
// resource efficiency, residual waste, reliability and Pareto standing. The
// league's workers, places and contracts are its economy script's.

import "work-league" as league;

// A name as people read it: `atlas` is `Atlas`.
fn capital(name) {
    name.sub_string(0, 1).to_upper() + name.sub_string(1)
}

fn policy_label(policy) {
    switch policy {
        "sprinter" => "Sprinter",
        "steward" => "Steward",
        "value-hunter" => "Value hunter",
        "resilient" => "Resilient",
    }
}

fn asset_label(parts) {
    switch parts[1] {
        "world-identity" => "Identity: work economy",
        "season-active" => "Season active",
        "season-ended" => "Season ended",
        "tick" => "World time",
        "horizon" => "Season horizon",
        "contracts-open" => "Unresolved contracts",
        "contracts-closed" => "Resolved contracts",
        "activities" => "Activities in progress",
        "permit" => `${capital(parts[2])} may decide this tick`,
        "acted" => `${capital(parts[2])} has decided this tick`,
        "idle" => "Ready for work",
        "busy" => "Activity in progress",
        "free-contract" => "Can accept a contract",
        "ready-at" => "Activity finishes at",
        "release-at" => "Contract opens at",
        "deadline" => "Delivery deadline",
        "lease-until" => "Claim reserved through",
        "expired" => "Expired without delivery",
        "transferable" => "Handoff available",
        "handed-off" => "Contract handed off",
        "assists" => "Paid handoffs",
        "completed-job" => `Dependency completed: job ${parts[2]}`,
        "credit" => "Spendable credit",
        "wear" => "Equipment wear",
        "repairing" => "Under repair",
        "facility-identity" => `Identity: ${capital(parts[2])}`,
        "vacant" => `${capital(parts[2])} service slot free`,
        "occupied" => `${capital(parts[2])} occupied by ${capital(parts[3])}`,
        "servicing" => `Receiving ${parts[2]} service`,
        "charging-energy" => "Energy reserved for charging",
        "reclaiming-material" => "Material being reclaimed",
        "embodied-material" => "Material incorporated into work",
        "recycling-loss" => "Material lost during recycling",
        "agent-identity" => `Identity: ${capital(parts[2])}`,
        "job-identity" => `Identity: job ${parts[2]}`,
        "policy" => `${policy_label(parts[2])} policy`,
        "at" => `At ${capital(parts[2])}`,
        "operational" => "Operational",
        "energy" => "Energy left",
        "time-remaining" => "Time left",
        "material" => "Material left",
        "available" => "Available for claim",
        "assigned" => `Assigned to ${capital(parts[2])}`,
        "claimed" => `Claim on job ${parts[2]}`,
        "pending" => "Waiting for work",
        "in-progress" => "Work in progress",
        "awaiting" => `Job ${parts[2]} ${parts[3]} work awaiting its outcome`,
        "resolved" => `Job ${parts[2]} ${parts[3]} work ${if parts[4] == "success" { "succeeded" } else { "failed" }}`,
        "completed" => "Jobs completed",
        "value" => "Contract value earned",
        "attempts" => "Work attempts",
        "successes" => "Successful attempts",
        "failures" => "Failed attempts",
        "spent-energy" => "Energy spent",
        "elapsed-time" => "Elapsed time",
        "material-spent" => "Material spent",
        "waste" => "Residual waste",
        "recycled-waste" => "Waste recycled",
        "damage" => "Damage requiring repair",
        "repair-supply" => "Repair supply",
        "spent-repair-supply" => "Repair supply used",
        "charge-supply" => "Charging energy supply",
        "recycler-capacity" => "Recycler available",
        "outcome-weight" => `Job ${parts[2]} ${parts[3]} ${parts[4]} weight`,
        _ => (),
    }
}

// A deadline missed: unclaimed, while a worker holds the job, or during
// its work.
fn expiry_label(parts) {
    if parts[3] == "none" {
        return `Job ${parts[2]} misses its deadline unclaimed`;
    }
    let agent = capital(parts[4]);
    if parts[5] == "none" {
        `Job ${parts[2]} misses its deadline while ${agent} holds it`
    } else {
        `Job ${parts[2]} misses its deadline during ${agent}'s ${parts[6]} work`
    }
}

fn rate_label(parts) {
    switch parts[1] {
        "tick" => "Advance the shared work clock",
        "wait" => `${capital(parts[2])} waits this tick`,
        "begin-service" => `${capital(parts[2])} starts ${
            if parts[4] == "true" { "preventive maintenance" } else { "service" }
        } at the ${parts[3]}`,
        "end-service" => `${capital(parts[2])} finishes service and frees the ${parts[3]}`,
        "handoff" => `${capital(parts[2])} hands job ${parts[4]} to ${capital(parts[3])} for 2 credits`,
        "release-claim" => if parts[4] == "true" {
            `${capital(parts[2])}'s claim on job ${parts[3]} lapses`
        } else {
            `${capital(parts[2])} releases job ${parts[3]} for another worker`
        },
        "expire" => expiry_label(parts),
        "end-season" => "Close the resolved work season",
        "claim" => `${capital(parts[2])} claims job ${parts[3]}`,
        "move" => `${capital(parts[2])} moves from ${capital(parts[3])} to ${capital(parts[4])}`,
        "begin" => `${capital(parts[2])} begins job ${parts[3]} in ${parts[4]} mode`,
        "resolve" => `${capital(parts[2])}'s ${parts[4]} work on job ${parts[3]} ${
            if parts[5] == "success" { "succeeds" } else { "fails" }
        }`,
        "finish" => `${capital(parts[2])} records a ${parts[4]} ${parts[5]} on job ${parts[3]}`,
        "repair" => `Repair ${capital(parts[2])}`,
        "recharge" => `Recharge ${capital(parts[2])}`,
        "recycle" => `${capital(parts[2])} recycles one waste unit`,
        _ => (),
    }
}

fn label(item) {
    let parts = item.split("/");
    switch parts[0] {
        "account" => switch parts[1] {
            "agent" => `Worker ${capital(parts[2])}`,
            "job" => `Job ${parts[2]}`,
            "facility" => `${capital(parts[2])} facility`,
            "nature" => "Disruption model",
            "world" => "Shared work clock and contracts",
            _ => (),
        },
        "asset" => asset_label(parts),
        "rate" => rate_label(parts),
        // A role reads as its words: `worker/atlas` is `Worker Atlas`.
        "role" => if parts.len() > 2 {
            `${capital(parts[1])} ${capital(parts[2])}`
        } else {
            capital(parts[1])
        },
        _ => (),
    }
}

// The unit of an asset's quantities, if it has one.
fn unit(asset) {
    switch asset.split("/")[0] {
        "tick" | "horizon" | "ready-at" | "release-at" | "deadline" | "lease-until"
            | "time-remaining" | "elapsed-time" => "ticks",
        "credit" | "value" => "credits",
        "energy" | "spent-energy" | "charging-energy" | "charge-supply" | "material"
            | "material-spent" | "reclaiming-material" | "embodied-material" | "recycling-loss"
            | "waste" | "recycled-waste" | "repair-supply" | "spent-repair-supply" => "units",
        _ => (),
    }
}

// Time reads in ticks of the shared work clock, earnings in credits, and
// energy, material, waste and supplies in units.
fn format(asset) {
    let unit = unit(asset);
    if unit == () { () } else { #{ scale: 0, unit: unit } }
}

fn val(state, account, asset) {
    state.balance(account, asset).to_int()
}

fn metric(key, label, value, unit) {
    #{ key: key, label: label, value: value.to_string(), unit: unit }
}

// A metric showing a quantity of `asset` in its unit.
fn asset_metric(key, label, asset, quantity) {
    metric(key, label, quantity, unit(asset))
}

// Where a worker stands, if anywhere.
fn whereabouts(state, agent) {
    league::first_held(state.balances(`agent/${agent}`), "at", league::locations())
}

// An entity on the location `node`, shown as `account`.
fn entity(key, label, glyph, node, role, tone, status, account) {
    #{
        id: #{ key: key, label: label },
        glyph: glyph,
        anchor: #{ kind: "graph_node", node: `league/location/${node}` },
        role: role,
        tone: tone,
        status: status,
        account: `work-league/account/${account}`,
        evidence: [#{ kind: "account", account: `work-league/account/${account}` }],
    }
}

fn worker(state, agent, continuous) {
    let account = `agent/${agent}`;
    let location = whereabouts(state, agent);
    if location == () { location = "depot"; }
    let balances = state.balances(account);
    let policy = league::first_held(balances, "policy", league::policies());
    if policy == () { policy = "sprinter"; }
    let status = if val(state, account, "busy") > 0 {
        let servicing = league::first_held(balances, "servicing", league::facilities());
        let activity = if servicing == () { "working" } else { `${capital(servicing)} service` };
        `${activity} · ready at ${val(state, account, "ready-at")}`
    } else if val(state, account, "damage") > 0 {
        "damaged · repair or handoff needed"
    } else {
        `${val(state, account, "completed")} jobs · ${val(state, account, "value")} value`
    };
    let drawn = entity(
        `league/agent/${agent}`, capital(agent), "robot", location, "occupant",
        if val(state, account, "damage") > 0 { "danger" } else { "active" },
        status, account,
    );
    drawn.evidence = [#{
        kind: "balance", account: `work-league/account/${account}`,
        asset: `work-league/asset/at/${location}`,
    }];
    drawn.metrics = [
        metric("policy", "Policy", policy_label(policy), ()),
        asset_metric("value", "Value", "value", val(state, account, "value")),
        metric("jobs", "Completed", val(state, account, "completed"), "jobs"),
        asset_metric("energy", "Energy left", "energy", val(state, account, "energy")),
        asset_metric("waste", "Residual waste", "waste", val(state, account, "waste")),
    ];
    if continuous {
        drawn.metrics += [
            asset_metric("credit", "Spendable credit", "credit", val(state, account, "credit")),
            asset_metric("material", "Material left", "material", val(state, account, "material")),
            asset_metric("wear", "Equipment wear", "wear", val(state, account, "wear")),
            metric("handoffs", "Paid handoffs", val(state, account, "assists"), "contracts"),
        ];
    }
    drawn
}

fn contract(state, number, continuous, tick) {
    let account = `job/${number}`;
    let spec = league::job_spec(number);
    let assigned = league::first_held(state.balances(account), "assigned", league::workers());
    let shown = if val(state, account, "completed") > 0 {
        ["success", "completed"]
    } else if val(state, account, "expired") > 0 {
        ["danger", "deadline missed"]
    } else if continuous && tick < val(state, account, "release-at") {
        ["neutral", `opens at ${val(state, account, "release-at")}`]
    } else if assigned != () {
        let work = if val(state, account, "in-progress") > 0 { "being worked" } else { "claimed" };
        ["active", `${work} by ${capital(assigned)}`]
    } else {
        ["neutral", `${spec.value} credits · risk ${spec.risk}`]
    };
    let drawn = entity(
        `league/job/${number}`, `Job ${number}`, "task", spec.location, "attachment",
        shown[0], shown[1], account,
    );
    drawn.metrics = [
        asset_metric("value", "Value", "value", spec.value),
        metric("risk", "Failure weight", spec.risk, "of 10"),
    ];
    if continuous {
        drawn.metrics.push(asset_metric("deadline", "Deadline", "deadline", val(state, account, "deadline")));
        if assigned != () && val(state, account, "pending") > 0 {
            drawn.metrics.push(asset_metric(
                "lease", "Claim expires after", "lease-until", val(state, account, "lease-until"),
            ));
        }
    }
    drawn
}

fn bay(state, facility) {
    let account = `facility/${facility}`;
    let occupant = league::first_held(
        state.balances(account), `occupied/${facility}`, league::workers(),
    );
    let drawn = entity(
        `league/facility/${facility}`, `${capital(facility)} bay`, "task", facility, "attachment",
        if occupant != () { "active" } else { "neutral" },
        if occupant != () {
            `${capital(occupant)} until ${val(state, `agent/${occupant}`, "ready-at")}`
        } else {
            "free"
        },
        account,
    );
    let waiting = 0;
    for agent in league::workers() {
        if whereabouts(state, agent) == facility && val(state, `agent/${agent}`, "idle") > 0 {
            waiting += 1;
        }
    }
    drawn.metrics = [metric("waiting", "Workers waiting", waiting, "workers")];
    drawn
}

fn scene(state, frame) {
    let continuous = state.has_account("world");
    let tick = val(state, "world", "tick");
    let competitors = league::competitors(state);
    let nodes = [
        ["depot", 360.0, 210.0],
        ["north", 360.0, 40.0],
        ["east", 650.0, 170.0],
        ["south", 360.0, 380.0],
        ["west", 70.0, 170.0],
        ["workshop", 80.0, 340.0],
        ["charger", 640.0, 340.0],
        ["recycler", 650.0, 40.0],
    ].map(|node| #{
        id: #{ key: `league/location/${node[0]}`, label: capital(node[0]) },
        classes: if node[0] in league::facilities() { ["facility"] } else { [] },
        x: node[1],
        y: node[2],
    });
    // Each route once, from the place first in the identifiers' order, in
    // that order.
    let routes = #{};
    for rate in state.rates("move/") {
        let parts = rate.split("/");
        if parts[2] < parts[3] { routes[`${parts[2]}/${parts[3]}`] = [parts[2], parts[3]]; }
    }
    let ordered = routes.keys();
    ordered.sort();
    // Where the competing workers stand.
    let occupied = [];
    for agent in competitors {
        let location = whereabouts(state, agent);
        if location != () { occupied.push(location); }
    }
    let edges = [];
    for (key, index) in ordered {
        let route = routes[key];
        edges.push(#{
            id: `league/route/${index}`,
            source: `league/location/${route[0]}`,
            target: `league/location/${route[1]}`,
            label: "1 energy · 1 tick",
            classes: if route[1] in occupied { ["current"] } else { [] },
        });
    }

    let entities = [];
    for agent in competitors {
        entities.push(worker(state, agent, continuous));
    }
    let completed = 0;
    let expired = 0;
    for number in 1..=24 {
        let account = `job/${number}`;
        if val(state, account, "completed") > 0 { completed += 1; }
        if val(state, account, "expired") > 0 { expired += 1; }
        if state.has_account(account) {
            entities.push(contract(state, number, continuous, tick));
        }
    }
    if continuous {
        for facility in league::facilities() {
            entities.push(bay(state, facility));
        }
    }

    let scene = scene::entities(
        scene::graph("Autonomous work network", nodes, edges, ()),
        entities,
    );
    scene.metrics = [
        metric("completed", "Jobs completed", completed, "jobs"),
        metric("active-agents", "Competing agents", competitors.len(), "agents"),
        asset_metric("repair-supply", "Repair supply", "repair-supply", val(state, "facility/workshop", "repair-supply")),
    ];
    if continuous {
        scene.metrics.insert(0, asset_metric("clock", "World time", "tick", tick));
        scene.metrics.insert(2, metric("expired", "Missed deadlines", expired, "jobs"));
        scene.metrics.push(metric("activities", "Simultaneous activities", val(state, "world", "activities"), "workers"));
    }
    scene
}

// What a worker has done, as every board reads it.
fn standing(state, agent, order) {
    let account = `agent/${agent}`;
    #{
        agent: agent,
        order: order,
        value: val(state, account, "value"),
        completed: val(state, account, "completed"),
        attempts: val(state, account, "attempts"),
        successes: val(state, account, "successes"),
        elapsed: val(state, account, "elapsed-time"),
        energy: val(state, account, "spent-energy"),
        material: val(state, account, "material-spent"),
        waste: val(state, account, "waste"),
        recycled: val(state, account, "recycled-waste"),
    }
}

fn components(row) {
    [
        asset_metric("value", "Contract value", "value", row.value),
        metric("completed", "Jobs completed", row.completed, "jobs"),
        asset_metric("elapsed", "Elapsed time", "elapsed-time", row.elapsed),
        asset_metric("energy", "Energy spent", "spent-energy", row.energy),
        asset_metric("material", "Material spent", "material-spent", row.material),
        asset_metric("waste", "Residual waste", "waste", row.waste),
        asset_metric("recycled", "Waste recycled", "recycled-waste", row.recycled),
        asset_metric("attempts", "Attempts", "attempts", row.attempts),
    ]
}

fn gcd(left, right) {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    if left < 1 { 1 } else { left }
}

// A worker's score on a board: a ratio in lowest terms, whether the board
// ranks it, and what it shows instead of the ratio, if anything.
fn score(row, numerator, denominator, eligible, display) {
    let denominator = if denominator < 1 { 1 } else { denominator };
    let divisor = gcd(numerator, denominator);
    #{
        row: row,
        numerator: numerator / divisor,
        denominator: denominator / divisor,
        eligible: eligible,
        display: display,
    }
}

// The order the league ranks scores in: the ranked first, then by ratio in
// the board's direction, then by worker.
fn ranked_before(left, right, maximize) {
    if left.eligible != right.eligible {
        return if left.eligible { -1 } else { 1 };
    }
    let ahead = left.numerator * right.denominator;
    let behind = right.numerator * left.denominator;
    let comparison = if ahead < behind { -1 } else if ahead > behind { 1 } else { 0 };
    if maximize { comparison = -comparison; }
    if comparison != 0 { comparison } else { left.row.order - right.row.order }
}

// A board of `scores`, in the league's order. Each ranked worker's score is
// its ratio over the board's common denominator, so equal ratios tie.
fn board(key, label, description, maximize, unit, scores) {
    scores.sort(|left, right| ranked_before(left, right, maximize));
    let common = 1;
    for score in scores {
        if score.eligible { common = common / gcd(common, score.denominator) * score.denominator; }
    }
    let entries = [];
    for score in scores {
        entries.push(#{
            participant: #{
                key: `work-league/account/agent/${score.row.agent}`,
                label: `Worker ${capital(score.row.agent)}`,
            },
            score: score.numerator * (common / score.denominator),
            value: if score.display != () { score.display }
                else if score.denominator == 1 { score.numerator.to_string() }
                else { `${score.numerator}/${score.denominator}` },
            unit: unit,
            eligible: score.eligible,
            components: components(score.row),
        });
    }
    #{
        key: key,
        label: label,
        description: description,
        direction: if maximize { "maximize" } else { "minimize" },
        entries: entries,
    }
}

// Whether another worker who completed work is no worse than `row` on
// value, completions, residual waste and elapsed time, and better on one.
fn dominated(row, rows) {
    if row.completed == 0 { return false; }
    for other in rows {
        if other.agent != row.agent && other.completed > 0
            && other.value >= row.value && other.completed >= row.completed
            && other.waste <= row.waste && other.elapsed <= row.elapsed
            && (other.value > row.value || other.completed > row.completed
                || other.waste < row.waste || other.elapsed < row.elapsed) {
            return true;
        }
    }
    false
}

fn leaderboards(state, frame) {
    let rows = [];
    for (agent, order) in league::competitors(state) {
        rows.push(standing(state, agent, order));
    }
    let standings = [];
    for row in rows {
        let beaten = dominated(row, rows);
        standings.push(score(
            row, if beaten { 0 } else { 1 }, 1, row.completed > 0,
            if beaten { "dominated" } else { "non-dominated" },
        ));
    }
    [
        board(
            "contract-value", "Contract value", "Total value earned from completed jobs.",
            true, "credits",
            rows.map(|row| score(row, row.value, 1, true, ())),
        ),
        board(
            "throughput", "Fastest throughput",
            "Completed jobs per elapsed tick; agents without a completion are not yet eligible.",
            true, "jobs / tick",
            rows.map(|row| score(row, row.completed, row.elapsed, row.completed > 0, ())),
        ),
        board(
            "resource-efficiency", "Resource efficiency",
            "Contract value per unit of energy and material spent.",
            true, "value / resource",
            rows.map(|row| score(row, row.value, row.energy + row.material, row.completed > 0, ())),
        ),
        board(
            "least-waste", "Least residual waste",
            "Waste still held after recycling; only agents that completed work are ranked.",
            false, "units",
            rows.map(|row| score(row, row.waste, 1, row.completed > 0, ())),
        ),
        board(
            "reliability", "Reliability",
            "Successful jobs per attempt, retaining failures rather than hiding them.",
            true, "successes / attempt",
            rows.map(|row| score(row, row.successes, row.attempts, row.attempts > 0, ())),
        ),
        board(
            "pareto-standing", "Pareto standing",
            "Non-dominated agents are not worse on value, completions, residual waste, and elapsed time all at once.",
            true, (), standings,
        ),
    ]
}
