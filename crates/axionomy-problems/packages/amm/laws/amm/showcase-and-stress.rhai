[
    [
        "add-liquidity",
        #{
            accounts: [["pool", "pool"]],
            computed_consume: [
                #{
                    asset: "energy",
                    quantity: #{kind: "parameter", name: "energy-amount"},
                    role: "liquidity-provider"
                },
                #{
                    asset: "credit",
                    quantity: #{kind: "parameter", name: "credit-amount"},
                    role: "liquidity-provider"
                },
                #{
                    asset: "unissued-lp-share",
                    quantity: #{
                        kind: "minimum",
                        left: #{
                            denominator: #{asset: "energy", kind: "balance", role: "pool"},
                            kind: "mul_div",
                            left: #{kind: "parameter", name: "energy-amount"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{asset: "credit", kind: "balance", role: "pool"},
                            kind: "mul_div",
                            left: #{kind: "parameter", name: "credit-amount"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            rounding: "floor"
                        }
                    },
                    role: "pool"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{kind: "constant", value: "1"},
                    role: "liquidity-provider"
                },
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "pool"},
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"}
            ],
            computed_produce: [
                #{asset: "energy", quantity: #{kind: "parameter", name: "energy-amount"}, role: "pool"},
                #{asset: "credit", quantity: #{kind: "parameter", name: "credit-amount"}, role: "pool"},
                #{
                    asset: "lp-share",
                    quantity: #{
                        kind: "minimum",
                        left: #{
                            denominator: #{asset: "energy", kind: "balance", role: "pool"},
                            kind: "mul_div",
                            left: #{kind: "parameter", name: "energy-amount"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{asset: "credit", kind: "balance", role: "pool"},
                            kind: "mul_div",
                            left: #{kind: "parameter", name: "credit-amount"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            rounding: "floor"
                        }
                    },
                    role: "liquidity-provider"
                }
            ],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{
                        kind: "multiply",
                        left: #{kind: "parameter", name: "energy-amount"},
                        right: #{asset: "credit", kind: "balance", role: "pool"}
                    },
                    name: "liquidity-must-preserve-the-discovered-price",
                    right: #{
                        kind: "multiply",
                        left: #{kind: "parameter", name: "credit-amount"},
                        right: #{asset: "energy", kind: "balance", role: "pool"}
                    }
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        kind: "minimum",
                        left: #{
                            denominator: #{asset: "energy", kind: "balance", role: "pool"},
                            kind: "mul_div",
                            left: #{kind: "parameter", name: "energy-amount"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            rounding: "floor"
                        },
                        right: #{
                            denominator: #{asset: "credit", kind: "balance", role: "pool"},
                            kind: "mul_div",
                            left: #{kind: "parameter", name: "credit-amount"},
                            right: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            rounding: "floor"
                        }
                    },
                    name: "minimum-liquidity-shares",
                    right: #{kind: "parameter", name: "minimum-lp-shares"}
                },
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-liquidity-deposit",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [],
            distinct: [["liquidity-provider", "pool"]],
            preserve: [],
            produce: []
        }
    ],
    [
        "advance-day/{day}",
        #{
            accounts: [
                ["information", "information"],
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
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/founder"},
                    role: "participant/founder"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/founder"},
                    role: "participant/founder"
                },
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
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/generator"},
                    role: "participant/generator"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/generator"},
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
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/factory"},
                    role: "participant/factory"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/factory"},
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
                },
                #{
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/household"},
                    role: "participant/household"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/household"},
                    role: "participant/household"
                },
                #{
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/speculator"},
                    role: "participant/speculator"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/speculator"},
                    role: "participant/speculator"
                },
                #{
                    asset: "informed/speculator",
                    quantity: #{asset: "informed/speculator", kind: "balance", role: "participant/speculator"},
                    role: "participant/speculator"
                },
                #{
                    asset: "uninformed/speculator",
                    quantity: #{asset: "uninformed/speculator", kind: "balance", role: "participant/speculator"},
                    role: "participant/speculator"
                },
                #{
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/adaptive-lp"},
                    role: "participant/adaptive-lp"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/adaptive-lp"},
                    role: "participant/adaptive-lp"
                },
                #{
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/arbitrageur"},
                    role: "participant/arbitrageur"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/arbitrageur"},
                    role: "participant/arbitrageur"
                },
                #{
                    asset: "reference-price",
                    quantity: #{asset: "reference-price", kind: "balance", role: "participant/whale"},
                    role: "participant/whale"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{asset: "liquidity-decision", kind: "balance", role: "participant/whale"},
                    role: "participant/whale"
                },
                #{
                    asset: "shortage-signal",
                    quantity: #{asset: "shortage-signal", kind: "balance", role: "information"},
                    role: "information"
                },
                #{
                    asset: "current-sunlight",
                    quantity: #{asset: "current-sunlight", kind: "balance", role: "world"},
                    role: "world"
                }
            ],
            computed_produce: [
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/founder"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/founder"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
                    role: "participant/founder"
                },
                #{
                    asset: "defaulted-obligation/generator",
                    quantity: #{asset: "obligation/generator", kind: "balance", role: "participant/generator"},
                    role: "participant/generator"
                },
                #{
                    asset: "production-allowance",
                    quantity: #{
                        denominator: #{kind: "constant", value: "100"},
                        kind: "mul_div",
                        left: #{asset: "production-capacity", kind: "balance", role: "participant/generator"},
                        right: #{kind: "variable", name: "sunlight"},
                        rounding: "floor"
                    },
                    role: "participant/generator"
                },
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/generator"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/generator"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
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
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/factory"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/factory"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
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
                },
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/household"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/household"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
                    role: "participant/household"
                },
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/speculator"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/speculator"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
                    role: "participant/speculator"
                },
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/adaptive-lp"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/adaptive-lp"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
                    role: "participant/adaptive-lp"
                },
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/arbitrageur"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/arbitrageur"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
                    role: "participant/arbitrageur"
                },
                #{
                    asset: "reference-price",
                    quantity: #{
                        denominator: #{kind: "constant", value: "4"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "reference-price", kind: "balance", role: "participant/whale"},
                                right: #{kind: "constant", value: "3"}
                            },
                            right: #{
                                denominator: #{asset: "energy", kind: "balance", role: "pool"},
                                kind: "mul_div",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "constant", value: "1000"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "constant", value: "1"},
                        rounding: "floor"
                    },
                    role: "participant/whale"
                },
                #{
                    asset: "decisions",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "daily-decisions"},
                        right: #{kind: "units"}
                    },
                    role: "participant/whale"
                },
                #{asset: "current-sunlight", quantity: #{kind: "variable", name: "sunlight"}, role: "world"},
                #{
                    asset: "shortage-signal",
                    quantity: #{kind: "variable", name: "shortage"},
                    role: "information"
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
                ["participant/adaptive-lp", [["done", "1"]]],
                ["participant/arbitrageur", [["done", "1"]]],
                [
                    "participant/factory",
                    [["done", "1"], ["future-need/factory", "1"], ["future-obligation/factory", "1"]]
                ],
                ["participant/founder", [["done", "1"]]],
                ["participant/generator", [["done", "1"], ["future-obligation/generator", "1"]]],
                [
                    "participant/household",
                    [["done", "1"], ["future-need/household", "1"], ["future-income", "1"]]
                ],
                ["participant/speculator", [["done", "1"]]],
                ["participant/whale", [["done", "1"]]],
                ["world", [["at-day/{day}", "1"]]]
            ],
            derived: [
                #{
                    name: "next",
                    value: #{
                        kind: "add",
                        left: #{kind: "variable", name: "day"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            distinct: [],
            preserve: [],
            produce: [
                ["participant/adaptive-lp", [["trading", "1"], ["liquidity-decision", "1"]]],
                ["participant/arbitrageur", [["trading", "1"], ["liquidity-decision", "1"]]],
                [
                    "participant/factory",
                    [
                        ["need/factory", "1"],
                        ["obligation/factory", "1"],
                        ["trading", "1"],
                        ["liquidity-decision", "1"]
                    ]
                ],
                ["participant/founder", [["trading", "1"], ["liquidity-decision", "1"]]],
                [
                    "participant/generator",
                    [["obligation/generator", "1"], ["trading", "1"], ["liquidity-decision", "1"]]
                ],
                [
                    "participant/household",
                    [
                        ["need/household", "1"],
                        ["income-due", "1"],
                        ["trading", "1"],
                        ["liquidity-decision", "1"]
                    ]
                ],
                [
                    "participant/speculator",
                    [["trading", "1"], ["liquidity-decision", "1"], ["uninformed/speculator", "1"]]
                ],
                ["participant/whale", [["trading", "1"], ["liquidity-decision", "1"]]],
                ["world", [["at-day/{next}", "1"], ["day", "1"]]]
            ]
        }
    ],
    [
        "buy-energy",
        #{
            accounts: [["pool", "pool"]],
            computed_consume: [
                #{
                    asset: "energy",
                    quantity: #{
                        denominator: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "variable", name: "fee-denominator"}
                            },
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            }
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "fee-numerator"},
                            right: #{kind: "units"}
                        },
                        rounding: "floor"
                    },
                    role: "pool"
                },
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "trader"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "pool"},
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "trader"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "trader"}
            ],
            computed_produce: [
                #{
                    asset: "energy",
                    quantity: #{
                        denominator: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "variable", name: "fee-denominator"}
                            },
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            }
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "fee-numerator"},
                            right: #{kind: "units"}
                        },
                        rounding: "floor"
                    },
                    role: "trader"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        denominator: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "credit", kind: "balance", role: "pool"},
                                right: #{kind: "variable", name: "fee-denominator"}
                            },
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            }
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "fee-numerator"},
                            right: #{kind: "units"}
                        },
                        rounding: "floor"
                    },
                    name: "minimum-output",
                    right: #{kind: "parameter", name: "minimum-output"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "energy", kind: "balance", role: "trader"},
                        right: #{
                            denominator: #{
                                kind: "add",
                                left: #{
                                    kind: "multiply",
                                    left: #{asset: "credit", kind: "balance", role: "pool"},
                                    right: #{kind: "variable", name: "fee-denominator"}
                                },
                                right: #{
                                    kind: "multiply",
                                    left: #{kind: "variable", name: "fee-numerator"},
                                    right: #{kind: "units"}
                                }
                            },
                            kind: "mul_div",
                            left: #{asset: "energy", kind: "balance", role: "pool"},
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            },
                            rounding: "floor"
                        }
                    },
                    name: "purchased-energy-fits-storage",
                    right: #{asset: "storage-capacity", kind: "balance", role: "trader"}
                }
            ],
            consume: [["trader", [["credit", "1"]]]],
            distinct: [["pool", "trader"]],
            preserve: [],
            produce: [["pool", [["credit", "1"]]]]
        }
    ],
    [
        "collect-income",
        #{
            accounts: [["consumer", "actor/household"], ["treasury", "treasury"]],
            computed_consume: [
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "consumer"},
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "constant", value: "4000"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "treasury"
                }
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "consumer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "consumer"}
            ],
            computed_produce: [
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "constant", value: "4000"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "consumer"
                }
            ],
            consume: [["consumer", [["income-due", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["consumer", [["received-income", "1"]]]]
        }
    ],
    [
        "commission-production",
        #{
            accounts: [["producer", "actor/generator"], ["world", "world"]],
            computed_consume: [
                #{
                    asset: "construction-due",
                    quantity: #{asset: "construction-due", kind: "balance", role: "producer"},
                    role: "producer"
                },
                #{
                    asset: "pending-capacity",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "project-capacity"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                }
            ],
            computed_produce: [
                #{
                    asset: "production-capacity",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "project-capacity"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                },
                #{
                    asset: "production-allowance",
                    quantity: #{
                        denominator: #{kind: "constant", value: "100"},
                        kind: "mul_div",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "project-capacity"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{asset: "current-sunlight", kind: "balance", role: "world"},
                        rounding: "floor"
                    },
                    role: "producer"
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
                },
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "day", kind: "balance", role: "world"},
                    name: "construction-has-finished",
                    right: #{asset: "construction-due", kind: "balance", role: "producer"}
                }
            ],
            consume: [],
            distinct: [],
            preserve: [],
            produce: []
        }
    ],
    [
        "deliver-goods",
        #{
            accounts: [["producer", "actor/factory"], ["treasury", "treasury"]],
            computed_consume: [
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "producer"},
                #{
                    asset: "goods",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "batch"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "constant", value: "15000"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "treasury"
                }
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "producer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "producer"}
            ],
            computed_produce: [
                #{
                    asset: "delivered-goods",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "batch"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "treasury"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "constant", value: "15000"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                }
            ],
            consume: [],
            distinct: [],
            preserve: [],
            produce: []
        }
    ],
    [
        "end-trading/{actor}",
        #{
            accounts: [["consumer", "actor/{actor}"]],
            computed_consume: [
                #{
                    asset: "decisions",
                    quantity: #{asset: "decisions", kind: "balance", role: "consumer"},
                    role: "consumer"
                }
            ],
            consume: [["consumer", [["trading", "1"]]]],
            distinct: [],
            preserve: [["consumer", [["market-open", "1"]]]],
            produce: [["consumer", [["done", "1"]]]],
            variables: [
                #{
                    name: "actor",
                    values: [
                        "founder",
                        "generator",
                        "factory",
                        "household",
                        "speculator",
                        "adaptive-lp",
                        "arbitrageur",
                        "whale"
                    ]
                }
            ]
        }
    ],
    [
        "fund-liquidity",
        #{
            accounts: [["pool", "pool"]],
            computed_consume: [
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"},
                #{
                    asset: "liquidity-decision",
                    quantity: #{kind: "constant", value: "1"},
                    role: "liquidity-provider"
                },
                #{
                    asset: "energy",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "ceil"
                    },
                    role: "liquidity-provider"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "ceil"
                    },
                    role: "liquidity-provider"
                }
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"},
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "pool"}
            ],
            computed_produce: [
                #{
                    asset: "energy",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "ceil"
                    },
                    role: "pool"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "ceil"
                    },
                    role: "pool"
                }
            ],
            consume: [["pool", [["unissued-lp-share", "1"]]]],
            distinct: [["liquidity-provider", "pool"]],
            preserve: [],
            produce: [["liquidity-provider", [["lp-share", "1"]]]]
        }
    ],
    [
        "invest-production",
        #{
            accounts: [["producer", "actor/generator"], ["treasury", "treasury"], ["world", "world"]],
            computed_consume: [
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "producer"},
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "constant", value: "6000"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                },
                #{
                    asset: "future-capacity",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "project-capacity"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                }
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "producer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "producer"}
            ],
            computed_produce: [
                #{
                    asset: "pending-capacity",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "project-capacity"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "producer"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "constant", value: "6000"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "treasury"
                },
                #{
                    asset: "construction-due",
                    quantity: #{
                        kind: "add",
                        left: #{asset: "day", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "2"}
                    },
                    role: "producer"
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
                },
                #{
                    comparison: "equal",
                    left: #{asset: "pending-capacity", kind: "balance", role: "producer"},
                    name: "one-construction-project",
                    right: #{kind: "constant", value: "0"}
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "day", kind: "balance", role: "world"},
                        right: #{kind: "constant", value: "3"}
                    },
                    name: "capacity-arrives-before-close",
                    right: #{kind: "variable", name: "season-length"}
                }
            ],
            consume: [],
            distinct: [],
            preserve: [],
            produce: []
        }
    ],
    [
        "learn-shortage/speculator",
        #{
            accounts: [["information", "information"], ["observer", "actor/speculator"]],
            computed_consume: [#{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "observer"}],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "observer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "observer"}
            ],
            consume: [["observer", [["uninformed/speculator", "1"]]]],
            distinct: [],
            preserve: [["information", [["shortage-signal", "1"]]]],
            produce: [["observer", [["informed/speculator", "1"]]]]
        }
    ],
    [
        "produce-energy",
        #{
            accounts: [["producer", "actor/generator"]],
            computed_consume: [
                #{asset: "production-allowance", quantity: #{kind: "units"}, role: "producer"},
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "producer"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "producer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "producer"}
            ],
            conditions: [
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "energy", kind: "balance", role: "producer"},
                        right: #{kind: "units"}
                    },
                    name: "production-fits-storage",
                    right: #{asset: "storage-capacity", kind: "balance", role: "producer"}
                }
            ],
            consume: [["producer", [["solar-potential", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["producer", [["energy", "1"]]]]
        }
    ],
    [
        "remove-liquidity",
        #{
            accounts: [["pool", "pool"]],
            computed_consume: [
                #{
                    asset: "energy",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "floor"
                    },
                    role: "pool"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "floor"
                    },
                    role: "pool"
                },
                #{
                    asset: "liquidity-decision",
                    quantity: #{kind: "constant", value: "1"},
                    role: "liquidity-provider"
                },
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "pool"},
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "liquidity-provider"}
            ],
            computed_produce: [
                #{
                    asset: "energy",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "energy", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "floor"
                    },
                    role: "liquidity-provider"
                },
                #{
                    asset: "credit",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{kind: "variable", name: "share-limit"},
                            right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{kind: "units"},
                        rounding: "floor"
                    },
                    role: "liquidity-provider"
                }
            ],
            conditions: [
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "add",
                        left: #{asset: "energy", kind: "balance", role: "liquidity-provider"},
                        right: #{
                            denominator: #{
                                kind: "subtract",
                                left: #{kind: "variable", name: "share-limit"},
                                right: #{asset: "unissued-lp-share", kind: "balance", role: "pool"}
                            },
                            kind: "mul_div",
                            left: #{asset: "energy", kind: "balance", role: "pool"},
                            right: #{kind: "units"},
                            rounding: "floor"
                        }
                    },
                    name: "withdrawn-energy-fits-storage",
                    right: #{asset: "storage-capacity", kind: "balance", role: "liquidity-provider"}
                }
            ],
            consume: [["liquidity-provider", [["lp-share", "1"]]]],
            distinct: [["liquidity-provider", "pool"]],
            preserve: [],
            produce: [["pool", [["unissued-lp-share", "1"]]]]
        }
    ],
    [
        "sell-energy",
        #{
            accounts: [["pool", "pool"]],
            computed_consume: [
                #{
                    asset: "credit",
                    quantity: #{
                        denominator: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "energy", kind: "balance", role: "pool"},
                                right: #{kind: "variable", name: "fee-denominator"}
                            },
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            }
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "fee-numerator"},
                            right: #{kind: "units"}
                        },
                        rounding: "floor"
                    },
                    role: "pool"
                },
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "trader"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "pool"},
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "trader"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "trader"}
            ],
            computed_produce: [
                #{
                    asset: "credit",
                    quantity: #{
                        denominator: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "energy", kind: "balance", role: "pool"},
                                right: #{kind: "variable", name: "fee-denominator"}
                            },
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            }
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "fee-numerator"},
                            right: #{kind: "units"}
                        },
                        rounding: "floor"
                    },
                    role: "trader"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        denominator: #{
                            kind: "add",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "energy", kind: "balance", role: "pool"},
                                right: #{kind: "variable", name: "fee-denominator"}
                            },
                            right: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "fee-numerator"},
                                right: #{kind: "units"}
                            }
                        },
                        kind: "mul_div",
                        left: #{asset: "credit", kind: "balance", role: "pool"},
                        right: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "fee-numerator"},
                            right: #{kind: "units"}
                        },
                        rounding: "floor"
                    },
                    name: "minimum-output",
                    right: #{kind: "parameter", name: "minimum-output"}
                }
            ],
            consume: [["trader", [["energy", "1"]]]],
            distinct: [["pool", "trader"]],
            preserve: [],
            produce: [["pool", [["energy", "1"]]]]
        }
    ],
    [
        "settle-obligation/{actor}",
        #{
            accounts: [["consumer", "actor/{actor}"], ["treasury", "treasury"]],
            computed_consume: [
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "cost"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "consumer"
                },
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "consumer"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "consumer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "consumer"}
            ],
            computed_produce: [
                #{
                    asset: "credit",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "cost"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "treasury"
                }
            ],
            consume: [["consumer", [["obligation/{actor}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["consumer", [["settled-obligation/{actor}", "1"]]]],
            variables: [#{name: "actor", values: ["generator", "factory"]}],
            relations: [#{gives: ["cost"], rows: [["generator", "4000"], ["factory", "6000"]], variables: ["actor"]}]
        }
    ],
    [
        "use-energy/{actor}",
        #{
            accounts: [["consumer", "actor/{actor}"]],
            computed_consume: [
                #{
                    asset: "energy",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "energy"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "consumer"
                },
                #{
                    asset: "unmade-goods",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "batches"},
                                right: #{kind: "variable", name: "batch"}
                            },
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "consumer"
                },
                #{asset: "decisions", quantity: #{kind: "constant", value: "1"}, role: "consumer"}
            ],
            computed_preserve: [
                #{asset: "market-open", quantity: #{kind: "constant", value: "1"}, role: "consumer"},
                #{asset: "trading", quantity: #{kind: "constant", value: "1"}, role: "consumer"}
            ],
            computed_produce: [
                #{
                    asset: "spent-energy",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{kind: "variable", name: "energy"},
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "consumer"
                },
                #{
                    asset: "utility",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "utility"}, right: #{kind: "units"}},
                    role: "consumer"
                },
                #{
                    asset: "goods",
                    quantity: #{
                        kind: "multiply",
                        left: #{
                            kind: "multiply",
                            left: #{
                                kind: "multiply",
                                left: #{kind: "variable", name: "batches"},
                                right: #{kind: "variable", name: "batch"}
                            },
                            right: #{kind: "variable", name: "scale"}
                        },
                        right: #{kind: "units"}
                    },
                    role: "consumer"
                }
            ],
            consume: [["consumer", [["need/{actor}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["consumer", [["satisfied-need/{actor}", "1"]]]],
            variables: [#{name: "actor", values: ["factory", "household"]}],
            relations: [
                #{
                    gives: ["energy", "utility", "batches"],
                    rows: [["factory", "1500", "150", "1"], ["household", "350", "100", "0"]],
                    variables: ["actor"]
                }
            ]
        }
    ]
]
