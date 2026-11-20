[
    [
        "finish-clinical/{day}",
        #{
            accounts: [["medic", "agent/medic"], ["scout", "agent/scout"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "clinical-count",
                    quantity: #{asset: "clinical-count", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "next-clinical",
                    quantity: #{kind: "variable", name: "days-to-next-pulse"},
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-mission-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "next-clinical", kind: "balance", role: "world"},
                    name: "scheduled-pulse",
                    right: #{kind: "variable", name: "day"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "clinical-count", kind: "balance", role: "world"},
                    name: "all-casualties-assessed",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [],
            variables: [#{name: "day", range: ["8", "24"]}],
            relations: [
                #{
                    gives: ["days-to-next-pulse"],
                    rows: [["8", "8"], ["16", "8"], ["24", "100"]],
                    variables: ["day"]
                }
            ]
        }
    ]
]
