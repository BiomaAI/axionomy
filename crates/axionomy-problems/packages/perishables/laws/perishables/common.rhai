[
    [
        "advance/{from}/{to}",
        #{
            consume: [["world", [["now/{from}", "1"], ["before/{to}", "1"]]]],
            distinct: [],
            preserve: [["world", [["world-identity", "1"], ["active", "1"]]]],
            produce: [["world", [["now/{to}", "1"], ["reached/{to}", "1"]]]],
            variables: [
                #{name: "from", values: ["harvest", "ambient-expiry", "warmed-expiry", "cold-expiry"]},
                #{name: "to", values: ["harvest", "ambient-expiry", "warmed-expiry", "cold-expiry"]}
            ],
            relations: [
                #{
                    rows: [
                        ["harvest", "ambient-expiry"],
                        ["ambient-expiry", "warmed-expiry"],
                        ["warmed-expiry", "cold-expiry"]
                    ],
                    variables: ["from", "to"]
                }
            ]
        }
    ],
    [
        "eat/{cohort}/{exposure}",
        #{
            computed_preserve: [
                #{asset: "ambient", quantity: #{kind: "variable", name: "ambient"}, role: "holder"},
                #{asset: "cold", quantity: #{kind: "variable", name: "cold"}, role: "holder"},
                #{asset: "powered", quantity: #{kind: "variable", name: "powered"}, role: "holder"},
                #{asset: "unpowered", quantity: #{kind: "variable", name: "unpowered"}, role: "holder"}
            ],
            consume: [["holder", [["claim/{cohort}", "1"]]]],
            distinct: [["cohort", "holder"], ["cohort", "world"], ["holder", "world"]],
            preserve: [
                ["cohort", [["cohort-identity/{cohort}", "1"], ["fresh/{cohort}/{exposure}/{due}", "1"]]],
                ["holder", [["location-identity/{storage}", "1"]]],
                ["world", [["before/{due}", "1"], ["world-identity", "1"], ["active", "1"]]]
            ],
            produce: [["holder", [["consumed/{cohort}", "1"]]]],
            variables: [
                #{name: "cohort", values: ["ambient", "refrigerated"]},
                #{name: "exposure", values: ["ambient", "warmed-after-outage", "cold"]}
            ],
            relations: [
                #{
                    gives: ["storage", "due", "ambient", "cold", "powered", "unpowered"],
                    rows: [
                        ["ambient", "ambient", "warehouse", "ambient-expiry", "1", "0", "0", "0"],
                        ["refrigerated", "warmed-after-outage", "fridge", "warmed-expiry", "1", "0", "0", "1"],
                        ["refrigerated", "cold", "fridge", "cold-expiry", "0", "1", "1", "0"]
                    ],
                    variables: ["cohort", "exposure"]
                }
            ]
        }
    ],
    [
        "finish",
        #{
            consume: [["world", [["active", "1"]]]],
            distinct: [
                ["destination-cohort", "destination-storage"],
                ["destination-cohort", "source-cohort"],
                ["destination-cohort", "source-storage"],
                ["destination-cohort", "world"],
                ["destination-storage", "source-cohort"],
                ["destination-storage", "source-storage"],
                ["destination-storage", "world"],
                ["source-cohort", "source-storage"],
                ["source-cohort", "world"],
                ["source-storage", "world"]
            ],
            preserve: [
                ["destination-cohort", [["cohort-identity/refrigerated", "1"], ["rotten/refrigerated", "1"]]],
                ["destination-storage", [["location-identity/fridge", "1"]]],
                ["source-cohort", [["cohort-identity/ambient", "1"], ["rotten/ambient", "1"]]],
                ["source-storage", [["location-identity/warehouse", "1"]]],
                ["world", [["world-identity", "1"], ["now/cold-expiry", "1"]]]
            ],
            produce: [["world", [["solved", "1"]]]]
        }
    ],
    [
        "lose-power",
        #{
            consume: [
                ["cohort", [["fresh/refrigerated/cold/cold-expiry", "1"]]],
                ["storage", [["cold", "1"], ["powered", "1"]]]
            ],
            distinct: [["cohort", "storage"], ["cohort", "world"], ["storage", "world"]],
            preserve: [
                ["cohort", [["cohort-identity/refrigerated", "1"]]],
                ["storage", [["location-identity/fridge", "1"]]],
                [
                    "world",
                    [
                        ["reached/ambient-expiry", "1"],
                        ["before/warmed-expiry", "1"],
                        ["world-identity", "1"],
                        ["active", "1"]
                    ]
                ]
            ],
            produce: [
                ["cohort", [["fresh/refrigerated/warmed-after-outage/warmed-expiry", "1"]]],
                ["storage", [["ambient", "1"], ["unpowered", "1"]]]
            ]
        }
    ],
    [
        "move-to-fridge",
        #{
            consume: [["destination-storage", [["cooling-energy", "1"]]], ["source-storage", [["claim/ambient", "1"]]]],
            distinct: [
                ["destination-cohort", "destination-storage"],
                ["destination-cohort", "source-cohort"],
                ["destination-cohort", "source-storage"],
                ["destination-cohort", "world"],
                ["destination-storage", "source-cohort"],
                ["destination-storage", "source-storage"],
                ["destination-storage", "world"],
                ["source-cohort", "source-storage"],
                ["source-cohort", "world"],
                ["source-storage", "world"]
            ],
            preserve: [
                [
                    "destination-cohort",
                    [["cohort-identity/refrigerated", "1"], ["fresh/refrigerated/cold/cold-expiry", "1"]]
                ],
                ["destination-storage", [["location-identity/fridge", "1"], ["cold", "1"], ["powered", "1"]]],
                [
                    "source-cohort",
                    [["cohort-identity/ambient", "1"], ["fresh/ambient/ambient/ambient-expiry", "1"]]
                ],
                ["source-storage", [["location-identity/warehouse", "1"], ["ambient", "1"]]],
                ["world", [["before/ambient-expiry", "1"], ["world-identity", "1"], ["active", "1"]]]
            ],
            produce: [["destination-storage", [["claim/refrigerated", "1"], ["spent-cooling-energy", "1"]]]]
        }
    ],
    [
        "seal-storage-plan",
        #{
            consume: [["world", [["planning", "1"]]]],
            distinct: [
                ["destination-cohort", "destination-storage"],
                ["destination-cohort", "source-cohort"],
                ["destination-cohort", "source-storage"],
                ["destination-cohort", "world"],
                ["destination-storage", "source-cohort"],
                ["destination-storage", "source-storage"],
                ["destination-storage", "world"],
                ["source-cohort", "source-storage"],
                ["source-cohort", "world"],
                ["source-storage", "world"]
            ],
            preserve: [
                [
                    "destination-cohort",
                    [["cohort-identity/refrigerated", "1"], ["fresh/refrigerated/cold/cold-expiry", "1"]]
                ],
                ["destination-storage", [["location-identity/fridge", "1"], ["cold", "1"], ["powered", "1"]]],
                ["source-cohort", [["cohort-identity/ambient", "1"], ["rotten/ambient", "1"]]],
                ["source-storage", [["location-identity/warehouse", "1"]]],
                ["world", [["world-identity", "1"], ["now/ambient-expiry", "1"], ["active", "1"]]]
            ],
            produce: [["world", [["plan-solved", "1"]]]]
        }
    ],
    [
        "spoil/{cohort}/{exposure}",
        #{
            computed_preserve: [
                #{asset: "ambient", quantity: #{kind: "variable", name: "ambient"}, role: "storage"},
                #{asset: "cold", quantity: #{kind: "variable", name: "cold"}, role: "storage"},
                #{asset: "powered", quantity: #{kind: "variable", name: "powered"}, role: "storage"},
                #{asset: "unpowered", quantity: #{kind: "variable", name: "unpowered"}, role: "storage"}
            ],
            consume: [["cohort", [["fresh/{cohort}/{exposure}/{due}", "1"]]]],
            distinct: [["cohort", "storage"], ["cohort", "world"], ["storage", "world"]],
            preserve: [
                ["cohort", [["cohort-identity/{cohort}", "1"]]],
                ["storage", [["location-identity/{storage}", "1"]]],
                ["world", [["reached/{due}", "1"], ["world-identity", "1"], ["active", "1"]]]
            ],
            produce: [["cohort", [["rotten/{cohort}", "1"]]]],
            variables: [
                #{name: "cohort", values: ["ambient", "refrigerated"]},
                #{name: "exposure", values: ["ambient", "warmed-after-outage", "cold"]}
            ],
            relations: [
                #{
                    gives: ["storage", "due", "ambient", "cold", "powered", "unpowered"],
                    rows: [
                        ["ambient", "ambient", "warehouse", "ambient-expiry", "1", "0", "0", "0"],
                        ["refrigerated", "warmed-after-outage", "fridge", "warmed-expiry", "1", "0", "0", "1"],
                        ["refrigerated", "cold", "fridge", "cold-expiry", "0", "1", "1", "0"]
                    ],
                    variables: ["cohort", "exposure"]
                }
            ]
        }
    ]
]
