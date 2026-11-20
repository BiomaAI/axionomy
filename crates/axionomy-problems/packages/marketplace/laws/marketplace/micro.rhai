[
    [
        "discount-order/{order}",
        #{
            accounts: [["order", "order/{order}"], ["platform", "platform"], ["tax-authority", "tax-authority"]],
            computed_consume: [
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "gross"}, right: #{kind: "units"}},
                    role: "buyer"
                }
            ],
            computed_produce: [
                #{
                    asset: "utility",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "buyer-utility"},
                        right: #{kind: "units"}
                    },
                    role: "buyer"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "proceeds"}, right: #{kind: "units"}},
                    role: "seller"
                },
                #{
                    asset: "utility",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "seller-utility"},
                        right: #{kind: "units"}
                    },
                    role: "seller"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "commission"}, right: #{kind: "units"}},
                    role: "platform"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "tax"}, right: #{kind: "units"}},
                    role: "tax-authority"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "shipping"}, right: #{kind: "units"}},
                    role: "carrier"
                }
            ],
            consume: [
                ["buyer", [["purchase-intent/{order}", "1"]]],
                ["carrier", [["shipping-capacity", "1"]]],
                ["order", [["open-order/{order}", "1"]]],
                ["seller", [["item/{item}", "1"], ["sale-offer/{item}", "1"]]]
            ],
            distinct: [
                ["buyer", "carrier"],
                ["buyer", "order"],
                ["buyer", "platform"],
                ["buyer", "seller"],
                ["buyer", "tax-authority"],
                ["carrier", "order"],
                ["carrier", "platform"],
                ["carrier", "seller"],
                ["carrier", "tax-authority"],
                ["order", "seller"],
                ["platform", "seller"],
                ["seller", "tax-authority"]
            ],
            preserve: [],
            produce: [
                ["buyer", [["item/{item}", "1"], ["purchase-receipt/{order}", "1"]]],
                ["carrier", [["used-shipping-capacity", "1"]]],
                ["order", [["settled-order/{order}", "1"], ["settled-value/{gross}", "1"]]],
                ["seller", [["completed-sale/{order}", "1"]]]
            ],
            variables: [#{name: "order", values: ["a"]}],
            relations: [
                #{
                    gives: [
                        "item",
                        "gross",
                        "proceeds",
                        "tax",
                        "commission",
                        "shipping",
                        "buyer-utility",
                        "seller-utility"
                    ],
                    rows: [["a", "widget", "93", "73", "10", "5", "5", "37", "13"]],
                    variables: ["order"]
                }
            ]
        }
    ],
    [
        "settle-order/{order}",
        #{
            accounts: [["order", "order/{order}"], ["platform", "platform"], ["tax-authority", "tax-authority"]],
            computed_consume: [
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "gross"}, right: #{kind: "units"}},
                    role: "buyer"
                }
            ],
            computed_produce: [
                #{
                    asset: "utility",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "buyer-utility"},
                        right: #{kind: "units"}
                    },
                    role: "buyer"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "proceeds"}, right: #{kind: "units"}},
                    role: "seller"
                },
                #{
                    asset: "utility",
                    quantity: #{
                        kind: "multiply",
                        left: #{kind: "variable", name: "seller-utility"},
                        right: #{kind: "units"}
                    },
                    role: "seller"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "commission"}, right: #{kind: "units"}},
                    role: "platform"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "tax"}, right: #{kind: "units"}},
                    role: "tax-authority"
                },
                #{
                    asset: "money",
                    quantity: #{kind: "multiply", left: #{kind: "variable", name: "shipping"}, right: #{kind: "units"}},
                    role: "carrier"
                }
            ],
            consume: [
                ["buyer", [["purchase-intent/{order}", "1"]]],
                ["carrier", [["shipping-capacity", "1"]]],
                ["order", [["open-order/{order}", "1"]]],
                ["seller", [["item/{item}", "1"], ["sale-offer/{item}", "1"]]]
            ],
            distinct: [
                ["buyer", "carrier"],
                ["buyer", "order"],
                ["buyer", "platform"],
                ["buyer", "seller"],
                ["buyer", "tax-authority"],
                ["carrier", "order"],
                ["carrier", "platform"],
                ["carrier", "seller"],
                ["carrier", "tax-authority"],
                ["order", "seller"],
                ["platform", "seller"],
                ["seller", "tax-authority"]
            ],
            preserve: [],
            produce: [
                ["buyer", [["item/{item}", "1"], ["purchase-receipt/{order}", "1"]]],
                ["carrier", [["used-shipping-capacity", "1"]]],
                ["order", [["settled-order/{order}", "1"], ["settled-value/{gross}", "1"]]],
                ["seller", [["completed-sale/{order}", "1"]]]
            ],
            variables: [#{name: "order", values: ["a", "b"]}],
            relations: [
                #{
                    gives: [
                        "item",
                        "gross",
                        "proceeds",
                        "tax",
                        "commission",
                        "shipping",
                        "buyer-utility",
                        "seller-utility"
                    ],
                    rows: [
                        ["a", "widget", "100", "80", "10", "5", "5", "30", "20"],
                        ["b", "gadget", "90", "72", "9", "5", "4", "25", "18"]
                    ],
                    variables: ["order"]
                }
            ]
        }
    ]
]
