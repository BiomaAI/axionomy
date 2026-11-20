[
    [
        "basic-chair",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["wood", "2"], ["labor", "1"], ["time", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["tool", "1"]]]],
            produce: [["shop", [["chair", "1"], ["waste", "1"], ["spent-labor", "1"], ["spent-time", "1"]]]]
        }
    ],
    [
        "counterfeit-chair",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["wood", "1"], ["time", "1"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"]]]],
            produce: [["shop", [["chair", "2"], ["spent-time", "1"]]]]
        }
    ],
    [
        "efficient-batch",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["wood", "3"], ["labor", "2"], ["time", "3"]]]],
            distinct: [],
            preserve: [["shop", [["active", "1"], ["tool", "1"]]]],
            produce: [["shop", [["chair", "2"], ["waste", "1"], ["spent-labor", "2"], ["spent-time", "3"]]]]
        }
    ],
    [
        "finish",
        #{
            accounts: [["shop", "workshop"]],
            consume: [["shop", [["active", "1"]]]],
            distinct: [],
            preserve: [["shop", [["chair", "2"]]]],
            produce: [["shop", [["solved", "1"]]]]
        }
    ]
]
