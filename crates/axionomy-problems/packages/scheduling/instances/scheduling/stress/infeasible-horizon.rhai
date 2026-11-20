let model = #{
    ontology: "scheduling",
    laws: ["scheduling/short-horizon", "scheduling/showcase-infeasible-horizon-and-stress-infeasible-horizon"],
    tables: #{
        "schedule/one-a/{ready}/{start}": #{
            variables: [#{name: "ready", values: ["0"]}, #{name: "start", range: ["0", "1"]}],
            relations: [#{rows: [["0", "0"], ["0", "1"]], variables: ["ready", "start"]}]
        },
        "schedule/two-a/{ready}/{start}": #{
            variables: [#{name: "ready", values: ["0"]}, #{name: "start", range: ["0", "1"]}],
            relations: [#{rows: [["0", "0"], ["0", "1"]], variables: ["ready", "start"]}]
        }
    }
};
let accounts = [];
accounts.push(opening(account("job/one"), basket([amount(asset("ready-at/one-a/0"), 1)])));
accounts.push(opening(account("job/two"), basket([amount(asset("ready-at/two-a/0"), 1)])));
accounts.push(opening(account("slot/one/0"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/one/1"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/one/2"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/three/0"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/three/1"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/three/2"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/0"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/1"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/2"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("success"), basket([amount(asset("active"), 1)])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("two-job-state-tokens");
    for name in [
        "ready-at/one-a/0",
        "completed-at/one-a/0",
        "ready-at/one-a/1",
        "completed-at/one-a/1",
        "ready-at/one-a/2",
        "completed-at/one-a/2",
        "ready-at/one-a/3",
        "completed-at/one-a/3",
        "ready-at/one-b/0",
        "completed-at/one-b/0",
        "ready-at/one-b/1",
        "completed-at/one-b/1",
        "ready-at/one-b/2",
        "completed-at/one-b/2",
        "ready-at/one-b/3",
        "completed-at/one-b/3",
        "ready-at/one-c/0",
        "completed-at/one-c/0",
        "ready-at/one-c/1",
        "completed-at/one-c/1",
        "ready-at/one-c/2",
        "completed-at/one-c/2",
        "ready-at/one-c/3",
        "completed-at/one-c/3",
        "ready-at/two-a/0",
        "completed-at/two-a/0",
        "ready-at/two-a/1",
        "completed-at/two-a/1",
        "ready-at/two-a/2",
        "completed-at/two-a/2",
        "ready-at/two-a/3",
        "completed-at/two-a/3",
        "ready-at/two-b/0",
        "completed-at/two-b/0",
        "ready-at/two-b/1",
        "completed-at/two-b/1",
        "ready-at/two-b/2",
        "completed-at/two-b/2",
        "ready-at/two-b/3",
        "completed-at/two-b/3",
        "ready-at/two-c/0",
        "completed-at/two-c/0",
        "ready-at/two-c/1",
        "completed-at/two-c/1",
        "ready-at/two-c/2",
        "completed-at/two-c/2",
        "ready-at/two-c/3",
        "completed-at/two-c/3"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("machine-slot-capacity");
    for name in [
        "available",
        "reserved/one-a",
        "reserved/one-b",
        "reserved/one-c",
        "reserved/two-a",
        "reserved/two-b",
        "reserved/two-c"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("schedule-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
