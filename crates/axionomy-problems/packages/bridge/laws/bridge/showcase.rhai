[
    [
        "finish",
        #{
            accounts: [["agent-a", "agent/a"], ["agent-b", "agent/b"], ["bridge", "bridge"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["bridge", [["active", "1"]]]],
            distinct: [],
            preserve: [
                ["agent-a", [["crossed", "1"], ["completed-trip", "1"]]],
                ["agent-b", [["crossed", "1"], ["completed-trip", "1"]]]
            ],
            produce: [["bridge", [["solved", "1"]]]]
        }
    ]
]
