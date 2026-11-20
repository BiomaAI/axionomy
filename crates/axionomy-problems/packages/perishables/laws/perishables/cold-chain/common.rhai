[
    [
        "age-cold/{batch}",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["world", [["pending-age/{batch}", "1"]]]],
            distinct: [],
            preserve: [["batch", [["at/fridge", "1"], ["fresh", "1"]]], ["world", [["aging", "1"], ["cooling-on", "1"]]]],
            produce: [],
            variables: [#{name: "batch", range: ["0", "5"]}]
        }
    ],
    [
        "age-empty/{batch}",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "fresh", kind: "balance", role: "batch"},
                    name: "no-fresh-stock-to-age",
                    right: #{kind: "constant", value: "0"}
                }
            ],
            consume: [["world", [["pending-age/{batch}", "1"]]]],
            distinct: [],
            preserve: [["world", [["aging", "1"]]]],
            produce: [],
            variables: [#{name: "batch", range: ["0", "5"]}]
        }
    ],
    [
        "age-warm/{batch}/{at}/false",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            computed_preserve: [#{asset: "cooling-off", quantity: #{kind: "variable", name: "refrigerated"}, role: "world"}],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "fresh", kind: "balance", role: "batch"},
                    name: "fresh-stock-is-present",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "heat", kind: "balance", role: "batch"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "thermal-threshold",
                    right: #{kind: "constant", value: "2"}
                }
            ],
            consume: [["world", [["pending-age/{batch}", "1"]]]],
            distinct: [],
            preserve: [["batch", [["at/{at}", "1"]]], ["world", [["aging", "1"]]]],
            produce: [["batch", [["heat", "2"]]]],
            variables: [#{name: "batch", range: ["0", "5"]}, #{name: "at", values: ["warehouse", "fridge"]}],
            relations: [#{gives: ["refrigerated"], rows: [["warehouse", "0"], ["fridge", "1"]], variables: ["at"]}]
        }
    ],
    [
        "age-warm/{batch}/{at}/true",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            computed_consume: [#{asset: "fresh", quantity: #{asset: "fresh", kind: "balance", role: "batch"}, role: "batch"}],
            computed_preserve: [#{asset: "cooling-off", quantity: #{kind: "variable", name: "refrigerated"}, role: "world"}],
            computed_produce: [#{asset: "spoiled", quantity: #{asset: "fresh", kind: "balance", role: "batch"}, role: "batch"}],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "fresh", kind: "balance", role: "batch"},
                    name: "fresh-stock-is-present",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "heat", kind: "balance", role: "batch"},
                    name: "thermal-threshold",
                    right: #{kind: "constant", value: "2"}
                }
            ],
            consume: [["world", [["pending-age/{batch}", "1"]]]],
            distinct: [],
            preserve: [["batch", [["at/{at}", "1"]]], ["world", [["aging", "1"]]]],
            produce: [["batch", [["heat", "2"]]]],
            variables: [#{name: "batch", range: ["0", "5"]}, #{name: "at", values: ["warehouse", "fridge"]}],
            relations: [#{gives: ["refrigerated"], rows: [["warehouse", "0"], ["fridge", "1"]], variables: ["at"]}]
        }
    ],
    [
        "close",
        #{
            accounts: [["world", "world"]],
            computed_consume: [#{asset: "demand", quantity: #{asset: "demand", kind: "balance", role: "world"}, role: "world"}],
            computed_produce: [
                #{
                    asset: "missed-demand",
                    quantity: #{asset: "demand", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/0", kind: "balance", role: "world"},
                    name: "batch-aging-settled/0",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/1", kind: "balance", role: "world"},
                    name: "batch-aging-settled/1",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/2", kind: "balance", role: "world"},
                    name: "batch-aging-settled/2",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/3", kind: "balance", role: "world"},
                    name: "batch-aging-settled/3",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/4", kind: "balance", role: "world"},
                    name: "batch-aging-settled/4",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/5", kind: "balance", role: "world"},
                    name: "batch-aging-settled/5",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "day", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "last-day",
                    right: #{kind: "variable", name: "days"}
                }
            ],
            consume: [["world", [["aging", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["world", [["solved", "1"]]]]
        }
    ],
    [
        "cool",
        #{
            accounts: [["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["world", [["cooling-off", "1"], ["energy", "2"]]]],
            distinct: [],
            preserve: [["world", [["decisions", "1"]]]],
            produce: [["world", [["cooling-on", "1"], ["spent-energy", "2"]]]]
        }
    ],
    [
        "deliver/{batch}/{preserved}/{at}",
        #{
            accounts: [["batch", "batch/{batch}"], ["buyer", "buyer"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "fresh",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "fresh-crates"}
                    },
                    role: "batch"
                },
                #{
                    asset: "preserved",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "preserved-crates"}
                    },
                    role: "batch"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "units"}, right: #{kind: "variable", name: "price"}},
                    role: "buyer"
                },
                #{asset: "fuel", quantity: #{kind: "constant", value: "1"}, role: "world"},
                #{
                    asset: "cold-occupancy",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refrigerated"}
                    },
                    role: "batch"
                }
            ],
            computed_produce: [
                #{asset: "transport-fuel", quantity: #{kind: "constant", value: "1"}, role: "world"},
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "units"}, right: #{kind: "variable", name: "price"}},
                    role: "world"
                },
                #{
                    asset: "cold-space",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refrigerated"}
                    },
                    role: "world"
                }
            ],
            consume: [["world", [["freight", "1"], ["demand", "1"]]]],
            distinct: [],
            preserve: [["batch", [["active", "1"], ["at/{at}", "1"]]], ["world", [["decisions", "1"]]]],
            produce: [["buyer", [["delivered", "1"]]], ["world", [["satisfied-demand", "1"], ["used-freight", "1"]]]],
            variables: [
                #{name: "batch", range: ["0", "5"]},
                #{name: "at", values: ["warehouse", "fridge"]},
                #{name: "preserved", values: ["false", "true"]}
            ],
            relations: [
                #{gives: ["refrigerated"], rows: [["warehouse", "0"], ["fridge", "1"]], variables: ["at"]},
                #{
                    gives: ["price", "fresh-crates", "preserved-crates"],
                    rows: [["false", "3", "1", "0"], ["true", "2", "0", "1"]],
                    variables: ["preserved"]
                }
            ]
        }
    ],
    [
        "discard/{batch}/{at}",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "cold-occupancy",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refrigerated"}
                    },
                    role: "batch"
                }
            ],
            computed_produce: [
                #{
                    asset: "cold-space",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refrigerated"}
                    },
                    role: "world"
                }
            ],
            consume: [["batch", [["spoiled", "1"]]]],
            distinct: [],
            preserve: [["batch", [["active", "1"], ["at/{at}", "1"]]], ["world", [["decisions", "1"]]]],
            produce: [["world", [["disposed", "1"]]]],
            variables: [#{name: "batch", range: ["0", "5"]}, #{name: "at", values: ["warehouse", "fridge"]}],
            relations: [#{gives: ["refrigerated"], rows: [["warehouse", "0"], ["fridge", "1"]], variables: ["at"]}]
        }
    ],
    [
        "end-day",
        #{
            accounts: [["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["world", [["decisions", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [
                [
                    "world",
                    [
                        ["aging", "1"],
                        ["pending-age/0", "1"],
                        ["pending-age/1", "1"],
                        ["pending-age/2", "1"],
                        ["pending-age/3", "1"],
                        ["pending-age/4", "1"],
                        ["pending-age/5", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "generate",
        #{
            accounts: [["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "energy", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    name: "battery-capacity",
                    right: #{kind: "variable", name: "battery"}
                }
            ],
            consume: [["world", [["fuel", "1"]]]],
            distinct: [],
            preserve: [["world", [["decisions", "1"]]]],
            produce: [["world", [["generator-fuel", "1"], ["energy", "3"]]]]
        }
    ],
    [
        "move/{batch}/{from}/{to}",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "cold-space",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "add",
                                left: #{asset: "fresh", kind: "balance", role: "batch"},
                                right: #{asset: "preserved", kind: "balance", role: "batch"}
                            },
                            right: #{asset: "spoiled", kind: "balance", role: "batch"}
                        },
                        right: #{kind: "variable", name: "cold-slots-taken"}
                    },
                    role: "world"
                },
                #{
                    asset: "cold-occupancy",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "add",
                                left: #{asset: "fresh", kind: "balance", role: "batch"},
                                right: #{asset: "preserved", kind: "balance", role: "batch"}
                            },
                            right: #{asset: "spoiled", kind: "balance", role: "batch"}
                        },
                        right: #{kind: "variable", name: "cold-slots-freed"}
                    },
                    role: "batch"
                }
            ],
            computed_produce: [
                #{
                    asset: "cold-occupancy",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "add",
                                left: #{asset: "fresh", kind: "balance", role: "batch"},
                                right: #{asset: "preserved", kind: "balance", role: "batch"}
                            },
                            right: #{asset: "spoiled", kind: "balance", role: "batch"}
                        },
                        right: #{kind: "variable", name: "cold-slots-taken"}
                    },
                    role: "batch"
                },
                #{
                    asset: "cold-space",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "add",
                                left: #{asset: "fresh", kind: "balance", role: "batch"},
                                right: #{asset: "preserved", kind: "balance", role: "batch"}
                            },
                            right: #{asset: "spoiled", kind: "balance", role: "batch"}
                        },
                        right: #{kind: "variable", name: "cold-slots-freed"}
                    },
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{
                            kind: "add",
                            left: #{asset: "fresh", kind: "balance", role: "batch"},
                            right: #{asset: "preserved", kind: "balance", role: "batch"}
                        },
                        right: #{asset: "spoiled", kind: "balance", role: "batch"}
                    },
                    name: "move-a-nonempty-cohort",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["batch", [["at/{from}", "1"]]], ["world", [["labor", "1"]]]],
            distinct: [],
            preserve: [["batch", [["active", "1"]]], ["world", [["decisions", "1"]]]],
            produce: [["batch", [["at/{to}", "1"]]], ["world", [["spent-labor", "1"]]]],
            variables: [
                #{name: "batch", range: ["0", "5"]},
                #{name: "from", values: ["warehouse", "fridge"]},
                #{name: "to", values: ["warehouse", "fridge"]}
            ],
            relations: [
                #{
                    gives: ["cold-slots-freed", "cold-slots-taken"],
                    rows: [["warehouse", "fridge", "0", "1"], ["fridge", "warehouse", "1", "0"]],
                    variables: ["from", "to"]
                }
            ]
        }
    ],
    [
        "open-day/{day}",
        #{
            accounts: [["world", "world"]],
            computed_consume: [
                #{asset: "grid-potential", quantity: #{kind: "variable", name: "grid-supply"}, role: "world"},
                #{asset: "demand", quantity: #{asset: "demand", kind: "balance", role: "world"}, role: "world"},
                #{asset: "labor", quantity: #{asset: "labor", kind: "balance", role: "world"}, role: "world"},
                #{
                    asset: "freight",
                    quantity: #{asset: "freight", kind: "balance", role: "world"},
                    role: "world"
                },
                #{
                    asset: "cooling-on",
                    quantity: #{asset: "cooling-on", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "energy",
                    quantity: #{
                        kind: "minimum",
                        left: #{kind: "variable", name: "grid-supply"},
                        right: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "battery"},
                            right: #{asset: "energy", kind: "balance", role: "world"}
                        }
                    },
                    role: "world"
                },
                #{
                    asset: "spilled-energy",
                    quantity: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "grid-supply"},
                        right: #{
                            kind: "minimum",
                            left: #{kind: "variable", name: "grid-supply"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "battery"},
                                right: #{asset: "energy", kind: "balance", role: "world"}
                            }
                        }
                    },
                    role: "world"
                },
                #{
                    asset: "missed-demand",
                    quantity: #{asset: "demand", kind: "balance", role: "world"},
                    role: "world"
                },
                #{
                    asset: "unused-labor",
                    quantity: #{asset: "labor", kind: "balance", role: "world"},
                    role: "world"
                },
                #{
                    asset: "unused-freight",
                    quantity: #{asset: "freight", kind: "balance", role: "world"},
                    role: "world"
                },
                #{
                    asset: "cooling-off",
                    quantity: #{asset: "cooling-on", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/0", kind: "balance", role: "world"},
                    name: "batch-aging-settled/0",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/1", kind: "balance", role: "world"},
                    name: "batch-aging-settled/1",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/2", kind: "balance", role: "world"},
                    name: "batch-aging-settled/2",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/3", kind: "balance", role: "world"},
                    name: "batch-aging-settled/3",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/4", kind: "balance", role: "world"},
                    name: "batch-aging-settled/4",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-age/5", kind: "balance", role: "world"},
                    name: "batch-aging-settled/5",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "day", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "next-day",
                    right: #{kind: "variable", name: "day"}
                }
            ],
            consume: [
                [
                    "world",
                    [["aging", "1"], ["future-demand", "2"], ["future-labor", "2"], ["future-freight", "2"]]
                ]
            ],
            distinct: [],
            preserve: [],
            produce: [["world", [["decisions", "1"], ["day", "1"], ["demand", "2"], ["labor", "2"], ["freight", "2"]]]]
        }
    ],
    [
        "process/{batch}",
        #{
            accounts: [["batch", "batch/{batch}"], ["world", "world"]],
            consume: [["batch", [["fresh", "1"]]], ["world", [["energy", "1"], ["labor", "1"]]]],
            distinct: [],
            preserve: [["batch", [["active", "1"], ["at/warehouse", "1"]]], ["world", [["decisions", "1"]]]],
            produce: [["batch", [["preserved", "1"]]], ["world", [["spent-energy", "1"], ["spent-labor", "1"]]]],
            variables: [#{name: "batch", range: ["0", "5"]}]
        }
    ],
    [
        "split/{source}/{destination}/{at}",
        #{
            accounts: [["destination", "batch/{destination}"], ["source", "batch/{source}"], ["world", "world"]],
            computed_consume: [
                #{asset: "dormant", quantity: #{kind: "constant", value: "1"}, role: "destination"},
                #{
                    asset: "cold-occupancy",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refrigerated"}
                    },
                    role: "source"
                }
            ],
            computed_produce: [
                #{asset: "active", quantity: #{kind: "constant", value: "1"}, role: "destination"},
                #{asset: "at/{at}", quantity: #{kind: "constant", value: "1"}, role: "destination"},
                #{
                    asset: "heat",
                    quantity: #{asset: "heat", kind: "balance", role: "source"},
                    role: "destination"
                },
                #{
                    asset: "cold-occupancy",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "units"},
                        right: #{kind: "variable", name: "refrigerated"}
                    },
                    role: "destination"
                },
                #{
                    asset: "origin/0",
                    quantity: #{asset: "origin/0", kind: "balance", role: "source"},
                    role: "destination"
                },
                #{
                    asset: "origin/1",
                    quantity: #{asset: "origin/1", kind: "balance", role: "source"},
                    role: "destination"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "fresh", kind: "balance", role: "source"},
                    name: "leave-a-nonempty-parent-cohort",
                    right: #{kind: "add", left: #{kind: "units"}, right: #{kind: "constant", value: "1"}}
                }
            ],
            consume: [["source", [["fresh", "1"]]]],
            distinct: [],
            distinct_variables: [["destination", "source"]],
            preserve: [["source", [["active", "1"], ["at/{at}", "1"]]], ["world", [["decisions", "1"]]]],
            produce: [["destination", [["fresh", "1"]]]],
            variables: [
                #{name: "source", range: ["0", "5"]},
                #{name: "destination", range: ["0", "5"]},
                #{name: "at", values: ["warehouse", "fridge"]}
            ],
            relations: [#{gives: ["refrigerated"], rows: [["warehouse", "0"], ["fridge", "1"]], variables: ["at"]}]
        }
    ]
]
