[
    #{
        expand: "select/ab/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/ab", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/a", "1"],
                        ["uncovered/b", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/ab", "1"], ["progress/{after}", "1"], ["covered/a", "1"], ["covered/b", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["3", "4", "5", "6"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [["3", "5"], ["4", "6"], ["5", "7"], ["6", "8"]]
                }
            ]
        }
    },
    #{
        expand: "select/ae/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/ae", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/a", "1"],
                        ["uncovered/e", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/ae", "1"], ["progress/{after}", "1"], ["covered/a", "1"], ["covered/e", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["0", "1", "2", "3", "4", "5", "6"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [["0", "2"], ["1", "3"], ["2", "4"], ["3", "5"], ["4", "6"], ["5", "7"], ["6", "8"]]
                }
            ]
        }
    },
    #{
        expand: "select/fgh/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/fgh", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/f", "1"],
                        ["uncovered/g", "1"],
                        ["uncovered/h", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [
                        ["selected/fgh", "1"],
                        ["progress/{after}", "1"],
                        ["covered/f", "1"],
                        ["covered/g", "1"],
                        ["covered/h", "1"]
                    ]
                ]
            ],
            variables: [#{name: "before", values: ["0", "1", "2", "3", "4", "5"]}],
            relations: [
                #{
                    variables: ["before"],
                    gives: ["after"],
                    rows: [["0", "3"], ["1", "4"], ["2", "5"], ["3", "6"], ["4", "7"], ["5", "8"]]
                }
            ]
        }
    }
]
