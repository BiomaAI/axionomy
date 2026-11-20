[
    [
        "begin/{agent}/{job}/lean",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"]],
            computed_consume: [
                #{asset: "energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "time-remaining", quantity: #{kind: "variable", name: "takes-time"}, role: "agent"},
                #{asset: "material", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"}
            ],
            computed_produce: [
                #{asset: "spent-energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "elapsed-time", quantity: #{kind: "variable", name: "takes-time"}, role: "agent"},
                #{asset: "material-spent", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"},
                #{asset: "waste", quantity: #{kind: "variable", name: "leaves-waste"}, role: "agent"}
            ],
            consume: [["job", [["pending", "1"]]]],
            derived: [
                #{name: "takes-energy", value: #{kind: "variable", name: "energy"}},
                #{
                    name: "takes-time",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "time"},
                        right: #{kind: "constant", value: "2"}
                    }
                },
                #{name: "takes-material", value: #{kind: "variable", name: "material"}},
                #{
                    name: "leaves-waste",
                    value: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "job"},
                        right: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "2"},
                                kind: "mul_div",
                                left: #{kind: "variable", name: "job"},
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "2"}
                        }
                    }
                }
            ],
            distinct: [],
            preserve: [
                ["agent", [["at/{place}", "1"], ["operational", "1"], ["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"]]]
            ],
            produce: [["agent", [["attempts", "1"], ["awaiting/{job}/lean", "1"]]], ["job", [["in-progress", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}, #{name: "job", range: ["1", "4"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    ],
    [
        "begin/{agent}/{job}/rush",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"]],
            computed_consume: [
                #{asset: "energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "time-remaining", quantity: #{kind: "variable", name: "takes-time"}, role: "agent"},
                #{asset: "material", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"}
            ],
            computed_produce: [
                #{asset: "spent-energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "elapsed-time", quantity: #{kind: "variable", name: "takes-time"}, role: "agent"},
                #{asset: "material-spent", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"},
                #{asset: "waste", quantity: #{kind: "variable", name: "leaves-waste"}, role: "agent"}
            ],
            consume: [["job", [["pending", "1"]]]],
            derived: [
                #{
                    name: "takes-energy",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "energy"},
                        right: #{kind: "constant", value: "2"}
                    }
                },
                #{
                    name: "takes-time",
                    value: #{
                        kind: "maximum",
                        left: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "time"},
                            right: #{kind: "constant", value: "1"}
                        },
                        right: #{kind: "constant", value: "1"}
                    }
                },
                #{
                    name: "takes-material",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "material"},
                        right: #{kind: "constant", value: "1"}
                    }
                },
                #{name: "leaves-waste", value: #{kind: "constant", value: "2"}}
            ],
            distinct: [],
            preserve: [
                ["agent", [["at/{place}", "1"], ["operational", "1"], ["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"]]]
            ],
            produce: [["agent", [["attempts", "1"], ["awaiting/{job}/rush", "1"]]], ["job", [["in-progress", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}, #{name: "job", range: ["1", "4"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    ],
    [
        "begin/{agent}/{job}/safe",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"]],
            computed_consume: [
                #{asset: "energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "time-remaining", quantity: #{kind: "variable", name: "takes-time"}, role: "agent"},
                #{asset: "material", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"}
            ],
            computed_produce: [
                #{asset: "spent-energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "elapsed-time", quantity: #{kind: "variable", name: "takes-time"}, role: "agent"},
                #{asset: "material-spent", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"},
                #{asset: "waste", quantity: #{kind: "variable", name: "leaves-waste"}, role: "agent"}
            ],
            consume: [["job", [["pending", "1"]]]],
            derived: [
                #{
                    name: "takes-energy",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "energy"},
                        right: #{kind: "constant", value: "1"}
                    }
                },
                #{
                    name: "takes-time",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "time"},
                        right: #{kind: "constant", value: "1"}
                    }
                },
                #{name: "takes-material", value: #{kind: "variable", name: "material"}},
                #{name: "leaves-waste", value: #{kind: "constant", value: "1"}}
            ],
            distinct: [],
            preserve: [
                ["agent", [["at/{place}", "1"], ["operational", "1"], ["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"]]]
            ],
            produce: [["agent", [["attempts", "1"], ["awaiting/{job}/safe", "1"]]], ["job", [["in-progress", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}, #{name: "job", range: ["1", "4"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    ],
    [
        "claim/{agent}/{job}",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"]],
            consume: [["job", [["available", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["agent", [["claimed/{job}", "1"]]], ["job", [["assigned/{agent}", "1"], ["pending", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}, #{name: "job", range: ["1", "4"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    ],
    [
        "finish/{agent}/{job}/{mode}/failure",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"]],
            consume: [
                ["agent", [["resolved/{job}/{mode}/failure", "1"], ["operational", "1"]]],
                ["job", [["in-progress", "1"]]]
            ],
            distinct: [],
            preserve: [["agent", [["claimed/{job}", "1"]]], ["job", [["assigned/{agent}", "1"]]]],
            produce: [["agent", [["failures", "1"], ["damage", "1"]]], ["job", [["pending", "1"]]]],
            variables: [
                #{name: "agent", values: ["atlas", "bolt"]},
                #{name: "job", range: ["1", "4"]},
                #{name: "mode", values: ["rush", "lean", "safe"]}
            ],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    ],
    [
        "finish/{agent}/{job}/{mode}/success",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"]],
            computed_produce: [#{asset: "value", quantity: #{kind: "variable", name: "reward"}, role: "agent"}],
            consume: [
                ["agent", [["resolved/{job}/{mode}/success", "1"], ["claimed/{job}", "1"]]],
                ["job", [["in-progress", "1"], ["assigned/{agent}", "1"]]]
            ],
            distinct: [],
            preserve: [],
            produce: [["agent", [["completed", "1"], ["successes", "1"]]], ["job", [["completed", "1"]]]],
            variables: [
                #{name: "agent", values: ["atlas", "bolt"]},
                #{name: "job", range: ["1", "4"]},
                #{name: "mode", values: ["rush", "lean", "safe"]}
            ],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    ],
    [
        "move/{agent}/{from}/{to}",
        #{
            accounts: [["agent", "agent/{agent}"]],
            consume: [["agent", [["at/{from}", "1"], ["energy", "1"], ["time-remaining", "1"]]]],
            distinct: [],
            distinct_variables: [["from", "to"]],
            preserve: [],
            produce: [["agent", [["at/{to}", "1"], ["spent-energy", "1"], ["elapsed-time", "1"]]]],
            variables: [
                #{name: "agent", values: ["atlas", "bolt"]},
                #{
                    name: "from",
                    values: ["depot", "north", "east", "south", "west", "workshop", "charger", "recycler"]
                },
                #{
                    name: "to",
                    values: ["depot", "north", "east", "south", "west", "workshop", "charger", "recycler"]
                }
            ]
        }
    ],
    [
        "recharge/{agent}",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/charger"]],
            consume: [["agent", [["time-remaining", "2"]]], ["facility", [["charge-supply", "12"]]]],
            distinct: [],
            preserve: [["agent", [["at/charger", "1"]]]],
            produce: [["agent", [["energy", "12"], ["elapsed-time", "2"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}]
        }
    ],
    [
        "recycle/{agent}",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/recycler"]],
            consume: [["agent", [["waste", "1"], ["energy", "1"], ["time-remaining", "1"]]]],
            distinct: [],
            preserve: [["agent", [["at/recycler", "1"]]]],
            produce: [["agent", [["recycled-waste", "1"], ["spent-energy", "1"], ["elapsed-time", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}]
        }
    ],
    [
        "repair/{agent}",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/workshop"]],
            consume: [["agent", [["damage", "1"], ["time-remaining", "3"]]], ["facility", [["repair-supply", "1"]]]],
            distinct: [],
            preserve: [["agent", [["at/workshop", "1"]]]],
            produce: [
                ["agent", [["operational", "1"], ["elapsed-time", "3"]]],
                ["facility", [["spent-repair-supply", "1"]]]
            ],
            variables: [#{name: "agent", values: ["atlas", "bolt"]}]
        }
    ],
    [
        "resolve/{agent}/{job}/{mode}/{outcome}",
        #{
            accounts: [["agent", "agent/{agent}"]],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{kind: "variable", name: "weight"},
                    name: "the-outcome-can-happen",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["agent", [["awaiting/{job}/{mode}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["agent", [["resolved/{job}/{mode}/{outcome}", "1"]]]],
            variables: [
                #{name: "agent", values: ["atlas", "bolt"]},
                #{name: "job", range: ["1", "4"]},
                #{name: "mode", values: ["rush", "lean", "safe"]},
                #{name: "outcome", values: ["success", "failure"]}
            ],
            relations: [
                #{
                    gives: ["weight"],
                    rows: [
                        ["1", "rush", "success", "7"],
                        ["1", "rush", "failure", "3"],
                        ["1", "lean", "success", "9"],
                        ["1", "lean", "failure", "1"],
                        ["1", "safe", "success", "10"],
                        ["1", "safe", "failure", "0"],
                        ["2", "rush", "success", "4"],
                        ["2", "rush", "failure", "6"],
                        ["2", "lean", "success", "6"],
                        ["2", "lean", "failure", "4"],
                        ["2", "safe", "success", "7"],
                        ["2", "safe", "failure", "3"],
                        ["3", "rush", "success", "5"],
                        ["3", "rush", "failure", "5"],
                        ["3", "lean", "success", "7"],
                        ["3", "lean", "failure", "3"],
                        ["3", "safe", "success", "8"],
                        ["3", "safe", "failure", "2"],
                        ["4", "rush", "success", "6"],
                        ["4", "rush", "failure", "4"],
                        ["4", "lean", "success", "8"],
                        ["4", "lean", "failure", "2"],
                        ["4", "safe", "success", "9"],
                        ["4", "safe", "failure", "1"]
                    ],
                    variables: ["job", "mode", "outcome"]
                }
            ]
        }
    ]
]
