let model = #{
    ontology: "work-league",
    laws: ["work-league/season"],
    tables: #{
        "begin/{agent}/{job}/lean": #{
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}, #{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "begin/{agent}/{job}/rush": #{
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}, #{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "begin/{agent}/{job}/safe": #{
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}, #{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "claim/{agent}/{job}": #{
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}, #{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "expire/{job}/none/none": #{
            variables: [#{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "expire/{job}/some/{agent}/none": #{
            variables: [#{name: "job", range: ["1", "12"]}, #{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "expire/{job}/some/{agent}/some/{mode}": #{
            variables: [
                #{name: "job", range: ["1", "12"]},
                #{name: "agent", values: ["atlas", "bolt", "coda", "delta"]},
                #{name: "mode", values: ["rush", "lean", "safe"]}
            ],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "finish/{agent}/{job}/{mode}/failure": #{
            variables: [
                #{name: "agent", values: ["atlas", "bolt", "coda", "delta"]},
                #{name: "job", range: ["1", "12"]},
                #{name: "mode", values: ["rush", "lean", "safe"]}
            ],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["weight"],
                    rows: [
                        ["1", "rush", "3"],
                        ["1", "lean", "1"],
                        ["1", "safe", "0"],
                        ["2", "rush", "6"],
                        ["2", "lean", "4"],
                        ["2", "safe", "3"],
                        ["3", "rush", "5"],
                        ["3", "lean", "3"],
                        ["3", "safe", "2"],
                        ["4", "rush", "4"],
                        ["4", "lean", "2"],
                        ["4", "safe", "1"],
                        ["5", "rush", "3"],
                        ["5", "lean", "1"],
                        ["5", "safe", "0"],
                        ["6", "rush", "6"],
                        ["6", "lean", "4"],
                        ["6", "safe", "3"],
                        ["7", "rush", "5"],
                        ["7", "lean", "3"],
                        ["7", "safe", "2"],
                        ["8", "rush", "4"],
                        ["8", "lean", "2"],
                        ["8", "safe", "1"],
                        ["9", "rush", "3"],
                        ["9", "lean", "1"],
                        ["9", "safe", "0"],
                        ["10", "rush", "6"],
                        ["10", "lean", "4"],
                        ["10", "safe", "3"],
                        ["11", "rush", "5"],
                        ["11", "lean", "3"],
                        ["11", "safe", "2"],
                        ["12", "rush", "4"],
                        ["12", "lean", "2"],
                        ["12", "safe", "1"]
                    ],
                    variables: ["job", "mode"]
                }
            ]
        },
        "finish/{agent}/{job}/{mode}/success": #{
            variables: [
                #{name: "agent", values: ["atlas", "bolt", "coda", "delta"]},
                #{name: "job", range: ["1", "12"]},
                #{name: "mode", values: ["rush", "lean", "safe"]}
            ],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["weight"],
                    rows: [
                        ["1", "rush", "7"],
                        ["1", "lean", "9"],
                        ["1", "safe", "10"],
                        ["2", "rush", "4"],
                        ["2", "lean", "6"],
                        ["2", "safe", "7"],
                        ["3", "rush", "5"],
                        ["3", "lean", "7"],
                        ["3", "safe", "8"],
                        ["4", "rush", "6"],
                        ["4", "lean", "8"],
                        ["4", "safe", "9"],
                        ["5", "rush", "7"],
                        ["5", "lean", "9"],
                        ["5", "safe", "10"],
                        ["6", "rush", "4"],
                        ["6", "lean", "6"],
                        ["6", "safe", "7"],
                        ["7", "rush", "5"],
                        ["7", "lean", "7"],
                        ["7", "safe", "8"],
                        ["8", "rush", "6"],
                        ["8", "lean", "8"],
                        ["8", "safe", "9"],
                        ["9", "rush", "7"],
                        ["9", "lean", "9"],
                        ["9", "safe", "10"],
                        ["10", "rush", "4"],
                        ["10", "lean", "6"],
                        ["10", "safe", "7"],
                        ["11", "rush", "5"],
                        ["11", "lean", "7"],
                        ["11", "safe", "8"],
                        ["12", "rush", "6"],
                        ["12", "lean", "8"],
                        ["12", "safe", "9"]
                    ],
                    variables: ["job", "mode"]
                }
            ]
        },
        "handoff/{agent}/{recipient}/{job}": #{
            variables: [
                #{name: "agent", values: ["atlas", "bolt", "coda", "delta"]},
                #{name: "recipient", values: ["atlas", "bolt", "coda", "delta"]},
                #{name: "job", range: ["1", "12"]}
            ],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "release-claim/{agent}/{job}/false": #{
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}, #{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        },
        "release-claim/{agent}/{job}/true": #{
            variables: [#{name: "agent", values: ["atlas", "bolt", "coda", "delta"]}, #{name: "job", range: ["1", "12"]}],
            relations: [
                #{
                    gives: ["place", "reward", "energy", "time", "material"],
                    rows: [
                        ["1", "north", "8", "3", "3", "2"],
                        ["2", "east", "15", "6", "6", "4"],
                        ["3", "south", "22", "4", "3", "2"],
                        ["4", "west", "12", "7", "6", "4"],
                        ["5", "north", "19", "5", "3", "2"],
                        ["6", "east", "9", "3", "6", "4"],
                        ["7", "south", "16", "6", "3", "2"],
                        ["8", "west", "23", "4", "6", "4"],
                        ["9", "north", "13", "7", "3", "2"],
                        ["10", "east", "20", "5", "6", "4"],
                        ["11", "south", "10", "3", "3", "2"],
                        ["12", "west", "17", "6", "6", "4"]
                    ],
                    variables: ["job"]
                },
                #{
                    gives: ["release", "deadline"],
                    rows: [
                        ["1", "0", "23"],
                        ["2", "0", "26"],
                        ["3", "0", "20"],
                        ["4", "0", "23"],
                        ["5", "6", "32"],
                        ["6", "6", "26"],
                        ["7", "6", "29"],
                        ["8", "6", "32"],
                        ["9", "12", "32"],
                        ["10", "12", "35"],
                        ["11", "12", "38"],
                        ["12", "12", "32"]
                    ],
                    variables: ["job"]
                }
            ]
        }
    },
    constants: #{"season-length": "48"}
};
let accounts = [];
accounts.push(opening(account("agent/atlas"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("credit"), 5),
    amount(asset("energy"), 14),
    amount(asset("free-contract"), 1),
    amount(asset("idle"), 1),
    amount(asset("material"), 11),
    amount(asset("operational"), 1),
    amount(asset("policy/steward"), 1),
    amount(asset("time-remaining"), 48)
])));
accounts.push(opening(account("agent/bolt"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("credit"), 5),
    amount(asset("energy"), 14),
    amount(asset("free-contract"), 1),
    amount(asset("idle"), 1),
    amount(asset("material"), 11),
    amount(asset("operational"), 1),
    amount(asset("policy/steward"), 1),
    amount(asset("time-remaining"), 48)
])));
accounts.push(opening(account("agent/coda"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("credit"), 5),
    amount(asset("energy"), 14),
    amount(asset("free-contract"), 1),
    amount(asset("idle"), 1),
    amount(asset("material"), 11),
    amount(asset("operational"), 1),
    amount(asset("policy/resilient"), 1),
    amount(asset("time-remaining"), 48)
])));
accounts.push(opening(account("agent/delta"), basket([
    amount(asset("at/depot"), 1),
    amount(asset("credit"), 5),
    amount(asset("energy"), 14),
    amount(asset("free-contract"), 1),
    amount(asset("idle"), 1),
    amount(asset("material"), 11),
    amount(asset("operational"), 1),
    amount(asset("policy/steward"), 1),
    amount(asset("time-remaining"), 48)
])));
accounts.push(opening(account("facility/charger"), basket([amount(asset("charge-supply"), 144), amount(asset("vacant/charger"), 1)])));
accounts.push(opening(account("facility/recycler"), basket([amount(asset("vacant/recycler"), 1)])));
accounts.push(opening(account("facility/workshop"), basket([amount(asset("repair-supply"), 12), amount(asset("vacant/workshop"), 1)])));
accounts.push(opening(account("job/1"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 8),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/2"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 15),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/3"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 22),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/4"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 12),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/5"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 19),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/6"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 9),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/7"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 16),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/8"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 23),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/9"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 13),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/10"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 20),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/11"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 10),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("job/12"), basket([
    amount(asset("available"), 1),
    amount(asset("credit"), 17),
    amount(asset("transferable"), 1)
])));
accounts.push(opening(account("world"), basket([
    amount(asset("contracts-open"), 12),
    amount(asset("permit/atlas"), 1),
    amount(asset("permit/bolt"), 1),
    amount(asset("permit/coda"), 1),
    amount(asset("permit/delta"), 1),
    amount(asset("season-active"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("one-service-slot/workshop");
    for name in [
        "vacant/workshop",
        "occupied/workshop/atlas",
        "occupied/workshop/bolt",
        "occupied/workshop/coda",
        "occupied/workshop/delta"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-service-slot/charger");
    for name in [
        "vacant/charger",
        "occupied/charger/atlas",
        "occupied/charger/bolt",
        "occupied/charger/coda",
        "occupied/charger/delta"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("one-service-slot/recycler");
    for name in [
        "vacant/recycler",
        "occupied/recycler/atlas",
        "occupied/recycler/bolt",
        "occupied/recycler/coda",
        "occupied/recycler/delta"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("decision-permission/atlas");
    rule.weight(asset("permit/atlas"), 1);
    rule.weight(asset("acted/atlas"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("decision-permission/bolt");
    rule.weight(asset("permit/bolt"), 1);
    rule.weight(asset("acted/bolt"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("decision-permission/coda");
    rule.weight(asset("permit/coda"), 1);
    rule.weight(asset("acted/coda"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("decision-permission/delta");
    rule.weight(asset("permit/delta"), 1);
    rule.weight(asset("acted/delta"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("season-lifecycle");
    rule.weight(asset("season-active"), 1);
    rule.weight(asset("season-ended"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-contracts");
    rule.weight(asset("contracts-open"), 1);
    rule.weight(asset("contracts-closed"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("worker-availability");
    rule.weight(asset("idle"), 1);
    rule.weight(asset("busy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("worker-condition");
    for name in ["operational", "damage", "repairing"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("energy-lifecycle");
    for name in ["energy", "spent-energy", "charge-supply", "charging-energy"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("material-lifecycle");
    for name in ["material", "waste", "embodied-material", "reclaiming-material", "recycling-loss"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("closed-settlement-credit");
    rule.weight(asset("credit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("repair-supplies");
    rule.weight(asset("repair-supply"), 1);
    rule.weight(asset("spent-repair-supply"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("worker-time");
    rule.weight(asset("time-remaining"), 1);
    rule.weight(asset("elapsed-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-contract-per-worker");
    for name in [
        "free-contract",
        "claimed/1",
        "claimed/2",
        "claimed/3",
        "claimed/4",
        "claimed/5",
        "claimed/6",
        "claimed/7",
        "claimed/8",
        "claimed/9",
        "claimed/10",
        "claimed/11",
        "claimed/12"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
