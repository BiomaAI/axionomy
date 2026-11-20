#{
    start: "sokoban/micro",
    steps: [
        #{
            exchange: #{rate: "move/8/9", units: "1", bindings: #{from: "cell/8", puzzle: "success", to: "cell/9"}},
            status: "Applicable",
            balances: [["cell/8", "empty", "1"], ["cell/9", "player", "1"]],
            supply: #{player: "1"}
        }
    ]
}
