[
    [
        "accrue",
        #{
            accounts: [["borrower", "borrower"], ["clock", "clock"], ["market", "market"], ["oracle", "oracle"]],
            computed_produce: [
                #{
                    asset: "borrow-index",
                    quantity: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{asset: "borrow-index", kind: "balance", role: "market"},
                        right: #{
                            kind: "add",
                            left: #{kind: "variable", name: "base-rate"},
                            right: #{
                                kind: "add",
                                left: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{
                                        kind: "minimum",
                                        left: #{
                                            denominator: #{
                                                kind: "maximum",
                                                left: #{
                                                    kind: "add",
                                                    left: #{asset: "cash", kind: "balance", role: "market"},
                                                    right: #{
                                                        denominator: #{kind: "constant", value: "1000000000000000000"},
                                                        kind: "mul_div",
                                                        left: #{
                                                            asset: "debt-share-supply",
                                                            kind: "balance",
                                                            role: "market"
                                                        },
                                                        right: #{
                                                            asset: "borrow-index",
                                                            kind: "balance",
                                                            role: "market"
                                                        },
                                                        rounding: "floor"
                                                    }
                                                },
                                                right: #{kind: "constant", value: "1"}
                                            },
                                            kind: "mul_div",
                                            left: #{
                                                denominator: #{kind: "constant", value: "1000000000000000000"},
                                                kind: "mul_div",
                                                left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                                right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                                rounding: "floor"
                                            },
                                            right: #{kind: "constant", value: "1000000000000000000"},
                                            rounding: "ceil"
                                        },
                                        right: #{kind: "variable", name: "kink"}
                                    },
                                    right: #{kind: "variable", name: "slope-below-kink"},
                                    rounding: "ceil"
                                },
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{
                                        kind: "subtract",
                                        left: #{
                                            kind: "maximum",
                                            left: #{
                                                denominator: #{
                                                    kind: "maximum",
                                                    left: #{
                                                        kind: "add",
                                                        left: #{asset: "cash", kind: "balance", role: "market"},
                                                        right: #{
                                                            denominator: #{kind: "constant", value: "1000000000000000000"},
                                                            kind: "mul_div",
                                                            left: #{
                                                                asset: "debt-share-supply",
                                                                kind: "balance",
                                                                role: "market"
                                                            },
                                                            right: #{
                                                                asset: "borrow-index",
                                                                kind: "balance",
                                                                role: "market"
                                                            },
                                                            rounding: "floor"
                                                        }
                                                    },
                                                    right: #{kind: "constant", value: "1"}
                                                },
                                                kind: "mul_div",
                                                left: #{
                                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                                    kind: "mul_div",
                                                    left: #{
                                                        asset: "debt-share-supply",
                                                        kind: "balance",
                                                        role: "market"
                                                    },
                                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                                    rounding: "floor"
                                                },
                                                right: #{kind: "constant", value: "1000000000000000000"},
                                                rounding: "ceil"
                                            },
                                            right: #{kind: "variable", name: "kink"}
                                        },
                                        right: #{kind: "variable", name: "kink"}
                                    },
                                    right: #{kind: "variable", name: "slope-above-kink"},
                                    rounding: "ceil"
                                }
                            }
                        },
                        rounding: "ceil"
                    },
                    role: "market"
                },
                #{
                    asset: "reserves",
                    quantity: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{
                            kind: "subtract",
                            left: #{
                                denominator: #{kind: "constant", value: "1000000000000000000"},
                                kind: "mul_div",
                                left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                right: #{
                                    kind: "add",
                                    left: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    right: #{
                                        denominator: #{kind: "constant", value: "1000000000000000000"},
                                        kind: "mul_div",
                                        left: #{asset: "borrow-index", kind: "balance", role: "market"},
                                        right: #{
                                            kind: "add",
                                            left: #{kind: "variable", name: "base-rate"},
                                            right: #{
                                                kind: "add",
                                                left: #{
                                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                                    kind: "mul_div",
                                                    left: #{
                                                        kind: "minimum",
                                                        left: #{
                                                            denominator: #{
                                                                kind: "maximum",
                                                                left: #{
                                                                    kind: "add",
                                                                    left: #{
                                                                        asset: "cash",
                                                                        kind: "balance",
                                                                        role: "market"
                                                                    },
                                                                    right: #{
                                                                        denominator: #{
                                                                            kind: "constant",
                                                                            value: "1000000000000000000"
                                                                        },
                                                                        kind: "mul_div",
                                                                        left: #{
                                                                            asset: "debt-share-supply",
                                                                            kind: "balance",
                                                                            role: "market"
                                                                        },
                                                                        right: #{
                                                                            asset: "borrow-index",
                                                                            kind: "balance",
                                                                            role: "market"
                                                                        },
                                                                        rounding: "floor"
                                                                    }
                                                                },
                                                                right: #{kind: "constant", value: "1"}
                                                            },
                                                            kind: "mul_div",
                                                            left: #{
                                                                denominator: #{
                                                                    kind: "constant",
                                                                    value: "1000000000000000000"
                                                                },
                                                                kind: "mul_div",
                                                                left: #{
                                                                    asset: "debt-share-supply",
                                                                    kind: "balance",
                                                                    role: "market"
                                                                },
                                                                right: #{
                                                                    asset: "borrow-index",
                                                                    kind: "balance",
                                                                    role: "market"
                                                                },
                                                                rounding: "floor"
                                                            },
                                                            right: #{kind: "constant", value: "1000000000000000000"},
                                                            rounding: "ceil"
                                                        },
                                                        right: #{kind: "variable", name: "kink"}
                                                    },
                                                    right: #{kind: "variable", name: "slope-below-kink"},
                                                    rounding: "ceil"
                                                },
                                                right: #{
                                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                                    kind: "mul_div",
                                                    left: #{
                                                        kind: "subtract",
                                                        left: #{
                                                            kind: "maximum",
                                                            left: #{
                                                                denominator: #{
                                                                    kind: "maximum",
                                                                    left: #{
                                                                        kind: "add",
                                                                        left: #{
                                                                            asset: "cash",
                                                                            kind: "balance",
                                                                            role: "market"
                                                                        },
                                                                        right: #{
                                                                            denominator: #{
                                                                                kind: "constant",
                                                                                value: "1000000000000000000"
                                                                            },
                                                                            kind: "mul_div",
                                                                            left: #{
                                                                                asset: "debt-share-supply",
                                                                                kind: "balance",
                                                                                role: "market"
                                                                            },
                                                                            right: #{
                                                                                asset: "borrow-index",
                                                                                kind: "balance",
                                                                                role: "market"
                                                                            },
                                                                            rounding: "floor"
                                                                        }
                                                                    },
                                                                    right: #{kind: "constant", value: "1"}
                                                                },
                                                                kind: "mul_div",
                                                                left: #{
                                                                    denominator: #{
                                                                        kind: "constant",
                                                                        value: "1000000000000000000"
                                                                    },
                                                                    kind: "mul_div",
                                                                    left: #{
                                                                        asset: "debt-share-supply",
                                                                        kind: "balance",
                                                                        role: "market"
                                                                    },
                                                                    right: #{
                                                                        asset: "borrow-index",
                                                                        kind: "balance",
                                                                        role: "market"
                                                                    },
                                                                    rounding: "floor"
                                                                },
                                                                right: #{
                                                                    kind: "constant",
                                                                    value: "1000000000000000000"
                                                                },
                                                                rounding: "ceil"
                                                            },
                                                            right: #{kind: "variable", name: "kink"}
                                                        },
                                                        right: #{kind: "variable", name: "kink"}
                                                    },
                                                    right: #{kind: "variable", name: "slope-above-kink"},
                                                    rounding: "ceil"
                                                }
                                            }
                                        },
                                        rounding: "ceil"
                                    }
                                },
                                rounding: "floor"
                            },
                            right: #{
                                denominator: #{kind: "constant", value: "1000000000000000000"},
                                kind: "mul_div",
                                left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                rounding: "floor"
                            }
                        },
                        right: #{kind: "variable", name: "reserve-factor"},
                        rounding: "ceil"
                    },
                    role: "market"
                }
            ],
            conditions: [
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    name: "the-loan-is-not-liquidatable",
                    right: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{
                            kind: "multiply",
                            left: #{asset: "posted-collateral", kind: "balance", role: "borrower"},
                            right: #{asset: "collateral-price", kind: "balance", role: "market"}
                        },
                        right: #{kind: "variable", name: "liquidation-threshold"},
                        rounding: "floor"
                    }
                },
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        kind: "multiply",
                        left: #{asset: "pending-shock", kind: "balance", role: "oracle"},
                        right: #{
                            kind: "add",
                            left: #{asset: "elapsed-period", kind: "balance", role: "clock"},
                            right: #{kind: "constant", value: "1"}
                        }
                    },
                    name: "the-scheduled-price-drop-has-happened",
                    right: #{kind: "variable", name: "shock-after"}
                },
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-period-at-a-time",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["clock", [["period", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["clock", [["elapsed-period", "1"]]]]
        }
    ],
    [
        "borrow",
        #{
            accounts: [["borrower", "borrower"], ["market", "market"]],
            computed_produce: [
                #{
                    asset: "debt-share",
                    quantity: #{
                        denominator: #{asset: "borrow-index", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{kind: "constant", value: "1000000000000000000"},
                        rounding: "ceil"
                    },
                    role: "borrower"
                },
                #{
                    asset: "debt-share-supply",
                    quantity: #{
                        denominator: #{asset: "borrow-index", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{kind: "constant", value: "1000000000000000000"},
                        rounding: "ceil"
                    },
                    role: "market"
                }
            ],
            conditions: [
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{
                            kind: "add",
                            left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                            right: #{
                                denominator: #{asset: "borrow-index", kind: "balance", role: "market"},
                                kind: "mul_div",
                                left: #{kind: "units"},
                                right: #{kind: "constant", value: "1000000000000000000"},
                                rounding: "ceil"
                            }
                        },
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    name: "within-the-loan-to-value-limit",
                    right: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{
                            kind: "multiply",
                            left: #{asset: "posted-collateral", kind: "balance", role: "borrower"},
                            right: #{asset: "collateral-price", kind: "balance", role: "market"}
                        },
                        right: #{kind: "variable", name: "loan-to-value"},
                        rounding: "floor"
                    }
                }
            ],
            consume: [["market", [["cash", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["borrower", [["cash", "1"]]]]
        }
    ],
    [
        "collect-revenue/{installment}",
        #{
            accounts: [["borrower", "borrower"], ["clock", "clock"], ["customer", "customer"]],
            computed_consume: [#{asset: "cash", quantity: #{kind: "variable", name: "amount"}, role: "customer"}],
            computed_produce: [#{asset: "cash", quantity: #{kind: "variable", name: "amount"}, role: "borrower"}],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{asset: "elapsed-period", kind: "balance", role: "clock"},
                    name: "the-installment-is-due",
                    right: #{kind: "variable", name: "due"}
                }
            ],
            consume: [["customer", [["purchase-order/{installment}", "1"]]]],
            distinct: [],
            preserve: [],
            produce: []
        }
    ],
    [
        "deposit",
        #{
            accounts: [["market", "market"]],
            computed_produce: [
                #{
                    asset: "share",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{
                                kind: "add",
                                left: #{asset: "cash", kind: "balance", role: "market"},
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "floor"
                                }
                            },
                            right: #{asset: "reserves", kind: "balance", role: "market"}
                        },
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{asset: "share-supply", kind: "balance", role: "market"},
                        rounding: "floor"
                    },
                    role: "depositor"
                },
                #{
                    asset: "share-supply",
                    quantity: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{
                                kind: "add",
                                left: #{asset: "cash", kind: "balance", role: "market"},
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "floor"
                                }
                            },
                            right: #{asset: "reserves", kind: "balance", role: "market"}
                        },
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{asset: "share-supply", kind: "balance", role: "market"},
                        rounding: "floor"
                    },
                    role: "market"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        denominator: #{
                            kind: "subtract",
                            left: #{
                                kind: "add",
                                left: #{asset: "cash", kind: "balance", role: "market"},
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "floor"
                                }
                            },
                            right: #{asset: "reserves", kind: "balance", role: "market"}
                        },
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{asset: "share-supply", kind: "balance", role: "market"},
                        rounding: "floor"
                    },
                    name: "mints-at-least-the-minimum-shares",
                    right: #{
                        kind: "maximum",
                        left: #{kind: "parameter", name: "min-shares"},
                        right: #{kind: "constant", value: "1"}
                    }
                }
            ],
            consume: [["depositor", [["cash", "1"]]]],
            distinct: [["depositor", "market"]],
            preserve: [],
            produce: [["market", [["cash", "1"]]]]
        }
    ],
    [
        "donate",
        #{
            accounts: [["market", "market"]],
            consume: [["depositor", [["cash", "1"]]]],
            distinct: [["depositor", "market"]],
            preserve: [],
            produce: [["market", [["cash", "1"]]]]
        }
    ],
    [
        "liquidate",
        #{
            accounts: [["borrower", "borrower"], ["market", "market"]],
            computed_consume: [
                #{
                    asset: "cash",
                    quantity: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    role: "keeper"
                },
                #{
                    asset: "debt-share",
                    quantity: #{asset: "debt-share", kind: "balance", role: "borrower"},
                    role: "borrower"
                },
                #{
                    asset: "debt-share-supply",
                    quantity: #{asset: "debt-share", kind: "balance", role: "borrower"},
                    role: "market"
                },
                #{
                    asset: "posted-collateral",
                    quantity: #{
                        kind: "minimum",
                        left: #{asset: "posted-collateral", kind: "balance", role: "borrower"},
                        right: #{
                            denominator: #{asset: "collateral-price", kind: "balance", role: "market"},
                            kind: "mul_div",
                            left: #{
                                denominator: #{kind: "constant", value: "1000000000000000000"},
                                kind: "mul_div",
                                left: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "ceil"
                                },
                                right: #{
                                    kind: "add",
                                    left: #{kind: "constant", value: "1000000000000000000"},
                                    right: #{kind: "variable", name: "liquidation-bonus"}
                                },
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        }
                    },
                    role: "borrower"
                }
            ],
            computed_produce: [
                #{
                    asset: "cash",
                    quantity: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    role: "market"
                },
                #{
                    asset: "collateral",
                    quantity: #{
                        kind: "minimum",
                        left: #{asset: "posted-collateral", kind: "balance", role: "borrower"},
                        right: #{
                            denominator: #{asset: "collateral-price", kind: "balance", role: "market"},
                            kind: "mul_div",
                            left: #{
                                denominator: #{kind: "constant", value: "1000000000000000000"},
                                kind: "mul_div",
                                left: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "ceil"
                                },
                                right: #{
                                    kind: "add",
                                    left: #{kind: "constant", value: "1000000000000000000"},
                                    right: #{kind: "variable", name: "liquidation-bonus"}
                                },
                                rounding: "floor"
                            },
                            right: #{kind: "constant", value: "1"},
                            rounding: "floor"
                        }
                    },
                    role: "keeper"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    name: "the-loan-is-past-its-liquidation-threshold",
                    right: #{
                        kind: "add",
                        left: #{
                            denominator: #{kind: "constant", value: "1000000000000000000"},
                            kind: "mul_div",
                            left: #{
                                kind: "multiply",
                                left: #{asset: "posted-collateral", kind: "balance", role: "borrower"},
                                right: #{asset: "collateral-price", kind: "balance", role: "market"}
                            },
                            right: #{kind: "variable", name: "liquidation-threshold"},
                            rounding: "floor"
                        },
                        right: #{kind: "constant", value: "1"}
                    }
                },
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-liquidation",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [],
            distinct: [["borrower", "keeper"], ["keeper", "market"]],
            preserve: [],
            produce: []
        }
    ],
    [
        "post-collateral",
        #{
            accounts: [["borrower", "borrower"], ["market", "market"]],
            consume: [["borrower", [["collateral", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["borrower", [["posted-collateral", "1"]]]]
        }
    ],
    [
        "redeem",
        #{
            accounts: [["market", "market"]],
            computed_consume: [
                #{
                    asset: "cash",
                    quantity: #{
                        denominator: #{asset: "share-supply", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{
                            kind: "subtract",
                            left: #{
                                kind: "add",
                                left: #{asset: "cash", kind: "balance", role: "market"},
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "floor"
                                }
                            },
                            right: #{asset: "reserves", kind: "balance", role: "market"}
                        },
                        rounding: "floor"
                    },
                    role: "market"
                }
            ],
            computed_produce: [
                #{
                    asset: "cash",
                    quantity: #{
                        denominator: #{asset: "share-supply", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{
                            kind: "subtract",
                            left: #{
                                kind: "add",
                                left: #{asset: "cash", kind: "balance", role: "market"},
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "floor"
                                }
                            },
                            right: #{asset: "reserves", kind: "balance", role: "market"}
                        },
                        rounding: "floor"
                    },
                    role: "depositor"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        denominator: #{asset: "share-supply", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{
                            kind: "subtract",
                            left: #{
                                kind: "add",
                                left: #{asset: "cash", kind: "balance", role: "market"},
                                right: #{
                                    denominator: #{kind: "constant", value: "1000000000000000000"},
                                    kind: "mul_div",
                                    left: #{asset: "debt-share-supply", kind: "balance", role: "market"},
                                    right: #{asset: "borrow-index", kind: "balance", role: "market"},
                                    rounding: "floor"
                                }
                            },
                            right: #{asset: "reserves", kind: "balance", role: "market"}
                        },
                        rounding: "floor"
                    },
                    name: "pays-at-least-the-minimum-cash",
                    right: #{kind: "parameter", name: "min-cash"}
                }
            ],
            consume: [["depositor", [["share", "1"]]], ["market", [["share-supply", "1"]]]],
            distinct: [["depositor", "market"]],
            preserve: [],
            produce: []
        }
    ],
    [
        "repay",
        #{
            accounts: [["borrower", "borrower"], ["market", "market"]],
            computed_consume: [
                #{
                    asset: "debt-share",
                    quantity: #{
                        denominator: #{asset: "borrow-index", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{kind: "constant", value: "1000000000000000000"},
                        rounding: "floor"
                    },
                    role: "borrower"
                },
                #{
                    asset: "debt-share-supply",
                    quantity: #{
                        denominator: #{asset: "borrow-index", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{kind: "constant", value: "1000000000000000000"},
                        rounding: "floor"
                    },
                    role: "market"
                }
            ],
            conditions: [
                #{
                    comparison: "greater_than_or_equal",
                    left: #{
                        denominator: #{asset: "borrow-index", kind: "balance", role: "market"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{kind: "constant", value: "1000000000000000000"},
                        rounding: "floor"
                    },
                    name: "retires-debt",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["borrower", [["cash", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["market", [["cash", "1"]]]]
        }
    ],
    [
        "repay-shares",
        #{
            accounts: [["borrower", "borrower"], ["market", "market"]],
            computed_consume: [
                #{
                    asset: "cash",
                    quantity: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    role: "borrower"
                }
            ],
            computed_produce: [
                #{
                    asset: "cash",
                    quantity: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{kind: "units"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    role: "market"
                }
            ],
            consume: [["borrower", [["debt-share", "1"]]], ["market", [["debt-share-supply", "1"]]]],
            distinct: [],
            preserve: [],
            produce: []
        }
    ],
    [
        "reprice",
        #{
            accounts: [["clock", "clock"], ["market", "market"], ["oracle", "oracle"]],
            computed_consume: [#{asset: "collateral-price", quantity: #{kind: "variable", name: "price-drop"}, role: "market"}],
            conditions: [
                #{
                    comparison: "equal",
                    left: #{asset: "elapsed-period", kind: "balance", role: "clock"},
                    name: "at-the-scheduled-period",
                    right: #{kind: "variable", name: "shock-after"}
                },
                #{
                    comparison: "equal",
                    left: #{kind: "units"},
                    name: "one-price-drop",
                    right: #{kind: "constant", value: "1"}
                }
            ],
            consume: [["oracle", [["pending-shock", "1"]]]],
            distinct: [],
            preserve: [],
            produce: []
        }
    ],
    [
        "withdraw-collateral",
        #{
            accounts: [["borrower", "borrower"], ["market", "market"]],
            conditions: [
                #{
                    comparison: "less_than_or_equal",
                    left: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{asset: "debt-share", kind: "balance", role: "borrower"},
                        right: #{asset: "borrow-index", kind: "balance", role: "market"},
                        rounding: "ceil"
                    },
                    name: "the-remaining-collateral-covers-the-loan",
                    right: #{
                        denominator: #{kind: "constant", value: "1000000000000000000"},
                        kind: "mul_div",
                        left: #{
                            kind: "multiply",
                            left: #{
                                kind: "subtract",
                                left: #{asset: "posted-collateral", kind: "balance", role: "borrower"},
                                right: #{kind: "units"}
                            },
                            right: #{asset: "collateral-price", kind: "balance", role: "market"}
                        },
                        right: #{kind: "variable", name: "loan-to-value"},
                        rounding: "floor"
                    }
                }
            ],
            consume: [["borrower", [["posted-collateral", "1"]]]],
            distinct: [],
            preserve: [],
            produce: [["borrower", [["collateral", "1"]]]]
        }
    ]
]
