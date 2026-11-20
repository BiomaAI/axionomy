[
    #{
        expand: "select/cd/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/cd", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/c", "1"],
                        ["uncovered/d", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/cd", "1"], ["progress/{after}", "1"], ["covered/c", "1"], ["covered/d", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["0", "1", "2"]}],
            relations: [#{variables: ["before"], gives: ["after"], rows: [["0", "2"], ["1", "3"], ["2", "4"]]}]
        }
    }
]
