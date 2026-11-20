[
    [
        "finish/{job-one-end}/{job-two-end}/{makespan}",
        #{
            accounts: [["primary-job", "job/one"], ["schedule", "success"], ["secondary-job", "job/two"]],
            consume: [["schedule", [["active", "1"]]]],
            distinct: [],
            preserve: [
                ["primary-job", [["completed-at/one-c/{job-one-end}", "1"]]],
                ["secondary-job", [["completed-at/two-c/{job-two-end}", "1"]]]
            ],
            produce: [["schedule", [["makespan/{makespan}", "1"], ["solved", "1"]]]],
            variables: [
                #{name: "job-one-end", range: ["0", "3"]},
                #{name: "job-two-end", range: ["0", "3"]},
                #{name: "makespan", range: ["0", "3"]}
            ],
            relations: [
                #{
                    rows: [
                        ["0", "0", "0"],
                        ["0", "1", "1"],
                        ["0", "2", "2"],
                        ["0", "3", "3"],
                        ["1", "0", "1"],
                        ["1", "1", "1"],
                        ["1", "2", "2"],
                        ["1", "3", "3"],
                        ["2", "0", "2"],
                        ["2", "1", "2"],
                        ["2", "2", "2"],
                        ["2", "3", "3"],
                        ["3", "0", "3"],
                        ["3", "1", "3"],
                        ["3", "2", "3"],
                        ["3", "3", "3"]
                    ],
                    variables: ["job-one-end", "job-two-end", "makespan"]
                }
            ]
        }
    ],
    [
        "schedule/one-b/{ready}/{start}",
        #{
            accounts: [["primary-job", "job/one"], ["schedule", "success"], ["slot0", "slot/two/{start}"]],
            consume: [["primary-job", [["ready-at/one-b/{ready}", "1"]]], ["slot0", [["available", "1"]]]],
            derived: [
                #{
                    name: "end",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "start"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            distinct: [],
            preserve: [["schedule", [["active", "1"]]]],
            produce: [["primary-job", [["ready-at/one-c/{end}", "1"]]], ["slot0", [["reserved/one-b", "1"]]]],
            variables: [#{name: "ready", values: ["2"]}, #{name: "start", range: ["2", "2"]}],
            relations: [#{rows: [["2", "2"]], variables: ["ready", "start"]}]
        }
    ],
    [
        "schedule/one-c/{ready}/{start}",
        #{
            accounts: [["primary-job", "job/one"], ["schedule", "success"], ["slot0", "slot/three/{start}"]],
            consume: [["primary-job", [["ready-at/one-c/{ready}", "1"]]], ["slot0", [["available", "1"]]]],
            derived: [
                #{
                    name: "end",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "start"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            distinct: [],
            preserve: [["schedule", [["active", "1"]]]],
            produce: [["primary-job", [["completed-at/one-c/{end}", "1"]]], ["slot0", [["reserved/one-c", "1"]]]],
            variables: [#{name: "ready", values: ["1", "2"]}, #{name: "start", range: ["1", "2"]}],
            relations: [#{rows: [["1", "1"], ["1", "2"], ["2", "2"]], variables: ["ready", "start"]}]
        }
    ],
    [
        "schedule/two-b/{ready}/{start}",
        #{
            accounts: [["primary-job", "job/two"], ["schedule", "success"], ["slot0", "slot/two/{start}"]],
            consume: [["primary-job", [["ready-at/two-b/{ready}", "1"]]], ["slot0", [["available", "1"]]]],
            derived: [
                #{
                    name: "end",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "start"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            distinct: [],
            preserve: [["schedule", [["active", "1"]]]],
            produce: [["primary-job", [["ready-at/two-c/{end}", "1"]]], ["slot0", [["reserved/two-b", "1"]]]],
            variables: [#{name: "ready", values: ["2"]}, #{name: "start", range: ["2", "2"]}],
            relations: [#{rows: [["2", "2"]], variables: ["ready", "start"]}]
        }
    ],
    [
        "schedule/two-c/{ready}/{start}",
        #{
            accounts: [["primary-job", "job/two"], ["schedule", "success"], ["slot0", "slot/three/{start}"]],
            consume: [["primary-job", [["ready-at/two-c/{ready}", "1"]]], ["slot0", [["available", "1"]]]],
            derived: [
                #{
                    name: "end",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "start"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            distinct: [],
            preserve: [["schedule", [["active", "1"]]]],
            produce: [["primary-job", [["completed-at/two-c/{end}", "1"]]], ["slot0", [["reserved/two-c", "1"]]]],
            variables: [#{name: "ready", values: ["1", "2"]}, #{name: "start", range: ["1", "2"]}],
            relations: [#{rows: [["1", "1"], ["1", "2"], ["2", "2"]], variables: ["ready", "start"]}]
        }
    ]
]
