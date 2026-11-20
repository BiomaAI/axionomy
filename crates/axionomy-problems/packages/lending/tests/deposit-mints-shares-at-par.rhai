#{
    start: "lending/micro",
    steps: [
        #{
            exchange: #{
                rate: "deposit",
                units: "5",
                bindings: #{market: "market", depositor: "depositor/0"},
                parameters: #{"min-shares": "5"}
            },
            status: "Applicable",
            balances: [
                ["market", "cash", "7005"],
                ["market", "share-supply", "7005"],
                ["depositor/0", "share", "3005"],
                ["depositor/0", "cash", "1995"]
            ]
        },
        #{
            exchange: #{
                rate: "deposit",
                units: "5",
                bindings: #{market: "market", depositor: "depositor/0"},
                parameters: #{"min-shares": "6"}
            },
            status: "Invalid",
            balances: [["market", "cash", "7005"], ["depositor/0", "share", "3005"]]
        }
    ]
}
