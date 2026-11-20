[
    [
        "draw/{player}",
        #{
            accounts: [
                ["full/0", "column/0"],
                ["full/1", "column/1"],
                ["full/2", "column/2"],
                ["full/3", "column/3"],
                ["game", "game"],
                ["result", "result"]
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{asset: "height", kind: "balance", role: "full/0"},
                    name: "the-column-is-full/0",
                    right: #{kind: "constant", value: "4"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "height", kind: "balance", role: "full/1"},
                    name: "the-column-is-full/1",
                    right: #{kind: "constant", value: "4"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "height", kind: "balance", role: "full/2"},
                    name: "the-column-is-full/2",
                    right: #{kind: "constant", value: "4"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "height", kind: "balance", role: "full/3"},
                    name: "the-column-is-full/3",
                    right: #{kind: "constant", value: "4"}
                }
            ],
            consume: [["game", [["turn/{player}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["result", [["draw", "1"]]]],
            variables: [#{name: "player", values: ["red", "yellow"]}]
        }
    ]
]
