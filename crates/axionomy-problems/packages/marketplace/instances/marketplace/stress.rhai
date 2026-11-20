let model = #{
    ontology: "marketplace",
    laws: ["marketplace/showcase-and-stress"],
    tables: #{
        "cancel/{order}/{discounted}": #{
            variables: [
                #{name: "order", values: ["a", "b", "c", "d", "e", "f"]},
                #{name: "discounted", values: ["false", "true"]}
            ],
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
                        ["b", "gadget", "90", "72", "9", "5", "4", "25", "18"],
                        ["c", "widget", "110", "86", "11", "7", "6", "34", "17"],
                        ["d", "gadget", "95", "75", "10", "5", "5", "22", "24"],
                        ["e", "widget", "105", "83", "10", "6", "6", "28", "21"],
                        ["f", "gadget", "115", "90", "12", "7", "6", "31", "19"]
                    ],
                    variables: ["order"]
                },
                #{
                    gives: ["arrival", "deadline", "route-delay"],
                    rows: [
                        ["a", "0", "4", "0"],
                        ["b", "0", "4", "0"],
                        ["c", "2", "7", "2"],
                        ["d", "2", "7", "0"],
                        ["e", "4", "10", "2"],
                        ["f", "4", "10", "0"]
                    ],
                    variables: ["order"]
                },
                #{gives: ["discount"], rows: [["false", "0"], ["true", "10"]], variables: ["discounted"]}
            ]
        },
        "contract/{order}/{discounted}": #{
            variables: [
                #{name: "order", values: ["a", "b", "c", "d", "e", "f"]},
                #{name: "discounted", values: ["false", "true"]}
            ],
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
                        ["b", "gadget", "90", "72", "9", "5", "4", "25", "18"],
                        ["c", "widget", "110", "86", "11", "7", "6", "34", "17"],
                        ["d", "gadget", "95", "75", "10", "5", "5", "22", "24"],
                        ["e", "widget", "105", "83", "10", "6", "6", "28", "21"],
                        ["f", "gadget", "115", "90", "12", "7", "6", "31", "19"]
                    ],
                    variables: ["order"]
                },
                #{
                    gives: ["arrival", "deadline", "route-delay"],
                    rows: [
                        ["a", "0", "4", "0"],
                        ["b", "0", "4", "0"],
                        ["c", "2", "7", "2"],
                        ["d", "2", "7", "0"],
                        ["e", "4", "10", "2"],
                        ["f", "4", "10", "0"]
                    ],
                    variables: ["order"]
                },
                #{gives: ["discount"], rows: [["false", "0"], ["true", "10"]], variables: ["discounted"]}
            ]
        },
        "deliver/{order}/{discounted}/false": #{
            variables: [
                #{name: "order", values: ["a", "b", "c", "d", "e", "f"]},
                #{name: "discounted", values: ["false", "true"]}
            ],
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
                        ["b", "gadget", "90", "72", "9", "5", "4", "25", "18"],
                        ["c", "widget", "110", "86", "11", "7", "6", "34", "17"],
                        ["d", "gadget", "95", "75", "10", "5", "5", "22", "24"],
                        ["e", "widget", "105", "83", "10", "6", "6", "28", "21"],
                        ["f", "gadget", "115", "90", "12", "7", "6", "31", "19"]
                    ],
                    variables: ["order"]
                },
                #{
                    gives: ["arrival", "deadline", "route-delay"],
                    rows: [
                        ["a", "0", "4", "0"],
                        ["b", "0", "4", "0"],
                        ["c", "2", "7", "2"],
                        ["d", "2", "7", "0"],
                        ["e", "4", "10", "2"],
                        ["f", "4", "10", "0"]
                    ],
                    variables: ["order"]
                },
                #{gives: ["discount"], rows: [["false", "0"], ["true", "10"]], variables: ["discounted"]}
            ]
        },
        "deliver/{order}/{discounted}/true": #{
            variables: [
                #{name: "order", values: ["a", "b", "c", "d", "e", "f"]},
                #{name: "discounted", values: ["false", "true"]}
            ],
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
                        ["b", "gadget", "90", "72", "9", "5", "4", "25", "18"],
                        ["c", "widget", "110", "86", "11", "7", "6", "34", "17"],
                        ["d", "gadget", "95", "75", "10", "5", "5", "22", "24"],
                        ["e", "widget", "105", "83", "10", "6", "6", "28", "21"],
                        ["f", "gadget", "115", "90", "12", "7", "6", "31", "19"]
                    ],
                    variables: ["order"]
                },
                #{
                    gives: ["arrival", "deadline", "route-delay"],
                    rows: [
                        ["a", "0", "4", "0"],
                        ["b", "0", "4", "0"],
                        ["c", "2", "7", "2"],
                        ["d", "2", "7", "0"],
                        ["e", "4", "10", "2"],
                        ["f", "4", "10", "0"]
                    ],
                    variables: ["order"]
                },
                #{gives: ["discount"], rows: [["false", "0"], ["true", "10"]], variables: ["discounted"]}
            ]
        },
        "dispatch/{order}/{carrier}": #{
            variables: [#{name: "order", values: ["a", "b", "c", "d", "e", "f"]}, #{name: "carrier", values: ["a", "b"]}],
            relations: [
                #{
                    gives: ["arrival", "deadline", "route-delay"],
                    rows: [
                        ["a", "0", "4", "0"],
                        ["b", "0", "4", "0"],
                        ["c", "2", "7", "2"],
                        ["d", "2", "7", "0"],
                        ["e", "4", "10", "2"],
                        ["f", "4", "10", "0"]
                    ],
                    variables: ["order"]
                },
                #{
                    gives: ["transit", "exposure"],
                    rows: [["a", "2", "1"], ["b", "3", "0"]],
                    variables: ["carrier"]
                }
            ]
        }
    }
};
let accounts = [];
accounts.push(opening(account("buyer/a"), basket([
    amount(asset("buyer-license"), 1),
    amount(asset("money"), 330),
    amount(asset("purchase-intent/a"), 1),
    amount(asset("purchase-intent/c"), 1),
    amount(asset("purchase-intent/e"), 1)
])));
accounts.push(opening(account("buyer/b"), basket([
    amount(asset("buyer-license"), 1),
    amount(asset("money"), 300),
    amount(asset("purchase-intent/b"), 1),
    amount(asset("purchase-intent/f"), 1)
])));
accounts.push(opening(account("buyer/c"), basket([
    amount(asset("buyer-license"), 1),
    amount(asset("money"), 350),
    amount(asset("purchase-intent/d"), 1)
])));
accounts.push(opening(account("carrier/a"), basket([amount(asset("carrier-license"), 1), amount(asset("shipping-capacity"), 1)])));
accounts.push(opening(account("carrier/b"), basket([amount(asset("carrier-license"), 1), amount(asset("shipping-capacity"), 1)])));
accounts.push(opening(account("order/a"), basket([amount(asset("open-order/a"), 1)])));
accounts.push(opening(account("order/b"), basket([amount(asset("open-order/b"), 1)])));
accounts.push(opening(account("order/c"), basket([amount(asset("open-order/c"), 1)])));
accounts.push(opening(account("order/d"), basket([amount(asset("open-order/d"), 1)])));
accounts.push(opening(account("order/e"), basket([amount(asset("open-order/e"), 1)])));
accounts.push(opening(account("order/f"), basket([amount(asset("open-order/f"), 1)])));
accounts.push(opening(account("platform"), basket([amount(asset("time-remaining"), 32)])));
accounts.push(opening(account("seller/a"), basket([
    amount(asset("item/widget"), 2),
    amount(asset("sale-offer/widget"), 2),
    amount(asset("seller-license"), 1)
])));
accounts.push(opening(account("seller/b"), basket([
    amount(asset("item/gadget"), 1),
    amount(asset("item/widget"), 1),
    amount(asset("sale-offer/gadget"), 1),
    amount(asset("sale-offer/widget"), 1),
    amount(asset("seller-license"), 1)
])));
accounts.push(opening(account("seller/c"), basket([
    amount(asset("item/gadget"), 2),
    amount(asset("sale-offer/gadget"), 2),
    amount(asset("seller-license"), 1)
])));
accounts.push(opening(account("tax-authority"), basket([])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("money-accounting");
    rule.weight(asset("money"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("item-accounting");
    rule.weight(asset("item/widget"), 1);
    rule.weight(asset("item/gadget"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("buyer-order-lifecycle");
    for name in [
        "purchase-intent/a",
        "purchase-receipt/a",
        "purchase-intent/b",
        "purchase-receipt/b",
        "purchase-intent/c",
        "purchase-receipt/c",
        "purchase-intent/d",
        "purchase-receipt/d",
        "purchase-intent/e",
        "purchase-receipt/e",
        "purchase-intent/f",
        "purchase-receipt/f",
        "purchase-locked/a",
        "purchase-locked/b",
        "purchase-locked/c",
        "purchase-locked/d",
        "purchase-locked/e",
        "purchase-locked/f"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("seller-order-lifecycle");
    for name in [
        "sale-offer/widget",
        "sale-offer/gadget",
        "completed-sale/a",
        "completed-sale/b",
        "completed-sale/c",
        "completed-sale/d",
        "completed-sale/e",
        "completed-sale/f",
        "sale-reserved/a",
        "sale-reserved/b",
        "sale-reserved/c",
        "sale-reserved/d",
        "sale-reserved/e",
        "sale-reserved/f"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("shipping-capacity-accounting");
    rule.weight(asset("shipping-capacity"), 1);
    rule.weight(asset("used-shipping-capacity"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("order-lifecycle");
    for name in [
        "open-order/a",
        "settled-order/a",
        "open-order/b",
        "settled-order/b",
        "open-order/c",
        "settled-order/c",
        "open-order/d",
        "settled-order/d",
        "open-order/e",
        "settled-order/e",
        "open-order/f",
        "settled-order/f",
        "contract/a",
        "in-shipment/a",
        "contract/b",
        "in-shipment/b",
        "contract/c",
        "in-shipment/c",
        "contract/d",
        "in-shipment/d",
        "contract/e",
        "in-shipment/e",
        "contract/f",
        "in-shipment/f"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("market-time-budget");
    rule.weight(asset("now"), 1);
    rule.weight(asset("time-remaining"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
