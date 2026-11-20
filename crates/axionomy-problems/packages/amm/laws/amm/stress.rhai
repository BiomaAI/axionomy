[
    [
        "close-market",
        #{
            accounts: [
                ["participant/adaptive-lp", "actor/adaptive-lp"],
                ["participant/arbitrageur", "actor/arbitrageur"],
                ["participant/factory", "actor/factory"],
                ["participant/founder", "actor/founder"],
                ["participant/generator", "actor/generator"],
                ["participant/household", "actor/household"],
                ["participant/speculator", "actor/speculator"],
                ["participant/whale", "actor/whale"],
                ["pool", "pool"],
                ["world", "world"]
            ],
            computed_consume: [
                #{
                    asset: "obligation/generator",
                    quantity: #{asset: "obligation/generator", kind: "balance", role: "participant/generator"},
                    role: "participant/generator"
                },
                #{
                    asset: "production-allowance",
                    quantity: #{asset: "production-allowance", kind: "balance", role: "participant/generator"},
                    role: "participant/generator"
                },
                #{
                    asset: "need/factory",
                    quantity: #{asset: "need/factory", kind: "balance", role: "participant/factory"},
                    role: "participant/factory"
                },
                #{
                    asset: "obligation/factory",
                    quantity: #{asset: "obligation/factory", kind: "balance", role: "participant/factory"},
                    role: "participant/factory"
                },
                #{
                    asset: "need/household",
                    quantity: #{asset: "need/household", kind: "balance", role: "participant/household"},
                    role: "participant/household"
                },
                #{
                    asset: "income-due",
                    quantity: #{asset: "income-due", kind: "balance", role: "participant/household"},
                    role: "participant/household"
                }
            ],
            computed_produce: [
                #{
                    asset: "defaulted-obligation/generator",
                    quantity: #{asset: "obligation/generator", kind: "balance", role: "participant/generator"},
                    role: "participant/generator"
                },
                #{
                    asset: "missed-need/factory",
                    quantity: #{asset: "need/factory", kind: "balance", role: "participant/factory"},
                    role: "participant/factory"
                },
                #{
                    asset: "defaulted-obligation/factory",
                    quantity: #{asset: "obligation/factory", kind: "balance", role: "participant/factory"},
                    role: "participant/factory"
                },
                #{
                    asset: "missed-need/household",
                    quantity: #{asset: "need/household", kind: "balance", role: "participant/household"},
                    role: "participant/household"
                },
                #{
                    asset: "missed-income",
                    quantity: #{asset: "income-due", kind: "balance", role: "participant/household"},
                    role: "participant/household"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{asset: "market-active", kind: "balance", role: "world"},
                    name: "market-remains-active",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-period-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [
                ["participant/adaptive-lp", [["done", "1"], ["market-open", "1"]]],
                ["participant/arbitrageur", [["done", "1"], ["market-open", "1"]]],
                ["participant/factory", [["done", "1"], ["market-open", "1"]]],
                ["participant/founder", [["done", "1"], ["market-open", "1"]]],
                ["participant/generator", [["done", "1"], ["market-open", "1"]]],
                ["participant/household", [["done", "1"], ["market-open", "1"]]],
                ["participant/speculator", [["done", "1"], ["market-open", "1"]]],
                ["participant/whale", [["done", "1"], ["market-open", "1"]]],
                ["pool", [["market-open", "1"]]],
                ["world", [["at-day/11", "1"], ["market-active", "1"]]]
            ],
            distinct: [],
            preserve: [],
            produce: [["world", [["market-ended", "1"]]]]
        }
    ]
]
