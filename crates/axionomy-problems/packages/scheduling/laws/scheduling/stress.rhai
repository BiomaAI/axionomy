[
    [
        "advance/{to}",
        #{
            accounts: [["schedule", "success"]],
            computed_consume: [
                #{
                    asset: "time-remaining",
                    quantity: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "to"},
                        right: #{asset: "now", kind: "balance", role: "schedule"}
                    },
                    role: "schedule"
                }
            ],
            computed_produce: [
                #{
                    asset: "now",
                    quantity: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "to"},
                        right: #{asset: "now", kind: "balance", role: "schedule"}
                    },
                    role: "schedule"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-scheduling-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "now", kind: "balance", role: "schedule"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "clock-moves-forward",
                    right: #{kind: "variable", name: "to"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{kind: "variable", name: "to"},
                    name: "cannot-skip-a-scheduled-disruption",
                    right: #{
                        kind: "add",
                        left: #{kind: "variable", name: "fault-at"},
                        right: #{
                            kind: "multiply",
                            left: #{asset: "fault-occurred", kind: "balance", role: "schedule"},
                            right: #{kind: "constant", value: "24"}
                        }
                    }
                }
            ],
            consume: [],
            distinct: [],
            preserve: [["schedule", [["active", "1"]]]],
            produce: [],
            variables: [#{name: "to", range: ["1", "24"]}]
        }
    ]
]
