let model = #{
    ontology: "logistics/fleet",
    laws: ["logistics/fleet/common"],
    tables: #{
        "deliver/{van}/{order}/false": #{
            variables: [#{name: "van", values: ["cold", "freight"]}, #{name: "order", range: ["0", "5"]}],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "deliver/{van}/{order}/true": #{
            variables: [#{name: "van", values: ["cold", "freight"]}, #{name: "order", values: ["1", "2", "4", "5"]}],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "expire/{order}/none/{at}": #{
            variables: [#{name: "order", range: ["0", "5"]}, #{name: "at", values: ["depot", "hub"]}],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "expire/{order}/some/{van}/depot": #{
            variables: [#{name: "order", range: ["0", "5"]}, #{name: "van", values: ["cold", "freight"]}],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "load/{van}/{order}/{at}": #{
            variables: [
                #{name: "van", values: ["cold", "freight"]},
                #{name: "order", range: ["0", "5"]},
                #{name: "at", values: ["depot", "hub"]}
            ],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "release/{order}": #{
            variables: [#{name: "order", range: ["0", "5"]}],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "stage/{van}/{order}": #{
            variables: [#{name: "van", values: ["cold", "freight"]}, #{name: "order", range: ["0", "5"]}],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        },
        "transfer/{from}/{to}/{order}/{at}": #{
            variables: [
                #{name: "from", values: ["cold", "freight"]},
                #{name: "order", range: ["0", "5"]},
                #{name: "to", values: ["cold", "freight"]},
                #{name: "at", values: ["depot", "hub", "north", "south", "east"]}
            ],
            relations: [
                #{
                    gives: ["destination", "weight", "cold", "price", "arrival", "deadline", "expiry"],
                    rows: [
                        ["0", "north", "1", "1", "24", "0", "12", "13"],
                        ["1", "south", "3", "0", "28", "0", "18", "52"],
                        ["2", "south", "1", "0", "16", "0", "17", "52"],
                        ["3", "east", "1", "1", "24", "7", "30", "31"],
                        ["4", "east", "3", "0", "28", "10", "40", "52"],
                        ["5", "north", "1", "0", "16", "13", "44", "52"]
                    ],
                    variables: ["order"]
                }
            ]
        }
    },
    constants: #{horizon: "52", orders: "6"}
};
let accounts = [];
accounts.push(opening(account("nature"), basket([
    amount(asset("ticket-word/cold/depot/hub"), 74766790688833),
    amount(asset("ticket-word/cold/depot/north"), 145139896952916),
    amount(asset("ticket-word/cold/east/north"), 275951718401),
    amount(asset("ticket-word/cold/east/south"), 18338562320),
    amount(asset("ticket-word/cold/hub/depot"), 22012047130704),
    amount(asset("ticket-word/cold/hub/north"), 89147751546944),
    amount(asset("ticket-word/cold/hub/south"), 17185177605),
    amount(asset("ticket-word/cold/north/depot"), 70368752566400),
    amount(asset("ticket-word/cold/north/east"), 292062037012),
    amount(asset("ticket-word/cold/north/hub"), 17609449817089),
    amount(asset("ticket-word/cold/south/east"), 5368729668),
    amount(asset("ticket-word/cold/south/hub"), 71812928001029),
    amount(asset("ticket-word/freight/depot/hub"), 18989212517632),
    amount(asset("ticket-word/freight/depot/north"), 140123341204),
    amount(asset("ticket-word/freight/east/north"), 1444204843009),
    amount(asset("ticket-word/freight/east/south"), 5501854433600),
    amount(asset("ticket-word/freight/hub/depot"), 22334920790080),
    amount(asset("ticket-word/freight/hub/north"), 4398316279044),
    amount(asset("ticket-word/freight/hub/south"), 4741661724676),
    amount(asset("ticket-word/freight/north/depot"), 36283917271680),
    amount(asset("ticket-word/freight/north/east"), 75866320228352),
    amount(asset("ticket-word/freight/north/hub"), 5501925462033),
    amount(asset("ticket-word/freight/south/east"), 17596486259793),
    amount(asset("ticket-word/freight/south/hub"), 71743133978704)
])));
accounts.push(opening(account("order/0"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("money"), 24),
    amount(asset("scheduled"), 1)
])));
accounts.push(opening(account("order/1"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("money"), 28),
    amount(asset("scheduled"), 1)
])));
accounts.push(opening(account("order/2"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("money"), 16),
    amount(asset("scheduled"), 1)
])));
accounts.push(opening(account("order/3"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("money"), 24),
    amount(asset("scheduled"), 1)
])));
accounts.push(opening(account("order/4"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("money"), 28),
    amount(asset("scheduled"), 1)
])));
accounts.push(opening(account("order/5"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("money"), 16),
    amount(asset("scheduled"), 1)
])));
accounts.push(opening(account("station"), basket([
    amount(asset("fuel"), 18),
    amount(asset("fuel-pump"), 1),
    amount(asset("part"), 3),
    amount(asset("repair-bay"), 1),
    amount(asset("tow-truck"), 1)
])));
accounts.push(opening(account("vehicle/cold"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("cold-space"), 1),
    amount(asset("fuel"), 5),
    amount(asset("integrity"), 1),
    amount(asset("ready"), 1),
    amount(asset("space"), 2),
    amount(asset("tank-space"), 3),
    amount(asset("ticket-divisor"), 1)
])));
accounts.push(opening(account("vehicle/freight"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("fuel"), 5),
    amount(asset("integrity"), 1),
    amount(asset("ready"), 1),
    amount(asset("space"), 3),
    amount(asset("tank-space"), 3),
    amount(asset("ticket-divisor"), 1)
])));
accounts.push(opening(account("world"), basket([
    amount(asset("active"), 1),
    amount(asset("dock-space"), 2),
    amount(asset("money"), 20),
    amount(asset("permit/cold"), 1),
    amount(asset("permit/freight"), 1),
    amount(asset("road-space"), 1),
    amount(asset("time-remaining"), 52)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("dispatch-permit/cold");
    rule.weight(asset("permit/cold"), 1);
    rule.weight(asset("used-permit/cold"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("dispatch-permit/freight");
    rule.weight(asset("permit/freight"), 1);
    rule.weight(asset("used-permit/freight"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("physical-vehicle-cargo-capacity");
    rule.weight(asset("space"), 1);
    rule.weight(asset("cargo/0"), 1);
    rule.weight(asset("cargo/1"), 3);
    rule.weight(asset("cargo/2"), 1);
    rule.weight(asset("cargo/3"), 1);
    rule.weight(asset("cargo/4"), 3);
    rule.weight(asset("cargo/5"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-refrigerated-cargo-slot");
    for name in ["cold-space", "cargo/0", "cargo/3"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("two-vehicle-activities");
    for name in [
        "ready",
        "refueling",
        "repairing",
        "towing",
        "travelling/depot/hub",
        "travelling/hub/depot",
        "travelling/hub/north",
        "travelling/north/hub",
        "travelling/hub/south",
        "travelling/south/hub",
        "travelling/north/east",
        "travelling/east/north",
        "travelling/south/east",
        "travelling/east/south",
        "travelling/depot/north",
        "travelling/north/depot"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("cargo-lifecycle");
    for name in ["scheduled", "waiting", "in-transit", "delivered", "expired"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-occupied-trunk-road");
    rule.weight(asset("road-space"), 1);
    rule.weight(asset("on-main-road"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-hub-staging-capacity");
    rule.weight(asset("dock-space"), 1);
    rule.weight(asset("dock-used"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("fuel-stock");
    rule.weight(asset("fuel"), 1);
    rule.weight(asset("spent-fuel"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("funded-delivery-and-service-payments");
    rule.weight(asset("money"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("vehicle-condition");
    rule.weight(asset("integrity"), 1);
    rule.weight(asset("damage"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("fuel-pump-occupancy");
    rule.weight(asset("fuel-pump"), 1);
    rule.weight(asset("refueling"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("repair-bay-occupancy");
    rule.weight(asset("repair-bay"), 1);
    rule.weight(asset("repairing"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("tow-truck-occupancy");
    rule.weight(asset("tow-truck"), 1);
    rule.weight(asset("towing"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-repair-parts");
    rule.weight(asset("part"), 1);
    rule.weight(asset("used-part"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("bounded-fleet-clock");
    rule.weight(asset("time"), 1);
    rule.weight(asset("time-remaining"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("fleet-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("closed"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
