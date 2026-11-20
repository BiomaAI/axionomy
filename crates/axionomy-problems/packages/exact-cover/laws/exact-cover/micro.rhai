[
    #{
        expand: "select/bd/{before}",
        rate: #{
            accounts: [["problem", "problem"]],
            consume: [
                [
                    "problem",
                    [
                        ["available/bd", "1"],
                        ["progress/{before}", "1"],
                        ["uncovered/b", "1"],
                        ["uncovered/d", "1"]
                    ]
                ]
            ],
            distinct: [],
            preserve: [["problem", [["active", "1"]]]],
            produce: [
                [
                    "problem",
                    [["selected/bd", "1"], ["progress/{after}", "1"], ["covered/b", "1"], ["covered/d", "1"]]
                ]
            ],
            variables: [#{name: "before", values: ["0", "1", "2"]}],
            relations: [#{variables: ["before"], gives: ["after"], rows: [["0", "2"], ["1", "3"], ["2", "4"]]}]
        }
    }
]
