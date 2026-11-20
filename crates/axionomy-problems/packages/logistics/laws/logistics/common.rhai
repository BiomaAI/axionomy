[
    [
        "arrive/{route}/{outcome}",
        #{
            accounts: [["nature", "nature"], ["vehicle", "vehicle"]],
            computed_consume: [
                #{asset: "fuel", quantity: #{kind: "variable", name: "fuel"}, role: "vehicle"},
                #{asset: "time-remaining", quantity: #{kind: "variable", name: "time"}, role: "vehicle"}
            ],
            computed_produce: [
                #{asset: "spent-fuel", quantity: #{kind: "variable", name: "fuel"}, role: "vehicle"},
                #{asset: "elapsed-time", quantity: #{kind: "variable", name: "time"}, role: "vehicle"}
            ],
            consume: [["nature", [["outcome/{route}/{outcome}", "1"]]], ["vehicle", [["traveling/{route}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["nature", [["weather-ready", "1"]]], ["vehicle", [["at/{to}", "1"]]]],
            variables: [
                #{
                    name: "route",
                    values: [
                        "direct-out",
                        "direct-back",
                        "safe-out-first",
                        "safe-out-second",
                        "safe-back-first",
                        "safe-back-second"
                    ]
                },
                #{name: "outcome", values: ["clear", "delayed"]}
            ],
            relations: [
                #{
                    gives: ["from", "to"],
                    rows: [
                        ["direct-out", "depot", "customer"],
                        ["direct-back", "customer", "depot"],
                        ["safe-out-first", "depot", "junction"],
                        ["safe-out-second", "junction", "customer"],
                        ["safe-back-first", "customer", "junction"],
                        ["safe-back-second", "junction", "depot"]
                    ],
                    variables: ["route"]
                },
                #{
                    gives: ["fuel", "time"],
                    rows: [
                        ["direct-out", "clear", "2", "2"],
                        ["direct-out", "delayed", "2", "5"],
                        ["direct-back", "clear", "2", "2"],
                        ["direct-back", "delayed", "2", "5"],
                        ["safe-out-first", "clear", "1", "1"],
                        ["safe-out-first", "delayed", "1", "2"],
                        ["safe-out-second", "clear", "1", "1"],
                        ["safe-out-second", "delayed", "1", "2"],
                        ["safe-back-first", "clear", "1", "1"],
                        ["safe-back-first", "delayed", "1", "2"],
                        ["safe-back-second", "clear", "1", "1"],
                        ["safe-back-second", "delayed", "1", "2"]
                    ],
                    variables: ["route", "outcome"]
                }
            ]
        }
    ],
    [
        "deliver/{order}",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle"]],
            consume: [["order", [["in-transit", "1"]]], ["vehicle", [["cargo-occupied", "1"], ["package/{order}", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["at/customer", "1"]]]],
            produce: [["order", [["delivered", "1"], ["package/{order}", "1"]]], ["vehicle", [["cargo-space", "1"]]]],
            variables: [#{name: "order", values: ["a", "b", "c", "d"]}]
        }
    ],
    [
        "depart/{route}",
        #{
            accounts: [["vehicle", "vehicle"]],
            consume: [["vehicle", [["at/{from}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["vehicle", [["traveling/{route}", "1"]]]],
            variables: [
                #{
                    name: "route",
                    values: [
                        "direct-out",
                        "direct-back",
                        "safe-out-first",
                        "safe-out-second",
                        "safe-back-first",
                        "safe-back-second"
                    ]
                }
            ],
            relations: [
                #{
                    gives: ["from", "to"],
                    rows: [
                        ["direct-out", "depot", "customer"],
                        ["direct-back", "customer", "depot"],
                        ["safe-out-first", "depot", "junction"],
                        ["safe-out-second", "junction", "customer"],
                        ["safe-back-first", "customer", "junction"],
                        ["safe-back-second", "junction", "depot"]
                    ],
                    variables: ["route"]
                }
            ]
        }
    ],
    [
        "load/{order}",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle"]],
            consume: [["order", [["waiting", "1"], ["package/{order}", "1"]]], ["vehicle", [["cargo-space", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["at/depot", "1"]]]],
            produce: [["order", [["in-transit", "1"]]], ["vehicle", [["cargo-occupied", "1"], ["package/{order}", "1"]]]],
            variables: [#{name: "order", values: ["a", "b", "c", "d"]}]
        }
    ],
    [
        "refuel",
        #{
            accounts: [["fuel-station", "fuel-station"], ["vehicle", "vehicle"]],
            computed_consume: [
                #{
                    asset: "money",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refuel-amount"}
                    },
                    role: "vehicle"
                },
                #{
                    asset: "fuel",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refuel-amount"}
                    },
                    role: "fuel-station"
                }
            ],
            computed_produce: [
                #{
                    asset: "fuel",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refuel-amount"}
                    },
                    role: "vehicle"
                },
                #{
                    asset: "money",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refuel-amount"}
                    },
                    role: "fuel-station"
                }
            ],
            consume: [],
            distinct: [],
            preserve: [["vehicle", [["at/depot", "1"]]]],
            produce: []
        }
    ],
    [
        "repair/{route}",
        #{
            accounts: [["nature", "nature"], ["vehicle", "vehicle"]],
            consume: [["nature", [["outcome/{route}/breakdown", "1"]]], ["vehicle", [["time-remaining", "2"]]]],
            distinct: [],
            preserve: [["vehicle", [["traveling/{route}", "1"], ["repair-tool", "1"]]]],
            produce: [["nature", [["weather-ready", "1"]]], ["vehicle", [["elapsed-time", "2"]]]],
            variables: [
                #{
                    name: "route",
                    values: [
                        "direct-out",
                        "direct-back",
                        "safe-out-first",
                        "safe-out-second",
                        "safe-back-first",
                        "safe-back-second"
                    ]
                }
            ]
        }
    ],
    [
        "resolve/{route}/{outcome}",
        #{
            accounts: [["nature", "nature"], ["vehicle", "vehicle"]],
            consume: [["nature", [["weather-ready", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["traveling/{route}", "1"]]]],
            produce: [["nature", [["outcome/{route}/{outcome}", "1"]]]],
            variables: [
                #{
                    name: "route",
                    values: [
                        "direct-out",
                        "direct-back",
                        "safe-out-first",
                        "safe-out-second",
                        "safe-back-first",
                        "safe-back-second"
                    ]
                },
                #{name: "outcome", values: ["clear", "delayed", "breakdown"]}
            ],
            relations: [
                #{
                    gives: ["weight"],
                    rows: [
                        ["direct-out", "clear", "2"],
                        ["direct-out", "delayed", "1"],
                        ["direct-out", "breakdown", "1"],
                        ["direct-back", "clear", "2"],
                        ["direct-back", "delayed", "1"],
                        ["direct-back", "breakdown", "1"],
                        ["safe-out-first", "clear", "8"],
                        ["safe-out-first", "delayed", "1"],
                        ["safe-out-first", "breakdown", "1"],
                        ["safe-out-second", "clear", "8"],
                        ["safe-out-second", "delayed", "1"],
                        ["safe-out-second", "breakdown", "1"],
                        ["safe-back-first", "clear", "8"],
                        ["safe-back-first", "delayed", "1"],
                        ["safe-back-first", "breakdown", "1"],
                        ["safe-back-second", "clear", "8"],
                        ["safe-back-second", "delayed", "1"],
                        ["safe-back-second", "breakdown", "1"]
                    ],
                    variables: ["route", "outcome"]
                }
            ]
        }
    ]
]
