let model = #{
    ontology: "rescue/incident",
    laws: ["rescue/incident/common"],
    tables: #{
        "deteriorate/{patient}": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "discover/{patient}/{location}": #{
            variables: [
                #{name: "patient", range: ["0", "3"]},
                #{name: "location", values: ["north", "south", "east", "harbor", "west"]}
            ]
        },
        "drive/{from}/{to}/{fast}": #{
            variables: [
                #{name: "from", values: ["base", "north", "south", "east", "harbor", "west"]},
                #{name: "to", values: ["base", "north", "south", "east", "harbor", "west"]},
                #{name: "fast", values: ["false", "true"]}
            ],
            relations: [
                #{
                    gives: ["distance"],
                    rows: [
                        ["base", "north", "1"],
                        ["base", "south", "1"],
                        ["base", "east", "2"],
                        ["base", "harbor", "1"],
                        ["base", "west", "2"],
                        ["north", "base", "1"],
                        ["north", "south", "2"],
                        ["north", "east", "1"],
                        ["north", "harbor", "2"],
                        ["north", "west", "3"],
                        ["south", "base", "1"],
                        ["south", "north", "2"],
                        ["south", "east", "3"],
                        ["south", "harbor", "2"],
                        ["south", "west", "1"],
                        ["east", "base", "2"],
                        ["east", "north", "1"],
                        ["east", "south", "3"],
                        ["east", "harbor", "1"],
                        ["east", "west", "2"],
                        ["harbor", "base", "1"],
                        ["harbor", "north", "2"],
                        ["harbor", "south", "2"],
                        ["harbor", "east", "1"],
                        ["harbor", "west", "1"],
                        ["west", "base", "2"],
                        ["west", "north", "3"],
                        ["west", "south", "1"],
                        ["west", "east", "2"],
                        ["west", "harbor", "1"]
                    ],
                    variables: ["from", "to"]
                },
                #{
                    gives: ["ticks-per-road", "fuel-per-road"],
                    rows: [["false", "2", "1"], ["true", "1", "2"]],
                    variables: ["fast"]
                }
            ]
        },
        "load/{patient}/{location}": #{
            variables: [
                #{name: "patient", range: ["0", "3"]},
                #{name: "location", values: ["north", "south", "east", "harbor", "west"]}
            ]
        },
        "lose/{patient}/found/deadline": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/found/deterioration": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/found/horizon": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/hidden/deadline": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/hidden/deterioration": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/hidden/horizon": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/onboard/deadline": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/onboard/deterioration": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "lose/{patient}/onboard/horizon": #{variables: [#{name: "patient", range: ["0", "3"]}]},
        "report/{round}/{location}": #{
            variables: [
                #{name: "round", range: ["0", "1"]},
                #{name: "location", values: ["north", "south", "east", "harbor", "west"]}
            ]
        },
        "search/{location}": #{variables: [#{name: "location", values: ["north", "south", "east", "harbor", "west"]}]},
        "stabilize/{patient}/{location}/false": #{
            variables: [
                #{name: "patient", range: ["0", "3"]},
                #{name: "location", values: ["north", "south", "east", "harbor", "west"]}
            ]
        },
        "stabilize/{patient}/{location}/true": #{
            variables: [
                #{name: "patient", range: ["0", "3"]},
                #{name: "location", values: ["north", "south", "east", "harbor", "west"]}
            ]
        },
        "unload/{patient}": #{variables: [#{name: "patient", range: ["0", "3"]}]}
    },
    constants: #{horizon: "30", "patient-count": "4"}
};
let accounts = [];
accounts.push(opening(account("nature"), basket([
    amount(asset("signal/0/east"), 1),
    amount(asset("signal/1/south"), 1),
    amount(asset("truth/0/east"), 1),
    amount(asset("truth/1/south"), 1),
    amount(asset("truth/2/north"), 1),
    amount(asset("truth/3/west"), 1)
])));
accounts.push(opening(account("patient/0"), basket([
    amount(asset("alive"), 1),
    amount(asset("deadline"), 16),
    amount(asset("hidden"), 1),
    amount(asset("next-decline"), 6),
    amount(asset("treatable"), 1),
    amount(asset("vitality"), 3)
])));
accounts.push(opening(account("patient/1"), basket([
    amount(asset("alive"), 1),
    amount(asset("deadline"), 17),
    amount(asset("hidden"), 1),
    amount(asset("next-decline"), 7),
    amount(asset("treatable"), 1),
    amount(asset("vitality"), 3)
])));
accounts.push(opening(account("patient/2"), basket([
    amount(asset("alive"), 1),
    amount(asset("deadline"), 20),
    amount(asset("hidden"), 1),
    amount(asset("next-decline"), 10),
    amount(asset("treatable"), 1),
    amount(asset("vitality"), 3)
])));
accounts.push(opening(account("patient/3"), basket([
    amount(asset("alive"), 1),
    amount(asset("deadline"), 16),
    amount(asset("hidden"), 1),
    amount(asset("next-decline"), 6),
    amount(asset("treatable"), 1),
    amount(asset("vitality"), 3)
])));
accounts.push(opening(account("responder"), basket([
    amount(asset("at/base"), 1),
    amount(asset("energy"), 10),
    amount(asset("fuel"), 7),
    amount(asset("medical-kit"), 2),
    amount(asset("ready"), 1),
    amount(asset("scan-round/0"), 1),
    amount(asset("seat"), 1),
    amount(asset("sensor"), 2),
    amount(asset("unsearched/east"), 1),
    amount(asset("unsearched/harbor"), 1),
    amount(asset("unsearched/north"), 1),
    amount(asset("unsearched/south"), 1),
    amount(asset("unsearched/west"), 1)
])));
accounts.push(opening(account("world"), basket([
    amount(asset("active"), 1),
    amount(asset("energy"), 6),
    amount(asset("fuel"), 4),
    amount(asset("medical-kit"), 1),
    amount(asset("supply-available"), 1),
    amount(asset("time-remaining"), 30)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("one-evacuation-seat");
    for name in ["seat", "carrying/0", "carrying/1", "carrying/2", "carrying/3"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("responder-activity");
    rule.weight(asset("ready"), 1);
    rule.weight(asset("awaiting-scan"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-survey-per-sector");
    for name in [
        "unsearched/north",
        "surveyed/north",
        "unsearched/south",
        "surveyed/south",
        "unsearched/east",
        "surveyed/east",
        "unsearched/harbor",
        "surveyed/harbor",
        "unsearched/west",
        "surveyed/west"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("patient-outcomes");
    for name in ["alive", "saved", "dead", "left-behind"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("clinical-vitality");
    rule.weight(asset("vitality"), 1);
    rule.weight(asset("harm"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-medicine");
    rule.weight(asset("medical-kit"), 1);
    rule.weight(asset("used-medical-kit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-fuel");
    rule.weight(asset("fuel"), 1);
    rule.weight(asset("spent-fuel"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-effort");
    rule.weight(asset("energy"), 1);
    rule.weight(asset("spent-energy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("sensor-charges");
    rule.weight(asset("sensor"), 1);
    rule.weight(asset("used-sensor"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("resupply-opportunity");
    rule.weight(asset("supply-available"), 1);
    rule.weight(asset("supply-used"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("bounded-incident-time");
    rule.weight(asset("time"), 1);
    rule.weight(asset("time-remaining"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("incident-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("closed"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
