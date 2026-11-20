[
    [
        "finish",
        #{
            accounts: [["goal/0", "cell/11"], ["puzzle", "success"]],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "crate/0", kind: "balance", role: "goal/0"},
                    name: "goal-holds-a-crate/0",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["puzzle", [["active", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["puzzle", [["solved", "1"]]]]
        }
    ],
    [
        "move/{from}/{to}",
        #{
            accounts: [["from", "cell/{from}"], ["puzzle", "success"], ["to", "cell/{to}"]],
            consume: [["from", [["player", "1"]]], ["to", [["empty", "1"]]]],
            distinct: [],
            preserve: [["puzzle", [["active", "1"]]]],
            produce: [["from", [["empty", "1"]]], ["to", [["player", "1"]]]],
            variables: [
                #{name: "from", values: ["8", "9", "10", "11", "12"]},
                #{name: "to", values: ["8", "9", "10", "11", "12"]}
            ],
            relations: [
                #{
                    rows: [
                        ["8", "9"],
                        ["9", "8"],
                        ["9", "10"],
                        ["10", "9"],
                        ["10", "11"],
                        ["11", "10"],
                        ["11", "12"],
                        ["12", "11"]
                    ],
                    variables: ["from", "to"]
                }
            ]
        }
    ],
    [
        "push/{behind}/{crate-at}/{to}/{crate}",
        #{
            accounts: [
                ["from", "cell/{behind}"],
                ["middle", "cell/{crate-at}"],
                ["puzzle", "success"],
                ["to", "cell/{to}"]
            ],
            consume: [["from", [["player", "1"]]], ["middle", [["crate/{crate}", "1"]]], ["to", [["empty", "1"]]]],
            distinct: [],
            preserve: [["puzzle", [["active", "1"]]]],
            produce: [["from", [["empty", "1"]]], ["middle", [["player", "1"]]], ["to", [["crate/{crate}", "1"]]]],
            variables: [
                #{name: "behind", values: ["8", "9", "10", "11", "12"]},
                #{name: "crate-at", values: ["8", "9", "10", "11", "12"]},
                #{name: "to", values: ["8", "9", "10", "11", "12"]},
                #{name: "crate", values: ["0"]}
            ],
            relations: [
                #{
                    rows: [
                        ["8", "9", "10"],
                        ["10", "9", "8"],
                        ["9", "10", "11"],
                        ["11", "10", "9"],
                        ["10", "11", "12"],
                        ["12", "11", "10"]
                    ],
                    variables: ["behind", "crate-at", "to"]
                }
            ]
        }
    ]
]
