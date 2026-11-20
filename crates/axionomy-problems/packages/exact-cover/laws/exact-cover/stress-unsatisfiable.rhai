[
    #{
        expand: "select/ac/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/ac", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/a", "1"],
                        ["uncovered/c", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/ac", "1"], ["progress/{after}", "1"], ["covered/a", "1"], ["covered/c", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["7", "8", "9", "10"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [["7", "9"], ["8", "10"], ["9", "11"], ["10", "12"]]
                }
            ]
        }
    },
    #{
        expand: "select/ad/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/ad", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/a", "1"],
                        ["uncovered/d", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/ad", "1"], ["progress/{after}", "1"], ["covered/a", "1"], ["covered/d", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["7", "8", "9", "10"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [["7", "9"], ["8", "10"], ["9", "11"], ["10", "12"]]
                }
            ]
        }
    },
    #{
        expand: "select/aj/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/aj", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/a", "1"],
                        ["uncovered/j", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/aj", "1"], ["progress/{after}", "1"], ["covered/a", "1"], ["covered/j", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [
                        ["0", "2"],
                        ["1", "3"],
                        ["2", "4"],
                        ["3", "5"],
                        ["4", "6"],
                        ["5", "7"],
                        ["6", "8"],
                        ["7", "9"],
                        ["8", "10"],
                        ["9", "11"],
                        ["10", "12"]
                    ]
                }
            ]
        }
    },
    #{
        expand: "select/ak/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/ak", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/a", "1"],
                        ["uncovered/k", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/ak", "1"], ["progress/{after}", "1"], ["covered/a", "1"], ["covered/k", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [
                        ["0", "2"],
                        ["1", "3"],
                        ["2", "4"],
                        ["3", "5"],
                        ["4", "6"],
                        ["5", "7"],
                        ["6", "8"],
                        ["7", "9"],
                        ["8", "10"],
                        ["9", "11"],
                        ["10", "12"]
                    ]
                }
            ]
        }
    }
]
