let model = #{
    ontology: "scheduling",
    laws: ["scheduling/showcase-and-stress", "scheduling/stress"],
    tables: #{
        "queue/two-a/{machine}": #{relations: [#{gives: ["arrival"], rows: [["one", "3"], ["two", "3"]], variables: ["machine"]}]},
        "queue/two-b/{machine}": #{relations: [#{gives: ["arrival"], rows: [["two", "3"], ["three", "3"]], variables: ["machine"]}]},
        "queue/two-c/{machine}": #{relations: [#{gives: ["arrival"], rows: [["three", "3"], ["one", "3"]], variables: ["machine"]}]}
    },
    constants: #{"fault-at": "4", "repair-duration": "3"}
};
let accounts = [];
accounts.push(opening(account("job/one"), basket([amount(asset("ready/one-a"), 1)])));
accounts.push(opening(account("job/two"), basket([amount(asset("ready/two-a"), 1)])));
accounts.push(opening(account("machine/one"), basket([
    amount(asset("configured/one"), 1),
    amount(asset("idle"), 1),
    amount(asset("online"), 1),
    amount(asset("queue-free"), 1)
])));
accounts.push(opening(account("machine/three"), basket([
    amount(asset("configured/one"), 1),
    amount(asset("idle"), 1),
    amount(asset("online"), 1),
    amount(asset("queue-free"), 1)
])));
accounts.push(opening(account("machine/two"), basket([
    amount(asset("configured/two"), 1),
    amount(asset("idle"), 1),
    amount(asset("online"), 1),
    amount(asset("queue-free"), 1)
])));
accounts.push(opening(account("success"), basket([
    amount(asset("active"), 1),
    amount(asset("fault-pending"), 1),
    amount(asset("labor"), 1),
    amount(asset("not-started/one-a"), 1),
    amount(asset("not-started/one-b"), 1),
    amount(asset("not-started/one-c"), 1),
    amount(asset("not-started/two-a"), 1),
    amount(asset("not-started/two-b"), 1),
    amount(asset("not-started/two-c"), 1),
    amount(asset("repair-kit"), 1),
    amount(asset("time-remaining"), 24)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("job-phase-accounting");
    for name in [
        "done",
        "ready/one-a",
        "queued/one-a/one",
        "running/one-a/one",
        "paused/one-a/one",
        "queued/one-a/two",
        "running/one-a/two",
        "paused/one-a/two",
        "ready/one-b",
        "queued/one-b/two",
        "running/one-b/two",
        "paused/one-b/two",
        "queued/one-b/three",
        "running/one-b/three",
        "paused/one-b/three",
        "ready/one-c",
        "queued/one-c/three",
        "running/one-c/three",
        "paused/one-c/three",
        "queued/one-c/one",
        "running/one-c/one",
        "paused/one-c/one",
        "ready/two-a",
        "queued/two-a/one",
        "running/two-a/one",
        "paused/two-a/one",
        "queued/two-a/two",
        "running/two-a/two",
        "paused/two-a/two",
        "ready/two-b",
        "queued/two-b/two",
        "running/two-b/two",
        "paused/two-b/two",
        "queued/two-b/three",
        "running/two-b/three",
        "paused/two-b/three",
        "ready/two-c",
        "queued/two-c/three",
        "running/two-c/three",
        "paused/two-c/three",
        "queued/two-c/one",
        "running/two-c/one",
        "paused/two-c/one"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("three-machine-queues");
    for name in [
        "queue-free",
        "queue-ticket/one-a",
        "queue-ticket/one-b",
        "queue-ticket/one-c",
        "queue-ticket/two-a",
        "queue-ticket/two-b",
        "queue-ticket/two-c"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("each-operation-starts-once");
    for name in [
        "not-started/one-a",
        "started/one-a",
        "not-started/one-b",
        "started/one-b",
        "not-started/one-c",
        "started/one-c",
        "not-started/two-a",
        "started/two-a",
        "not-started/two-b",
        "started/two-b",
        "not-started/two-c",
        "started/two-c"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("machine-occupancy");
    rule.weight(asset("idle"), 1);
    rule.weight(asset("busy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("machine-condition");
    for name in ["online", "broken", "repairing"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("setup-configuration");
    rule.weight(asset("configured/one"), 1);
    rule.weight(asset("configured/two"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("shared-labor");
    rule.weight(asset("labor"), 1);
    rule.weight(asset("busy-labor"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-repair-supply");
    rule.weight(asset("repair-kit"), 1);
    rule.weight(asset("used-repair-kit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("scheduled-disruption");
    rule.weight(asset("fault-pending"), 1);
    rule.weight(asset("fault-occurred"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("bounded-workday");
    rule.weight(asset("now"), 1);
    rule.weight(asset("time-remaining"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("execution-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
