[
    [
        "arrive/{van}/{from}/{to}",
        #{
            accounts: [["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                },
                #{
                    asset: "departed-at",
                    quantity: #{asset: "departed-at", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                },
                #{asset: "on-main-road", quantity: #{kind: "variable", name: "trunk-lane"}, role: "vehicle"}
            ],
            computed_produce: [#{asset: "road-space", quantity: #{kind: "variable", name: "trunk-lane"}, role: "world"}],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "activity-duration-elapsed",
                    right: #{asset: "ready-at", kind: "balance", role: "vehicle"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "awaiting-outcome", kind: "balance", role: "vehicle"},
                    name: "nature-has-resolved-the-trip",
                    right: #{kind: "constant", value: "0"}
                }
            ],
            consume: [["vehicle", [["travelling/{from}/{to}", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["vehicle", [["ready", "1"], ["at/{to}", "1"]]]],
            variables: [
                #{name: "van", values: ["cold", "freight"]},
                #{name: "from", values: ["depot", "hub", "north", "south", "east"]},
                #{name: "to", values: ["depot", "hub", "north", "south", "east"]}
            ],
            relations: [
                #{
                    gives: ["fuel", "trunk-lane"],
                    rows: [
                        ["depot", "hub", "1", "1"],
                        ["hub", "depot", "1", "1"],
                        ["hub", "north", "1", "0"],
                        ["north", "hub", "1", "0"],
                        ["hub", "south", "1", "0"],
                        ["south", "hub", "1", "0"],
                        ["north", "east", "1", "0"],
                        ["east", "north", "1", "0"],
                        ["south", "east", "1", "0"],
                        ["east", "south", "1", "0"],
                        ["depot", "north", "2", "0"],
                        ["north", "depot", "2", "0"]
                    ],
                    variables: ["from", "to"]
                }
            ]
        }
    ],
    [
        "close",
        #{
            accounts: [["other", "vehicle/freight"], ["vehicle", "vehicle/cold"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "resolved", kind: "balance", role: "world"},
                    name: "all-customer-orders-resolved",
                    right: #{kind: "variable", name: "orders"}
                }
            ],
            consume: [["world", [["active", "1"]]]],
            distinct: [],
            preserve: [["other", [["ready", "1"]]], ["vehicle", [["ready", "1"]]]],
            produce: [["world", [["closed", "1"]]]]
        }
    ],
    [
        "deliver/{van}/{order}/false",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [#{asset: "money", quantity: #{kind: "variable", name: "price"}, role: "order"}],
            computed_produce: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "vehicle"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "vehicle"},
                #{asset: "money", quantity: #{kind: "variable", name: "price"}, role: "world"},
                #{asset: "revenue", quantity: #{kind: "variable", name: "price"}, role: "world"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "within-customer-window",
                    right: #{kind: "variable", name: "deadline"}
                }
            ],
            consume: [
                ["order", [["in-transit", "1"]]],
                ["vehicle", [["cargo/{order}", "1"]]],
                ["world", [["permit/{van}", "1"]]]
            ],
            distinct: [],
            preserve: [["vehicle", [["ready", "1"], ["at/{destination}", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["order", [["delivered", "1"]]],
                [
                    "world",
                    [
                        ["used-permit/{van}", "1"],
                        ["resolved", "1"],
                        ["delivered-count", "1"],
                        ["on-time-count", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "deliver/{van}/{order}/true",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "money",
                    quantity: #{
                        denominator: #{kind: "constant", value: "2"},
                        kind: "mul_div",
                        left: #{kind: "variable", name: "price"},
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "order"
                }
            ],
            computed_produce: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "vehicle"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "vehicle"},
                #{
                    asset: "money",
                    quantity: #{
                        denominator: #{kind: "constant", value: "2"},
                        kind: "mul_div",
                        left: #{kind: "variable", name: "price"},
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "world"
                },
                #{
                    asset: "revenue",
                    quantity: #{
                        denominator: #{kind: "constant", value: "2"},
                        kind: "mul_div",
                        left: #{kind: "variable", name: "price"},
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "late-delivery-earns-half-payment",
                    right: #{
                        kind: "add",
                        left: #{kind: "variable", name: "deadline"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            consume: [
                ["order", [["in-transit", "1"]]],
                ["vehicle", [["cargo/{order}", "1"]]],
                ["world", [["permit/{van}", "1"]]]
            ],
            distinct: [],
            preserve: [["vehicle", [["ready", "1"], ["at/{destination}", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["order", [["delivered", "1"]]],
                ["world", [["used-permit/{van}", "1"], ["resolved", "1"], ["delivered-count", "1"]]]
            ]
        }
    ],
    [
        "depart/{van}/{from}/{to}",
        #{
            accounts: [["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{asset: "fuel", quantity: #{kind: "variable", name: "fuel"}, role: "vehicle"},
                #{asset: "road-space", quantity: #{kind: "variable", name: "trunk-lane"}, role: "world"}
            ],
            computed_produce: [
                #{asset: "spent-fuel", quantity: #{kind: "variable", name: "fuel"}, role: "vehicle"},
                #{asset: "tank-space", quantity: #{kind: "variable", name: "fuel"}, role: "vehicle"},
                #{
                    asset: "departed-at",
                    quantity: #{asset: "time", kind: "balance", role: "world"},
                    role: "vehicle"
                },
                #{asset: "on-main-road", quantity: #{kind: "variable", name: "trunk-lane"}, role: "vehicle"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "6"}
                    },
                    name: "activity-fits-remaining-horizon",
                    right: #{kind: "variable", name: "horizon"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "wear", kind: "balance", role: "vehicle"},
                    name: "service-before-excessive-wear",
                    right: #{kind: "constant", value: "5"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "trip", kind: "balance", role: "vehicle"},
                    name: "finite-travel-tickets",
                    right: #{kind: "constant", value: "23"}
                }
            ],
            consume: [["vehicle", [["ready", "1"], ["at/{from}", "1"]]], ["world", [["permit/{van}", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["integrity", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["vehicle", [["travelling/{from}/{to}", "1"], ["awaiting-outcome", "1"]]],
                ["world", [["used-permit/{van}", "1"]]]
            ],
            variables: [
                #{name: "van", values: ["cold", "freight"]},
                #{name: "from", values: ["depot", "hub", "north", "south", "east"]},
                #{name: "to", values: ["depot", "hub", "north", "south", "east"]}
            ],
            relations: [
                #{
                    gives: ["fuel", "trunk-lane"],
                    rows: [
                        ["depot", "hub", "1", "1"],
                        ["hub", "depot", "1", "1"],
                        ["hub", "north", "1", "0"],
                        ["north", "hub", "1", "0"],
                        ["hub", "south", "1", "0"],
                        ["south", "hub", "1", "0"],
                        ["north", "east", "1", "0"],
                        ["east", "north", "1", "0"],
                        ["south", "east", "1", "0"],
                        ["east", "south", "1", "0"],
                        ["depot", "north", "2", "0"],
                        ["north", "depot", "2", "0"]
                    ],
                    variables: ["from", "to"]
                }
            ]
        }
    ],
    [
        "end-refuel/{van}",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "activity-duration-elapsed",
                    right: #{asset: "ready-at", kind: "balance", role: "vehicle"}
                }
            ],
            consume: [["vehicle", [["refueling", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["station", [["fuel-pump", "1"]]], ["vehicle", [["ready", "1"]]]],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "end-repair/{van}/false",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                },
                #{asset: "wear", quantity: #{asset: "wear", kind: "balance", role: "vehicle"}, role: "vehicle"}
            ],
            computed_produce: [
                #{
                    asset: "maintained-wear",
                    quantity: #{asset: "wear", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "activity-duration-elapsed",
                    right: #{asset: "ready-at", kind: "balance", role: "vehicle"}
                }
            ],
            consume: [["vehicle", [["repairing", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["integrity", "1"]]], ["world", [["active", "1"]]]],
            produce: [["station", [["repair-bay", "1"]]], ["vehicle", [["ready", "1"]]]],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "end-repair/{van}/true",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                },
                #{asset: "wear", quantity: #{asset: "wear", kind: "balance", role: "vehicle"}, role: "vehicle"}
            ],
            computed_produce: [
                #{
                    asset: "maintained-wear",
                    quantity: #{asset: "wear", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "activity-duration-elapsed",
                    right: #{asset: "ready-at", kind: "balance", role: "vehicle"}
                }
            ],
            consume: [["vehicle", [["repairing", "1"], ["damage", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["station", [["repair-bay", "1"]]], ["vehicle", [["ready", "1"], ["integrity", "1"]]]],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "end-tow/{van}",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "vehicle"},
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "activity-duration-elapsed",
                    right: #{asset: "ready-at", kind: "balance", role: "vehicle"}
                }
            ],
            consume: [["vehicle", [["towing", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["station", [["tow-truck", "1"]]], ["vehicle", [["ready", "1"], ["at/depot", "1"]]]],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "expire/{order}/none/{at}",
        #{
            accounts: [["order", "order/{order}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "dock-used",
                    quantity: #{asset: "dock-used", kind: "balance", role: "order"},
                    role: "order"
                }
            ],
            computed_produce: [
                #{
                    asset: "dock-space",
                    quantity: #{asset: "dock-used", kind: "balance", role: "order"},
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "order-has-expired",
                    right: #{kind: "variable", name: "expiry"}
                }
            ],
            consume: [["order", [["waiting", "1"], ["at/{at}", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["order", [["expired", "1"]]], ["world", [["resolved", "1"], ["expired-count", "1"]]]]
        }
    ],
    [
        "expire/{order}/some/{van}/depot",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_produce: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "vehicle"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "vehicle"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "order-has-expired",
                    right: #{kind: "variable", name: "expiry"}
                }
            ],
            consume: [["order", [["in-transit", "1"]]], ["vehicle", [["cargo/{order}", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["order", [["expired", "1"]]], ["world", [["resolved", "1"], ["expired-count", "1"]]]]
        }
    ],
    [
        "load/{van}/{order}/{at}",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "vehicle"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "vehicle"},
                #{
                    asset: "dock-used",
                    quantity: #{asset: "dock-used", kind: "balance", role: "order"},
                    role: "order"
                }
            ],
            computed_produce: [
                #{
                    asset: "dock-space",
                    quantity: #{asset: "dock-used", kind: "balance", role: "order"},
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["order", [["waiting", "1"], ["at/{at}", "1"]]], ["world", [["permit/{van}", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["ready", "1"], ["at/{at}", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["order", [["in-transit", "1"]]],
                ["vehicle", [["cargo/{order}", "1"]]],
                ["world", [["used-permit/{van}", "1"]]]
            ]
        }
    ],
    [
        "refuel/{van}",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "2"}
                    },
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "2"}
                    },
                    name: "activity-fits-remaining-horizon",
                    right: #{kind: "variable", name: "horizon"}
                }
            ],
            consume: [
                ["station", [["fuel-pump", "1"], ["fuel", "3"]]],
                ["vehicle", [["ready", "1"], ["tank-space", "3"]]],
                ["world", [["permit/{van}", "1"], ["money", "3"]]]
            ],
            distinct: [],
            preserve: [["vehicle", [["at/depot", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["station", [["money", "3"]]],
                ["vehicle", [["refueling", "1"], ["fuel", "3"]]],
                ["world", [["used-permit/{van}", "1"]]]
            ],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "release/{order}",
        #{
            accounts: [["order", "order/{order}"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "time", kind: "balance", role: "world"},
                    name: "customer-order-has-arrived",
                    right: #{kind: "variable", name: "arrival"}
                }
            ],
            consume: [["order", [["scheduled", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["order", [["waiting", "1"]]]]
        }
    ],
    [
        "repair/{van}/false",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    name: "activity-fits-remaining-horizon",
                    right: #{kind: "variable", name: "horizon"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "wear", kind: "balance", role: "vehicle"},
                    name: "preventive-service-after-accumulated-wear",
                    right: #{kind: "constant", value: "3"}
                }
            ],
            consume: [
                ["station", [["repair-bay", "1"], ["part", "1"]]],
                ["vehicle", [["ready", "1"]]],
                ["world", [["permit/{van}", "1"], ["money", "2"]]]
            ],
            distinct: [],
            preserve: [["vehicle", [["at/depot", "1"], ["integrity", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["station", [["money", "2"], ["used-part", "1"]]],
                ["vehicle", [["repairing", "1"]]],
                ["world", [["used-permit/{van}", "1"]]]
            ],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "repair/{van}/true",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    name: "activity-fits-remaining-horizon",
                    right: #{kind: "variable", name: "horizon"}
                }
            ],
            consume: [
                ["station", [["repair-bay", "1"], ["part", "1"]]],
                ["vehicle", [["ready", "1"]]],
                ["world", [["permit/{van}", "1"], ["money", "2"]]]
            ],
            distinct: [],
            preserve: [["vehicle", [["at/depot", "1"], ["damage", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["station", [["money", "2"], ["used-part", "1"]]],
                ["vehicle", [["repairing", "1"]]],
                ["world", [["used-permit/{van}", "1"]]]
            ],
            variables: [#{name: "van", values: ["cold", "freight"]}]
        }
    ],
    [
        "resolve/{van}/{from}/{to}/{outcome}",
        #{
            accounts: [["nature", "nature"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [#{asset: "integrity", quantity: #{kind: "variable", name: "breakdowns"}, role: "vehicle"}],
            computed_produce: [
                #{
                    asset: "ticket-divisor",
                    quantity: #{
                        kind: "multiply",
                        left: #{asset: "ticket-divisor", kind: "balance", role: "vehicle"},
                        right: #{kind: "constant", value: "3"}
                    },
                    role: "vehicle"
                },
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "departed-at", kind: "balance", role: "vehicle"},
                        right: #{kind: "variable", name: "duration"}
                    },
                    role: "vehicle"
                },
                #{asset: "damage", quantity: #{kind: "variable", name: "breakdowns"}, role: "vehicle"},
                #{asset: "damage-count", quantity: #{kind: "variable", name: "breakdowns"}, role: "world"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "equal",
                    left: #{
                        kind: "subtract",
                        left: #{
                            denominator: #{asset: "ticket-divisor", kind: "balance", role: "vehicle"},
                            kind: "mul_div",
                            left: #{asset: "ticket-word/{van}/{from}/{to}", kind: "balance", role: "nature"},
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        },
                        right: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "4"},
                                kind: "mul_div",
                                left: #{
                                    denominator: #{asset: "ticket-divisor", kind: "balance", role: "vehicle"},
                                    kind: "mul_div",
                                    left: #{asset: "ticket-word/{van}/{from}/{to}", kind: "balance", role: "nature"},
                                    right: #{kind: "constant", value: "1"},
                                    rounding: "floor"
                                },
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "4"}
                        }
                    },
                    name: "match-hidden-vehicle-trip-route-ticket",
                    right: #{kind: "variable", name: "code"}
                }
            ],
            consume: [["vehicle", [["awaiting-outcome", "1"]]]],
            distinct: [],
            preserve: [["vehicle", [["travelling/{from}/{to}", "1"]]], ["world", [["active", "1"]]]],
            produce: [["vehicle", [["trip", "1"], ["wear", "1"]]]],
            variables: [
                #{name: "van", values: ["cold", "freight"]},
                #{name: "from", values: ["depot", "hub", "north", "south", "east"]},
                #{name: "to", values: ["depot", "hub", "north", "south", "east"]},
                #{name: "outcome", values: ["clear", "delayed", "breakdown"]}
            ],
            relations: [
                #{
                    gives: ["fuel", "trunk-lane"],
                    rows: [
                        ["depot", "hub", "1", "1"],
                        ["hub", "depot", "1", "1"],
                        ["hub", "north", "1", "0"],
                        ["north", "hub", "1", "0"],
                        ["hub", "south", "1", "0"],
                        ["south", "hub", "1", "0"],
                        ["north", "east", "1", "0"],
                        ["east", "north", "1", "0"],
                        ["south", "east", "1", "0"],
                        ["east", "south", "1", "0"],
                        ["depot", "north", "2", "0"],
                        ["north", "depot", "2", "0"]
                    ],
                    variables: ["from", "to"]
                },
                #{
                    gives: ["code", "duration", "breakdowns"],
                    rows: [["clear", "0", "2", "0"], ["delayed", "1", "4", "0"], ["breakdown", "2", "6", "1"]],
                    variables: ["outcome"]
                }
            ]
        }
    ],
    [
        "stage/{van}/{order}",
        #{
            accounts: [["order", "order/{order}"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_consume: [#{asset: "dock-space", quantity: #{kind: "variable", name: "weight"}, role: "world"}],
            computed_produce: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "vehicle"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "vehicle"},
                #{asset: "dock-used", quantity: #{kind: "variable", name: "weight"}, role: "order"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [
                ["order", [["in-transit", "1"]]],
                ["vehicle", [["cargo/{order}", "1"]]],
                ["world", [["permit/{van}", "1"]]]
            ],
            distinct: [],
            preserve: [["vehicle", [["ready", "1"], ["at/hub", "1"]]], ["world", [["active", "1"]]]],
            produce: [
                ["order", [["waiting", "1"], ["at/hub", "1"]]],
                ["world", [["used-permit/{van}", "1"], ["staged-count", "1"]]]
            ]
        }
    ],
    [
        "tick",
        #{
            accounts: [["world", "world"]],
            computed_consume: [
                #{
                    asset: "used-permit/cold",
                    quantity: #{asset: "used-permit/cold", kind: "balance", role: "world"},
                    role: "world"
                },
                #{
                    asset: "used-permit/freight",
                    quantity: #{asset: "used-permit/freight", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "permit/cold",
                    quantity: #{asset: "used-permit/cold", kind: "balance", role: "world"},
                    role: "world"
                },
                #{
                    asset: "permit/freight",
                    quantity: #{asset: "used-permit/freight", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["world", [["time-remaining", "1"]]]],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [["world", [["time", "1"]]]]
        }
    ],
    [
        "tow/{van}/{from}",
        #{
            accounts: [["station", "station"], ["vehicle", "vehicle/{van}"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    role: "vehicle"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "time", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    name: "activity-fits-remaining-horizon",
                    right: #{kind: "variable", name: "horizon"}
                }
            ],
            consume: [
                ["station", [["tow-truck", "1"]]],
                ["vehicle", [["ready", "1"], ["at/{from}", "1"]]],
                ["world", [["permit/{van}", "1"], ["money", "4"]]]
            ],
            distinct: [],
            preserve: [["world", [["active", "1"]]]],
            produce: [
                ["station", [["money", "4"]]],
                ["vehicle", [["towing", "1"]]],
                ["world", [["used-permit/{van}", "1"]]]
            ],
            variables: [
                #{name: "van", values: ["cold", "freight"]},
                #{name: "from", values: ["hub", "north", "south", "east"]}
            ]
        }
    ],
    [
        "transfer/{from}/{to}/{order}/{at}",
        #{
            accounts: [
                ["order", "order/{order}"],
                ["other", "vehicle/{to}"],
                ["vehicle", "vehicle/{from}"],
                ["world", "world"]
            ],
            computed_consume: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "other"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "other"}
            ],
            computed_produce: [
                #{asset: "space", quantity: #{kind: "variable", name: "weight"}, role: "vehicle"},
                #{asset: "cold-space", quantity: #{kind: "variable", name: "cold"}, role: "vehicle"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-fleet-event",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["vehicle", [["cargo/{order}", "1"]]], ["world", [["permit/{from}", "1"], ["permit/{to}", "1"]]]],
            distinct: [],
            distinct_variables: [["from", "to"]],
            preserve: [
                ["order", [["in-transit", "1"]]],
                ["other", [["at/{at}", "1"], ["ready", "1"]]],
                ["vehicle", [["ready", "1"], ["at/{at}", "1"]]],
                ["world", [["active", "1"]]]
            ],
            produce: [
                ["other", [["cargo/{order}", "1"]]],
                ["world", [["used-permit/{from}", "1"], ["used-permit/{to}", "1"], ["transfer-count", "1"]]]
            ]
        }
    ]
]
