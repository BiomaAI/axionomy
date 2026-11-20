[
    [
        "begin-observe",
        #{
            accounts: [["actor", "agent"]],
            consume: [["actor", [["planning", "1"], ["sensor", "1"]]]],
            distinct: [],
            preserve: [["actor", [["at/base", "1"]]]],
            produce: [["actor", [["awaiting-observation", "1"], ["used-sensor", "1"]]]]
        }
    ],
    [
        "finish",
        #{
            accounts: [["actor", "agent"]],
            consume: [["actor", [["rescued", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["actor", [["solved", "1"]]]]
        }
    ],
    [
        "instantiate/{truth}/{seed}",
        #{
            accounts: [["nature", "nature"]],
            consume: [["nature", [["unresolved", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["nature", [["truth/{truth}", "1"], ["seed/{seed}", "1"]]]],
            variables: [#{name: "truth", values: ["north", "south"]}, #{name: "seed", range: ["0", "3"]}],
            relations: [
                #{
                    gives: ["weight"],
                    rows: [
                        ["north", "0", "1"],
                        ["north", "1", "1"],
                        ["north", "2", "1"],
                        ["north", "3", "1"],
                        ["south", "0", "1"],
                        ["south", "1", "1"],
                        ["south", "2", "1"],
                        ["south", "3", "1"]
                    ],
                    variables: ["truth", "seed"]
                }
            ]
        }
    ],
    [
        "move/base/{to}",
        #{
            accounts: [["actor", "agent"]],
            consume: [["actor", [["planning", "1"], ["at/base", "1"], ["energy", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["actor", [["committed", "1"], ["spent-energy", "1"], ["at/{to}", "1"]]]],
            variables: [#{name: "to", values: ["north", "south"]}]
        }
    ],
    [
        "rescue/{location}",
        #{
            accounts: [["actor", "agent"], ["nature", "nature"]],
            consume: [["actor", [["committed", "1"], ["energy", "1"]]]],
            distinct: [],
            preserve: [["actor", [["at/{location}", "1"]]], ["nature", [["truth/{location}", "1"]]]],
            produce: [["actor", [["rescued", "1"], ["spent-energy", "1"]]]],
            variables: [#{name: "location", values: ["north", "south"]}]
        }
    ],
    [
        "resolve-observation/{truth}/{seed}/{report}",
        #{
            accounts: [["actor", "agent"], ["nature", "nature"]],
            consume: [["actor", [["awaiting-observation", "1"]]], ["nature", [["seed/{seed}", "1"]]]],
            derived: [
                #{
                    name: "next",
                    value: #{
                        kind: "subtract",
                        left: #{
                            kind: "add",
                            left: #{kind: "variable", name: "seed"},
                            right: #{kind: "constant", value: "1"}
                        },
                        right: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "4"},
                                kind: "mul_div",
                                left: #{
                                    kind: "add",
                                    left: #{kind: "variable", name: "seed"},
                                    right: #{kind: "constant", value: "1"}
                                },
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "4"}
                        }
                    }
                }
            ],
            distinct: [],
            preserve: [["actor", [["at/base", "1"]]], ["nature", [["truth/{truth}", "1"]]]],
            produce: [["actor", [["planning", "1"], ["belief/{report}", "1"]]], ["nature", [["seed/{next}", "1"]]]],
            variables: [
                #{name: "truth", values: ["north", "south"]},
                #{name: "seed", range: ["0", "3"]},
                #{name: "report", values: ["east", "north", "south"]}
            ],
            relations: [
                #{
                    rows: [
                        ["north", "0", "south"],
                        ["north", "1", "north"],
                        ["north", "2", "north"],
                        ["north", "3", "north"],
                        ["south", "0", "east"],
                        ["south", "1", "south"],
                        ["south", "2", "south"],
                        ["south", "3", "south"]
                    ],
                    variables: ["truth", "seed", "report"]
                }
            ]
        }
    ]
]
