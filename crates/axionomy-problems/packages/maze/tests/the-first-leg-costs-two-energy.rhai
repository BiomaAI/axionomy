#{
    start: "maze/micro",
    steps: [
        #{
            exchange: #{
                rate: "move/start/key-room/2/false",
                units: "1",
                bindings: #{actor: "agent", environment: "world"}
            },
            status: "Applicable",
            balances: [["agent", "energy", "7"], ["agent", "time", "5"], ["agent", "at/key-room", "1"]]
        }
    ]
}
