let model = #{
    ontology: "scheduling",
    laws: ["scheduling/micro", "scheduling/micro-and-micro-infeasible-horizon", "scheduling/short-horizon"],
    tables: #{
        "finish/{job-one-end}/{job-two-end}/{makespan}": #{
            variables: [
                #{name: "job-one-end", range: ["0", "5"]},
                #{name: "job-two-end", range: ["0", "5"]},
                #{name: "makespan", range: ["0", "5"]}
            ],
            relations: [
                #{
                    rows: [
                        ["0", "0", "0"],
                        ["0", "1", "1"],
                        ["0", "2", "2"],
                        ["0", "3", "3"],
                        ["0", "4", "4"],
                        ["0", "5", "5"],
                        ["1", "0", "1"],
                        ["1", "1", "1"],
                        ["1", "2", "2"],
                        ["1", "3", "3"],
                        ["1", "4", "4"],
                        ["1", "5", "5"],
                        ["2", "0", "2"],
                        ["2", "1", "2"],
                        ["2", "2", "2"],
                        ["2", "3", "3"],
                        ["2", "4", "4"],
                        ["2", "5", "5"],
                        ["3", "0", "3"],
                        ["3", "1", "3"],
                        ["3", "2", "3"],
                        ["3", "3", "3"],
                        ["3", "4", "4"],
                        ["3", "5", "5"],
                        ["4", "0", "4"],
                        ["4", "1", "4"],
                        ["4", "2", "4"],
                        ["4", "3", "4"],
                        ["4", "4", "4"],
                        ["4", "5", "5"],
                        ["5", "0", "5"],
                        ["5", "1", "5"],
                        ["5", "2", "5"],
                        ["5", "3", "5"],
                        ["5", "4", "5"],
                        ["5", "5", "5"]
                    ],
                    variables: ["job-one-end", "job-two-end", "makespan"]
                }
            ]
        },
        "schedule/one-a/{ready}/{start}": #{
            variables: [#{name: "ready", values: ["0"]}, #{name: "start", range: ["0", "3"]}],
            relations: [#{rows: [["0", "0"], ["0", "1"], ["0", "2"], ["0", "3"]], variables: ["ready", "start"]}]
        },
        "schedule/two-a/{ready}/{start}": #{
            variables: [#{name: "ready", values: ["0"]}, #{name: "start", range: ["0", "3"]}],
            relations: [#{rows: [["0", "0"], ["0", "1"], ["0", "2"], ["0", "3"]], variables: ["ready", "start"]}]
        }
    }
};
let accounts = [];
accounts.push(opening(account("job/one"), basket([amount(asset("ready-at/one-a/0"), 1)])));
accounts.push(opening(account("job/two"), basket([amount(asset("ready-at/two-a/0"), 1)])));
accounts.push(opening(account("slot/one/0"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/one/1"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/one/2"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/one/3"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/one/4"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/0"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/1"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/2"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/3"), basket([amount(asset("available"), 1)])));
accounts.push(opening(account("slot/two/4"), basket([amount(asset("available"), 1)])));
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
        "ready-at/one-a/4",
        "completed-at/one-a/4",
        "ready-at/one-a/5",
        "completed-at/one-a/5",
        "ready-at/one-b/0",
        "completed-at/one-b/0",
        "ready-at/one-b/1",
        "completed-at/one-b/1",
        "ready-at/one-b/2",
        "completed-at/one-b/2",
        "ready-at/one-b/3",
        "completed-at/one-b/3",
        "ready-at/one-b/4",
        "completed-at/one-b/4",
        "ready-at/one-b/5",
        "completed-at/one-b/5",
        "ready-at/two-a/0",
        "completed-at/two-a/0",
        "ready-at/two-a/1",
        "completed-at/two-a/1",
        "ready-at/two-a/2",
        "completed-at/two-a/2",
        "ready-at/two-a/3",
        "completed-at/two-a/3",
        "ready-at/two-a/4",
        "completed-at/two-a/4",
        "ready-at/two-a/5",
        "completed-at/two-a/5",
        "ready-at/two-b/0",
        "completed-at/two-b/0",
        "ready-at/two-b/1",
        "completed-at/two-b/1",
        "ready-at/two-b/2",
        "completed-at/two-b/2",
        "ready-at/two-b/3",
        "completed-at/two-b/3",
        "ready-at/two-b/4",
        "completed-at/two-b/4",
        "ready-at/two-b/5",
        "completed-at/two-b/5"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("machine-slot-capacity");
    for name in ["available", "reserved/one-a", "reserved/one-b", "reserved/two-a", "reserved/two-b"] {
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
