[
    [
        "finish",
        #{
            accounts: [["shop", "workshop"]],
            computed_consume: [#{asset: "offcut", quantity: #{asset: "offcut", kind: "balance", role: "shop"}, role: "shop"}],
            computed_produce: [#{asset: "waste", quantity: #{asset: "offcut", kind: "balance", role: "shop"}, role: "shop"}],
            consume: [["shop", [["active", "1"], ["idle", "1"]]]],
            distinct: [],
            preserve: [["shop", [["completed", "2"]]]],
            produce: [["shop", [["solved", "1"]]]]
        }
    ]
]
