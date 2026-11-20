#{
    start: "connect-four/micro",
    steps: [
        #{
            exchange: #{rate: "drop/red/0/0", units: "1", bindings: #{cell: "cell/0/0", column: "column/0", game: "game"}},
            status: "Applicable",
            balances: [["cell/0/0", "piece/red", "1"], ["column/0", "height", "1"], ["game", "turn/yellow", "1"]]
        }
    ]
}
