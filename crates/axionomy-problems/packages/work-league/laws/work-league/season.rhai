[
    [
        "begin/{agent}/{job}/lean",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                },
                #{asset: "energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "material", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"}
            ],
            computed_preserve: [
                #{
                    asset: "completed-job/{first}",
                    quantity: #{
                        kind: "subtract",
                        left: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{kind: "variable", name: "job"},
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "job"},
                                right: #{kind: "constant", value: "1"}
                            },
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        }
                    },
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    role: "agent"
                },
                #{asset: "spent-energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "material-spent", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"},
                #{asset: "waste", quantity: #{kind: "variable", name: "leaves-waste"}, role: "agent"},
                #{
                    asset: "embodied-material",
                    quantity: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "takes-material"},
                        right: #{kind: "variable", name: "leaves-waste"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "claim-lease-remains-valid",
                    right: #{asset: "lease-until", kind: "balance", role: "job"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    name: "work-finishes-by-deadline",
                    right: #{kind: "variable", name: "deadline"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "wear", kind: "balance", role: "agent"},
                    name: "equipment-needs-maintenance-below-wear-six",
                    right: #{kind: "constant", value: "5"}
                }
            ],
            consume: [["agent", [["idle", "1"]]], ["job", [["pending", "1"]]], ["world", [["permit/{agent}", "1"]]]],
            derived: [
                #{
                    name: "first",
                    value: #{
                        kind: "add",
                        left: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "4"},
                                kind: "mul_div",
                                left: #{
                                    kind: "subtract",
                                    left: #{kind: "variable", name: "job"},
                                    right: #{kind: "constant", value: "1"}
                                },
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "4"}
                        },
                        right: #{kind: "constant", value: "1"}
                    }
                },
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
                ["job", [["assigned/{agent}", "1"]]],
                ["world", [["season-active", "1"]]]
            ],
            produce: [
                ["agent", [["busy", "1"], ["attempts", "1"], ["wear", "1"], ["awaiting/{job}/lean", "1"]]],
                ["job", [["in-progress", "1"], ["attempts", "1"]]],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ]
        }
    ],
    [
        "begin/{agent}/{job}/rush",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                },
                #{asset: "energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "material", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"}
            ],
            computed_preserve: [
                #{
                    asset: "completed-job/{first}",
                    quantity: #{
                        kind: "subtract",
                        left: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{kind: "variable", name: "job"},
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "job"},
                                right: #{kind: "constant", value: "1"}
                            },
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        }
                    },
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    role: "agent"
                },
                #{asset: "spent-energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "material-spent", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"},
                #{asset: "waste", quantity: #{kind: "variable", name: "leaves-waste"}, role: "agent"},
                #{
                    asset: "embodied-material",
                    quantity: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "takes-material"},
                        right: #{kind: "variable", name: "leaves-waste"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "claim-lease-remains-valid",
                    right: #{asset: "lease-until", kind: "balance", role: "job"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    name: "work-finishes-by-deadline",
                    right: #{kind: "variable", name: "deadline"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "wear", kind: "balance", role: "agent"},
                    name: "equipment-needs-maintenance-below-wear-six",
                    right: #{kind: "constant", value: "5"}
                }
            ],
            consume: [["agent", [["idle", "1"]]], ["job", [["pending", "1"]]], ["world", [["permit/{agent}", "1"]]]],
            derived: [
                #{
                    name: "first",
                    value: #{
                        kind: "add",
                        left: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "4"},
                                kind: "mul_div",
                                left: #{
                                    kind: "subtract",
                                    left: #{kind: "variable", name: "job"},
                                    right: #{kind: "constant", value: "1"}
                                },
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "4"}
                        },
                        right: #{kind: "constant", value: "1"}
                    }
                },
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
                ["job", [["assigned/{agent}", "1"]]],
                ["world", [["season-active", "1"]]]
            ],
            produce: [
                ["agent", [["busy", "1"], ["attempts", "1"], ["wear", "2"], ["awaiting/{job}/rush", "1"]]],
                ["job", [["in-progress", "1"], ["attempts", "1"]]],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ]
        }
    ],
    [
        "begin/{agent}/{job}/safe",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                },
                #{asset: "energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "material", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"}
            ],
            computed_preserve: [
                #{
                    asset: "completed-job/{first}",
                    quantity: #{
                        kind: "subtract",
                        left: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{kind: "variable", name: "job"},
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "job"},
                                right: #{kind: "constant", value: "1"}
                            },
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        }
                    },
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    role: "agent"
                },
                #{asset: "spent-energy", quantity: #{kind: "variable", name: "takes-energy"}, role: "agent"},
                #{asset: "material-spent", quantity: #{kind: "variable", name: "takes-material"}, role: "agent"},
                #{asset: "waste", quantity: #{kind: "variable", name: "leaves-waste"}, role: "agent"},
                #{
                    asset: "embodied-material",
                    quantity: #{
                        kind: "subtract",
                        left: #{kind: "variable", name: "takes-material"},
                        right: #{kind: "variable", name: "leaves-waste"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "claim-lease-remains-valid",
                    right: #{asset: "lease-until", kind: "balance", role: "job"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "variable", name: "takes-time"}
                    },
                    name: "work-finishes-by-deadline",
                    right: #{kind: "variable", name: "deadline"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "wear", kind: "balance", role: "agent"},
                    name: "equipment-needs-maintenance-below-wear-six",
                    right: #{kind: "constant", value: "5"}
                }
            ],
            consume: [["agent", [["idle", "1"]]], ["job", [["pending", "1"]]], ["world", [["permit/{agent}", "1"]]]],
            derived: [
                #{
                    name: "first",
                    value: #{
                        kind: "add",
                        left: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "4"},
                                kind: "mul_div",
                                left: #{
                                    kind: "subtract",
                                    left: #{kind: "variable", name: "job"},
                                    right: #{kind: "constant", value: "1"}
                                },
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "4"}
                        },
                        right: #{kind: "constant", value: "1"}
                    }
                },
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
                ["job", [["assigned/{agent}", "1"]]],
                ["world", [["season-active", "1"]]]
            ],
            produce: [
                ["agent", [["busy", "1"], ["attempts", "1"], ["wear", "1"], ["awaiting/{job}/safe", "1"]]],
                ["job", [["in-progress", "1"], ["attempts", "1"]]],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ]
        }
    ],
    [
        "begin-service/{agent}/charger/false",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/charger"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                }
            ],
            consume: [
                ["agent", [["idle", "1"], ["credit", "1"]]],
                ["facility", [["vacant/charger", "1"], ["charge-supply", "12"]]],
                ["world", [["permit/{agent}", "1"]]]
            ],
            distinct: [],
            preserve: [["agent", [["at/charger", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["busy", "1"], ["servicing/charger", "1"], ["charging-energy", "12"]]],
                ["facility", [["occupied/charger/{agent}", "1"], ["credit", "1"]]],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "begin-service/{agent}/recycler/false",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/recycler"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "2"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "2"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                }
            ],
            consume: [
                ["agent", [["idle", "1"], ["waste", "2"], ["energy", "1"]]],
                ["facility", [["vacant/recycler", "1"]]],
                ["world", [["permit/{agent}", "1"]]]
            ],
            distinct: [],
            preserve: [["agent", [["at/recycler", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [
                [
                    "agent",
                    [
                        ["busy", "1"],
                        ["servicing/recycler", "1"],
                        ["spent-energy", "1"],
                        ["reclaiming-material", "1"],
                        ["recycling-loss", "1"],
                        ["recycled-waste", "2"]
                    ]
                ],
                ["facility", [["occupied/recycler/{agent}", "1"]]],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "begin-service/{agent}/workshop/false",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/workshop"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "4"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "4"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                }
            ],
            consume: [
                ["agent", [["idle", "1"], ["damage", "1"], ["credit", "2"]]],
                ["facility", [["vacant/workshop", "1"], ["repair-supply", "1"]]],
                ["world", [["permit/{agent}", "1"]]]
            ],
            distinct: [],
            preserve: [["agent", [["at/workshop", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["busy", "1"], ["servicing/workshop", "1"], ["repairing", "1"]]],
                [
                    "facility",
                    [["occupied/workshop/{agent}", "1"], ["spent-repair-supply", "1"], ["credit", "2"]]
                ],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "begin-service/{agent}/workshop/true",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/workshop"], ["world", "world"]],
            computed_produce: [
                #{
                    asset: "ready-at",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "4"}
                    },
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "4"}
                    },
                    name: "activity-fits-the-season",
                    right: #{kind: "variable", name: "season-length"}
                }
            ],
            consume: [
                ["agent", [["idle", "1"], ["operational", "1"], ["credit", "2"]]],
                ["facility", [["vacant/workshop", "1"], ["repair-supply", "1"]]],
                ["world", [["permit/{agent}", "1"]]]
            ],
            distinct: [],
            preserve: [["agent", [["at/workshop", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["busy", "1"], ["servicing/workshop", "1"], ["repairing", "1"]]],
                [
                    "facility",
                    [["occupied/workshop/{agent}", "1"], ["spent-repair-supply", "1"], ["credit", "2"]]
                ],
                ["world", [["acted/{agent}", "1"], ["activities", "1"]]]
            ],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "claim/{agent}/{job}",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_preserve: [
                #{
                    asset: "completed-job/{first}",
                    quantity: #{
                        kind: "subtract",
                        left: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{kind: "variable", name: "job"},
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{kind: "constant", value: "4"},
                            kind: "mul_div",
                            left: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "job"},
                                right: #{kind: "constant", value: "1"}
                            },
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        }
                    },
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "lease-until",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "6"}
                    },
                    role: "job"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "contract-has-arrived",
                    right: #{kind: "variable", name: "release"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "contract-remains-open",
                    right: #{kind: "variable", name: "deadline"}
                }
            ],
            consume: [
                ["agent", [["free-contract", "1"]]],
                ["job", [["available", "1"]]],
                ["world", [["permit/{agent}", "1"]]]
            ],
            derived: [
                #{
                    name: "first",
                    value: #{
                        kind: "add",
                        left: #{
                            kind: "multiply",
                            left: #{
                                denominator: #{kind: "constant", value: "4"},
                                kind: "mul_div",
                                left: #{
                                    kind: "subtract",
                                    left: #{kind: "variable", name: "job"},
                                    right: #{kind: "constant", value: "1"}
                                },
                                right: #{kind: "constant", value: "1"},
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "4"}
                        },
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            distinct: [],
            preserve: [["agent", [["idle", "1"], ["operational", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"], ["pending", "1"]]],
                ["world", [["acted/{agent}", "1"]]]
            ]
        }
    ],
    [
        "end-season",
        #{
            accounts: [["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{asset: "contracts-open", kind: "balance", role: "world"},
                    name: "all-contracts-resolved",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "equal",
                    left: #{asset: "activities", kind: "balance", role: "world"},
                    name: "all-activities-settled",
                    right: #{kind: "constant", value: "0"}
                }
            ],
            consume: [["world", [["season-active", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["world", [["season-ended", "1"]]]]
        }
    ],
    [
        "end-service/{agent}/charger",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/charger"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "agent"},
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "activity-is-due",
                    right: #{asset: "ready-at", kind: "balance", role: "agent"}
                }
            ],
            consume: [
                ["agent", [["busy", "1"], ["servicing/charger", "1"], ["charging-energy", "12"]]],
                ["facility", [["occupied/charger/{agent}", "1"]]],
                ["world", [["activities", "1"]]]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [["agent", [["idle", "1"], ["energy", "12"]]], ["facility", [["vacant/charger", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "end-service/{agent}/recycler",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/recycler"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "agent"},
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "activity-is-due",
                    right: #{asset: "ready-at", kind: "balance", role: "agent"}
                }
            ],
            consume: [
                ["agent", [["busy", "1"], ["servicing/recycler", "1"], ["reclaiming-material", "1"]]],
                ["facility", [["occupied/recycler/{agent}", "1"]]],
                ["world", [["activities", "1"]]]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [["agent", [["idle", "1"], ["material", "1"]]], ["facility", [["vacant/recycler", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "end-service/{agent}/workshop",
        #{
            accounts: [["agent", "agent/{agent}"], ["facility", "facility/workshop"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "agent"},
                    role: "agent"
                },
                #{asset: "wear", quantity: #{asset: "wear", kind: "balance", role: "agent"}, role: "agent"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "activity-is-due",
                    right: #{asset: "ready-at", kind: "balance", role: "agent"}
                }
            ],
            consume: [
                ["agent", [["busy", "1"], ["servicing/workshop", "1"], ["repairing", "1"]]],
                ["facility", [["occupied/workshop/{agent}", "1"]]],
                ["world", [["activities", "1"]]]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [["agent", [["idle", "1"], ["operational", "1"]]], ["facility", [["vacant/workshop", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ],
    [
        "expire/{job}/none/none",
        #{
            accounts: [["job", "job/{job}"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "contract-deadline-has-elapsed",
                    right: #{
                        kind: "add",
                        left: #{kind: "variable", name: "deadline"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            consume: [["job", [["available", "1"]]], ["world", [["contracts-open", "1"]]]],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [["job", [["expired", "1"]]], ["world", [["contracts-closed", "1"]]]]
        }
    ],
    [
        "expire/{job}/some/{agent}/none",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "contract-deadline-has-elapsed",
                    right: #{
                        kind: "add",
                        left: #{kind: "variable", name: "deadline"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            consume: [
                ["agent", [["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"], ["pending", "1"]]],
                ["world", [["contracts-open", "1"]]]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["free-contract", "1"]]],
                ["job", [["expired", "1"]]],
                ["world", [["contracts-closed", "1"]]]
            ]
        }
    ],
    [
        "expire/{job}/some/{agent}/some/{mode}",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "agent"},
                    role: "agent"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "contract-deadline-has-elapsed",
                    right: #{
                        kind: "add",
                        left: #{kind: "variable", name: "deadline"},
                        right: #{kind: "constant", value: "1"}
                    }
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "activity-is-due",
                    right: #{asset: "ready-at", kind: "balance", role: "agent"}
                }
            ],
            consume: [
                ["agent", [["claimed/{job}", "1"], ["busy", "1"], ["awaiting/{job}/{mode}", "1"]]],
                ["job", [["assigned/{agent}", "1"], ["in-progress", "1"]]],
                ["world", [["contracts-open", "1"], ["activities", "1"]]]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["free-contract", "1"], ["idle", "1"]]],
                ["job", [["expired", "1"]]],
                ["world", [["contracts-closed", "1"]]]
            ]
        }
    ],
    [
        "finish/{agent}/{job}/{mode}/failure",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "agent"},
                    role: "agent"
                }
            ],
            computed_produce: [
                #{
                    asset: "lease-until",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "6"}
                    },
                    role: "job"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "activity-is-due",
                    right: #{asset: "ready-at", kind: "balance", role: "agent"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{kind: "variable", name: "weight"},
                    name: "the-outcome-can-happen",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "outcome-settles-before-expiry",
                    right: #{kind: "variable", name: "deadline"}
                }
            ],
            consume: [
                ["agent", [["busy", "1"], ["awaiting/{job}/{mode}", "1"], ["operational", "1"]]],
                ["job", [["in-progress", "1"]]],
                ["world", [["activities", "1"]]]
            ],
            distinct: [],
            preserve: [
                ["agent", [["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"]]],
                ["world", [["season-active", "1"]]]
            ],
            produce: [["agent", [["idle", "1"], ["damage", "1"], ["failures", "1"]]], ["job", [["pending", "1"]]]]
        }
    ],
    [
        "finish/{agent}/{job}/{mode}/success",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "ready-at",
                    quantity: #{asset: "ready-at", kind: "balance", role: "agent"},
                    role: "agent"
                },
                #{asset: "credit", quantity: #{kind: "variable", name: "reward"}, role: "job"}
            ],
            computed_produce: [
                #{asset: "value", quantity: #{kind: "variable", name: "reward"}, role: "agent"},
                #{asset: "credit", quantity: #{kind: "variable", name: "reward"}, role: "agent"}
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "activity-is-due",
                    right: #{asset: "ready-at", kind: "balance", role: "agent"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{kind: "variable", name: "weight"},
                    name: "the-outcome-can-happen",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "outcome-settles-before-expiry",
                    right: #{kind: "variable", name: "deadline"}
                }
            ],
            consume: [
                ["agent", [["busy", "1"], ["awaiting/{job}/{mode}", "1"], ["claimed/{job}", "1"]]],
                ["job", [["in-progress", "1"], ["assigned/{agent}", "1"]]],
                ["world", [["activities", "1"], ["contracts-open", "1"]]]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["idle", "1"], ["free-contract", "1"], ["completed", "1"], ["successes", "1"]]],
                ["job", [["completed", "1"]]],
                ["world", [["contracts-closed", "1"], ["completed-job/{job}", "1"]]]
            ]
        }
    ],
    [
        "handoff/{agent}/{recipient}/{job}",
        #{
            accounts: [
                ["agent", "agent/{agent}"],
                ["job", "job/{job}"],
                ["partner", "agent/{recipient}"],
                ["world", "world"]
            ],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                }
            ],
            computed_produce: [
                #{
                    asset: "lease-until",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "6"}
                    },
                    role: "job"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "handoff-before-deadline",
                    right: #{kind: "variable", name: "deadline"}
                }
            ],
            consume: [
                ["agent", [["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"], ["transferable", "1"]]],
                ["partner", [["free-contract", "1"], ["credit", "2"]]],
                ["world", [["permit/{agent}", "1"], ["permit/{recipient}", "1"]]]
            ],
            distinct: [],
            distinct_variables: [["agent", "recipient"]],
            preserve: [
                ["agent", [["idle", "1"]]],
                ["job", [["pending", "1"]]],
                ["partner", [["idle", "1"], ["operational", "1"]]],
                ["world", [["season-active", "1"]]]
            ],
            produce: [
                ["agent", [["free-contract", "1"], ["credit", "2"], ["assists", "1"]]],
                ["job", [["assigned/{recipient}", "1"], ["handed-off", "1"]]],
                ["partner", [["claimed/{job}", "1"]]],
                ["world", [["acted/{agent}", "1"], ["acted/{recipient}", "1"]]]
            ]
        }
    ],
    [
        "move/{agent}/{from}/{to}",
        #{
            accounts: [["agent", "agent/{agent}"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["agent", [["at/{from}", "1"], ["energy", "1"]]], ["world", [["permit/{agent}", "1"]]]],
            distinct: [],
            preserve: [["agent", [["idle", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [["agent", [["at/{to}", "1"], ["spent-energy", "1"]]], ["world", [["acted/{agent}", "1"]]]],
            variables: [
                #{name: "agent", values: ["atlas", "bolt", "coda", "delta"]},
                #{
                    name: "from",
                    values: ["depot", "north", "east", "south", "west", "workshop", "charger", "recycler"]
                },
                #{
                    name: "to",
                    values: ["depot", "north", "east", "south", "west", "workshop", "charger", "recycler"]
                }
            ],
            relations: [
                #{
                    rows: [
                        ["depot", "north"],
                        ["depot", "east"],
                        ["depot", "south"],
                        ["depot", "west"],
                        ["north", "depot"],
                        ["north", "recycler"],
                        ["east", "depot"],
                        ["east", "charger"],
                        ["south", "depot"],
                        ["south", "workshop"],
                        ["south", "charger"],
                        ["west", "depot"],
                        ["west", "workshop"],
                        ["workshop", "south"],
                        ["workshop", "west"],
                        ["charger", "east"],
                        ["charger", "south"],
                        ["recycler", "north"]
                    ],
                    variables: ["from", "to"]
                }
            ]
        }
    ],
    [
        "release-claim/{agent}/{job}/false",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [
                ["agent", [["claimed/{job}", "1"]]],
                ["job", [["assigned/{agent}", "1"], ["pending", "1"]]],
                ["world", [["permit/{agent}", "1"]]]
            ],
            distinct: [],
            preserve: [["agent", [["idle", "1"]]], ["world", [["season-active", "1"]]]],
            produce: [
                ["agent", [["free-contract", "1"]]],
                ["job", [["available", "1"]]],
                ["world", [["acted/{agent}", "1"]]]
            ]
        }
    ],
    [
        "release-claim/{agent}/{job}/true",
        #{
            accounts: [["agent", "agent/{agent}"], ["job", "job/{job}"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "lease-until",
                    quantity: #{asset: "lease-until", kind: "balance", role: "job"},
                    role: "job"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "tick", kind: "balance", role: "world"},
                    name: "claim-lease-has-elapsed",
                    right: #{
                        kind: "add",
                        left: #{asset: "lease-until", kind: "balance", role: "job"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            consume: [["agent", [["claimed/{job}", "1"]]], ["job", [["assigned/{agent}", "1"], ["pending", "1"]]]],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [["agent", [["free-contract", "1"]]], ["job", [["available", "1"]]]]
        }
    ],
    [
        "tick",
        #{
            accounts: [
                ["worker/atlas", "agent/atlas"],
                ["worker/bolt", "agent/bolt"],
                ["worker/coda", "agent/coda"],
                ["worker/delta", "agent/delta"],
                ["world", "world"]
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "tick", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "1"}
                    },
                    name: "season-has-time-remaining",
                    right: #{kind: "variable", name: "season-length"}
                }
            ],
            consume: [
                ["worker/atlas", [["time-remaining", "1"]]],
                ["worker/bolt", [["time-remaining", "1"]]],
                ["worker/coda", [["time-remaining", "1"]]],
                ["worker/delta", [["time-remaining", "1"]]],
                [
                    "world",
                    [["acted/atlas", "1"], ["acted/bolt", "1"], ["acted/coda", "1"], ["acted/delta", "1"]]
                ]
            ],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [
                ["worker/atlas", [["elapsed-time", "1"]]],
                ["worker/bolt", [["elapsed-time", "1"]]],
                ["worker/coda", [["elapsed-time", "1"]]],
                ["worker/delta", [["elapsed-time", "1"]]],
                [
                    "world",
                    [
                        ["tick", "1"],
                        ["permit/atlas", "1"],
                        ["permit/bolt", "1"],
                        ["permit/coda", "1"],
                        ["permit/delta", "1"]
                    ]
                ]
            ]
        }
    ],
    [
        "wait/{agent}",
        #{
            accounts: [["agent", "agent/{agent}"], ["world", "world"]],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-event-at-a-time",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["world", [["permit/{agent}", "1"]]]],
            distinct: [],
            preserve: [["world", [["season-active", "1"]]]],
            produce: [["world", [["acted/{agent}", "1"]]]],
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}]
        }
    ]
]
