let model = #{
    ontology: "mission/expedition",
    laws: ["mission/expedition/common", "mission/expedition/showcase"],
    tables: #{
        "clinical/{objective}/{day}/false": #{variables: [#{name: "objective", values: ["0"]}, #{name: "day", range: ["8", "24"]}]},
        "clinical/{objective}/{day}/true": #{variables: [#{name: "objective", values: ["0"]}, #{name: "day", range: ["8", "24"]}]},
        "discover/{objective}/{location}": #{
            variables: [
                #{name: "objective", range: ["0", "1"]},
                #{name: "location", values: ["north", "east", "south", "west"]}
            ]
        },
        "extract/{objective}/{location}": #{
            variables: [
                #{name: "objective", values: ["0"]},
                #{name: "location", values: ["north", "east", "south", "west"]}
            ]
        },
        "fail/{objective}/carried/true": #{variables: [#{name: "objective", values: ["0"]}]},
        "fail/{objective}/hidden/false": #{variables: [#{name: "objective", range: ["0", "1"]}]},
        "fail/{objective}/hidden/true": #{variables: [#{name: "objective", values: ["0"]}]},
        "fail/{objective}/located/false": #{variables: [#{name: "objective", range: ["0", "1"]}]},
        "fail/{objective}/located/true": #{variables: [#{name: "objective", values: ["0"]}]},
        "finish-extract/{objective}": #{variables: [#{name: "objective", values: ["0"]}]},
        "share/{objective}/{location}/false": #{
            variables: [
                #{name: "objective", range: ["0", "1"]},
                #{name: "location", values: ["north", "east", "south", "west"]}
            ]
        },
        "share/{objective}/{location}/true": #{
            variables: [
                #{name: "objective", range: ["0", "1"]},
                #{name: "location", values: ["north", "east", "south", "west"]}
            ]
        },
        "tick/{day}": #{
            variables: [#{name: "day", range: ["0", "27"]}],
            relations: [
                #{
                    gives: ["weather-closes", "weather-clears"],
                    rows: [
                        ["0", "0", "0"],
                        ["1", "0", "0"],
                        ["2", "0", "0"],
                        ["3", "0", "0"],
                        ["4", "1", "0"],
                        ["5", "0", "0"],
                        ["6", "0", "0"],
                        ["7", "0", "0"],
                        ["8", "0", "0"],
                        ["9", "0", "1"],
                        ["10", "0", "0"],
                        ["11", "0", "0"],
                        ["12", "0", "0"],
                        ["13", "0", "0"],
                        ["14", "0", "0"],
                        ["15", "0", "0"],
                        ["16", "0", "0"],
                        ["17", "0", "0"],
                        ["18", "0", "0"],
                        ["19", "0", "0"],
                        ["20", "0", "0"],
                        ["21", "0", "0"],
                        ["22", "0", "0"],
                        ["23", "0", "0"],
                        ["24", "0", "0"],
                        ["25", "0", "0"],
                        ["26", "0", "0"],
                        ["27", "0", "0"]
                    ],
                    variables: ["day"]
                }
            ]
        },
        "treat-patient/{objective}/{location}/false": #{
            variables: [
                #{name: "objective", values: ["0"]},
                #{name: "location", values: ["north", "east", "south", "west"]}
            ]
        },
        "treat-patient/{objective}/{location}/true": #{
            variables: [
                #{name: "objective", values: ["0"]},
                #{name: "location", values: ["north", "east", "south", "west"]}
            ]
        }
    },
    constants: #{deadline: "26", horizon: "28", "objective-count": "2"}
};
let accounts = [];
accounts.push(opening(account("agent/medic"), basket([
    amount(asset("at/base"), 1),
    amount(asset("energy"), 16),
    amount(asset("health"), 2),
    amount(asset("medical-kit"), 2),
    amount(asset("ready"), 1),
    amount(asset("seat"), 1)
])));
accounts.push(opening(account("agent/scout"), basket([
    amount(asset("at/base"), 1),
    amount(asset("energy"), 16),
    amount(asset("health"), 2),
    amount(asset("radio"), 2),
    amount(asset("ready"), 1),
    amount(asset("scan-round/0"), 1),
    amount(asset("sensor"), 2),
    amount(asset("unsearched/east"), 1),
    amount(asset("unsearched/north"), 1),
    amount(asset("unsearched/south"), 1),
    amount(asset("unsearched/west"), 1)
])));
accounts.push(opening(account("nature"), basket([
    amount(asset("hazard-site/east"), 1),
    amount(asset("signal/0/east"), 1),
    amount(asset("signal/1/south"), 1),
    amount(asset("truth/0/east"), 1),
    amount(asset("truth/1/south"), 1)
])));
accounts.push(opening(account("objective/0"), basket([
    amount(asset("clinical-due/16"), 1),
    amount(asset("clinical-due/24"), 1),
    amount(asset("clinical-due/8"), 1),
    amount(asset("health"), 3),
    amount(asset("hidden"), 1),
    amount(asset("unstable"), 1)
])));
accounts.push(opening(account("objective/1"), basket([amount(asset("hidden"), 1)])));
accounts.push(opening(account("world"), basket([
    amount(asset("active"), 1),
    amount(asset("energy"), 8),
    amount(asset("environment-ready"), 1),
    amount(asset("medical-kit"), 2),
    amount(asset("next-clinical"), 8),
    amount(asset("permit/medic"), 1),
    amount(asset("permit/scout"), 1),
    amount(asset("supply"), 2),
    amount(asset("team-at/medic/base"), 1),
    amount(asset("team-at/scout/base"), 1),
    amount(asset("time-remaining"), 28),
    amount(asset("weather-open/base"), 1),
    amount(asset("weather-open/east"), 1),
    amount(asset("weather-open/north"), 1),
    amount(asset("weather-open/south"), 1),
    amount(asset("weather-open/west"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("clinical-pulse/8");
    rule.weight(asset("clinical-due/8"), 1);
    rule.weight(asset("clinical-settled/8"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("clinical-pulse/16");
    rule.weight(asset("clinical-due/16"), 1);
    rule.weight(asset("clinical-settled/16"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("clinical-pulse/24");
    rule.weight(asset("clinical-due/24"), 1);
    rule.weight(asset("clinical-settled/24"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-decision-per-tick/scout");
    rule.weight(asset("permit/scout"), 1);
    rule.weight(asset("used-permit/scout"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-decision-per-tick/medic");
    rule.weight(asset("permit/medic"), 1);
    rule.weight(asset("used-permit/medic"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("two-independent-agent-activities");
    for name in [
        "ready",
        "awaiting-report",
        "travelling/base",
        "travelling/north",
        "travelling/east",
        "travelling/south",
        "travelling/west",
        "collecting/0",
        "extracting/0",
        "collecting/1",
        "extracting/1"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("two-physical-agent-positions");
    for name in [
        "at/base",
        "travelling/base",
        "at/north",
        "travelling/north",
        "at/east",
        "travelling/east",
        "at/south",
        "travelling/south",
        "at/west",
        "travelling/west"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-sector-surveys");
    for name in [
        "unsearched/base",
        "surveyed/base",
        "unsearched/north",
        "surveyed/north",
        "unsearched/east",
        "surveyed/east",
        "unsearched/south",
        "surveyed/south",
        "unsearched/west",
        "surveyed/west"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-medic-evacuation-seat");
    for name in ["seat", "reserved-seat/0", "cargo/0"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("objective-lifecycle");
    for name in ["hidden", "located", "working", "carried", "complete", "failed"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("health-and-harm");
    rule.weight(asset("health"), 1);
    rule.weight(asset("harm"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("stabilization-state");
    rule.weight(asset("stable"), 1);
    rule.weight(asset("unstable"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("energy");
    rule.weight(asset("energy"), 1);
    rule.weight(asset("spent-energy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("medical-kits");
    rule.weight(asset("medical-kit"), 1);
    rule.weight(asset("used-medical-kit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("radio-charges");
    rule.weight(asset("radio"), 1);
    rule.weight(asset("used-radio"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("sensor-charges");
    rule.weight(asset("sensor"), 1);
    rule.weight(asset("used-sensor"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-resupply");
    rule.weight(asset("supply"), 1);
    rule.weight(asset("used-supply"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("time");
    rule.weight(asset("time"), 1);
    rule.weight(asset("time-remaining"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("mission-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("closed"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("weather-settlement");
    for name in ["environment-ready", "weather-due/false", "weather-due/true"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
