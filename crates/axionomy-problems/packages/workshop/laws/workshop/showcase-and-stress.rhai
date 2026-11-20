[
    [
        "accept/0",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["idle", "1"], ["contract/0", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [["shop", [["spent-time", "1"], ["raw/0", "1"]]]]
        }
    ],
    [
        "accept/1",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["idle", "1"], ["contract/1", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [["shop", [["spent-time", "1"], ["raw/1", "1"]]]]
        }
    ],
    [
        "accept/2",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["idle", "1"], ["contract/2", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [["shop", [["spent-time", "1"], ["raw/2", "1"]]]]
        }
    ],
    [
        "assemble/0",
        #{
            accounts: [["shop", "workshop"]],
            consume: [
                [
                    "shop",
                    [
                        ["time", "1"],
                        ["labor", "1"],
                        ["durability", "1"],
                        ["prepared/0", "1"],
                        ["frame", "1"],
                        ["panel", "2"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "1"],
                        ["assembled/0", "1"],
                        ["chair", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "assemble/1",
        #{
            accounts: [["shop", "workshop"]],
            consume: [
                [
                    "shop",
                    [
                        ["time", "2"],
                        ["labor", "2"],
                        ["durability", "2"],
                        ["prepared/1", "1"],
                        ["frame", "1"],
                        ["panel", "4"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "2"],
                        ["spent-labor", "2"],
                        ["wear", "2"],
                        ["assembled/1", "1"],
                        ["table", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "assemble/2",
        #{
            accounts: [["shop", "workshop"]],
            consume: [
                [
                    "shop",
                    [
                        ["time", "1"],
                        ["labor", "1"],
                        ["durability", "1"],
                        ["prepared/2", "1"],
                        ["frame", "1"],
                        ["panel", "2"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "1"],
                        ["assembled/2", "1"],
                        ["chair", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "counterfeit-chair",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["wood", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [["shop", [["spent-time", "1"], ["chair", "2"]]]]
        }
    ],
    [
        "deliver/0",
        #{
            accounts: [["customer", "customer/0"], ["shop", "workshop"]],
            consume: [["customer", [["credit", "8"]]], ["shop", [["time", "1"], ["assembled/0", "1"], ["chair", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                ["customer", [["chair", "1"]]],
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["idle", "1"],
                        ["fulfilled/0", "1"],
                        ["completed", "1"],
                        ["credit", "8"]
                    ]
                ]
            ]
        }
    ],
    [
        "deliver/1",
        #{
            accounts: [["customer", "customer/1"], ["shop", "workshop"]],
            consume: [["customer", [["credit", "21"]]], ["shop", [["time", "1"], ["assembled/1", "1"], ["table", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                ["customer", [["table", "1"]]],
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["idle", "1"],
                        ["fulfilled/1", "1"],
                        ["completed", "1"],
                        ["credit", "21"]
                    ]
                ]
            ]
        }
    ],
    [
        "deliver/2",
        #{
            accounts: [["customer", "customer/2"], ["shop", "workshop"]],
            consume: [["customer", [["credit", "14"]]], ["shop", [["time", "1"], ["assembled/2", "1"], ["chair", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                ["customer", [["chair", "1"]]],
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["idle", "1"],
                        ["fulfilled/2", "1"],
                        ["completed", "1"],
                        ["credit", "14"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/0/false/false",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["labor", "1"], ["durability", "2"], ["raw/0", "1"], ["wood", "4"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "2"],
                        ["prepared/0", "1"],
                        ["frame", "1"],
                        ["panel", "2"],
                        ["offcut", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/0/false/true",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["labor", "1"], ["durability", "1"], ["raw/0", "1"], ["wood", "3"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["upgraded", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "1"],
                        ["prepared/0", "1"],
                        ["frame", "1"],
                        ["panel", "2"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/0/true/false",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "3"], ["labor", "2"], ["durability", "1"], ["raw/0", "1"], ["wood", "3"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "3"],
                        ["spent-labor", "2"],
                        ["wear", "1"],
                        ["prepared/0", "1"],
                        ["frame", "1"],
                        ["panel", "2"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/1/false/false",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["labor", "1"], ["durability", "2"], ["raw/1", "1"], ["wood", "6"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "2"],
                        ["prepared/1", "1"],
                        ["frame", "1"],
                        ["panel", "4"],
                        ["offcut", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/1/false/true",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["labor", "1"], ["durability", "1"], ["raw/1", "1"], ["wood", "5"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["upgraded", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "1"],
                        ["prepared/1", "1"],
                        ["frame", "1"],
                        ["panel", "4"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/1/true/false",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "3"], ["labor", "2"], ["durability", "1"], ["raw/1", "1"], ["wood", "5"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "3"],
                        ["spent-labor", "2"],
                        ["wear", "1"],
                        ["prepared/1", "1"],
                        ["frame", "1"],
                        ["panel", "4"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/2/false/false",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["labor", "1"], ["durability", "2"], ["raw/2", "1"], ["wood", "4"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "2"],
                        ["prepared/2", "1"],
                        ["frame", "1"],
                        ["panel", "2"],
                        ["offcut", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/2/false/true",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "1"], ["labor", "1"], ["durability", "1"], ["raw/2", "1"], ["wood", "3"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["upgraded", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "1"],
                        ["spent-labor", "1"],
                        ["wear", "1"],
                        ["prepared/2", "1"],
                        ["frame", "1"],
                        ["panel", "2"]
                    ]
                ]
            ]
        }
    ],
    [
        "prepare/2/true/false",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "3"], ["labor", "2"], ["durability", "1"], ["raw/2", "1"], ["wood", "3"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [
                [
                    "shop",
                    [
                        ["spent-time", "3"],
                        ["spent-labor", "2"],
                        ["wear", "1"],
                        ["prepared/2", "1"],
                        ["frame", "1"],
                        ["panel", "2"]
                    ]
                ]
            ]
        }
    ],
    [
        "recycle",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "2"], ["labor", "1"], ["offcut", "2"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["idle", "1"]]]],
            produce: [["shop", [["spent-time", "2"], ["spent-labor", "1"], ["wood", "1"], ["waste", "1"]]]]
        }
    ],
    [
        "repair",
        #{
            accounts: [["shop", "workshop"]],
            computed_consume: [#{asset: "wear", quantity: #{asset: "wear", kind: "balance", role: "shop"}, role: "shop"}],
            computed_produce: [#{asset: "durability", quantity: #{asset: "wear", kind: "balance", role: "shop"}, role: "shop"}],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "wear", kind: "balance", role: "shop"},
                    name: "tool-has-wear",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["shop", [["time", "2"], ["labor", "1"], ["repair-kit", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [["shop", [["spent-time", "2"], ["spent-labor", "1"], ["used-repair-kit", "1"]]]]
        }
    ],
    [
        "upgrade",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["time", "4"], ["labor", "3"], ["upgrade-kit", "1"], ["tool-potential", "2"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["idle", "1"]]]],
            produce: [["shop", [["spent-time", "4"], ["spent-labor", "3"], ["upgraded", "1"], ["durability", "2"]]]]
        }
    ]
]
