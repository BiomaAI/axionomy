#{
    start: "perishables/micro",
    steps: [
        #{
            exchange: #{
                rate: "move-to-fridge",
                units: "10",
                bindings: #{
                    world: "world",
                    "source-storage": "storage/warehouse",
                    "destination-storage": "storage/fridge",
                    "source-cohort": "cohort/ambient",
                    "destination-cohort": "cohort/refrigerated"
                }
            },
            status: "Applicable",
            balances: [
                ["storage/fridge", "cooling-energy", "60"],
                ["storage/fridge", "claim/refrigerated", "40"],
                ["storage/warehouse", "claim/ambient", "60"]
            ]
        }
    ]
}
