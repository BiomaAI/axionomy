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
                }
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
                }
            ],
            consume: [],
            distinct: [["liquidity-provider", "pool"]],
            preserve: [],
            produce: []
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
                }
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
                }
            ],
            consume: [["trader", [["credit", "1"]]]],
            distinct: [["pool", "trader"]],
            preserve: [],
            produce: [["pool", [["credit", "1"]]]]
        }
    ],
    [
        "learn-shortage/speculator",
        #{
            accounts: [["information", "information"], ["observer", "actor/speculator"]],
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
                }
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
                }
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
                }
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
                }
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
                }
            ],
            consume: [["consumer", [["need/{actor}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["consumer", [["satisfied-need/{actor}", "1"]]]],
            variables: [#{name: "actor", values: ["factory", "household"]}],
            relations: [
                #{
                    gives: ["energy", "utility"],
                    rows: [["factory", "1500", "150"], ["household", "350", "100"]],
                    variables: ["actor"]
                }
            ]
        }
    ]
]
