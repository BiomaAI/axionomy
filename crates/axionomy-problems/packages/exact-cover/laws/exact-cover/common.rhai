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
            variables: [#{name: "before", values: ["0", "1", "2"]}],
            relations: [#{variables: ["before"], gives: ["after"], rows: [["0", "2"], ["1", "3"], ["2", "4"]]}]
        }
    }
]
