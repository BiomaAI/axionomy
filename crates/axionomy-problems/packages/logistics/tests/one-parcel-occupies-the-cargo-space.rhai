#{
    start: "logistics/micro",
    steps: [
        #{
            exchange: #{rate: "load/a", units: "1", bindings: #{order: "order/a", vehicle: "vehicle"}},
            status: "Applicable",
            balances: [["vehicle", "cargo-space", "0"], ["vehicle", "package/a", "1"], ["order/a", "in-transit", "1"]],
            supply: #{"package/a": "1"}
        }
    ]
}
