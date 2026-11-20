let definitions = #{
    candidates: #{"perishables/storage-plans": #{Script: #{function: "storage_plans", script: "perishables"}}},
    experiments: #{
        "perishables/micro": #{
            arms: [#{start: (), strategy: "perishables/outage"}],
            effort: #{budget: 10000000, seed: 17, turns: 9},
            goal: "perishables/solved",
            objectives: ["perishables/claims", "perishables/usable", "perishables/energy"],
            start: "perishables/micro",
            trials: 1
        },
        "perishables/micro/storage-plans": #{
            arms: [
                #{start: (), strategy: "perishables/pareto-inventory"},
                #{start: (), strategy: "perishables/pareto-energy"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "perishables/plan-sealed",
            objectives: ["perishables/claims", "perishables/usable", "perishables/energy"],
            start: "perishables/micro",
            trials: 1
        },
        "perishables/showcase": #{
            arms: [
                #{start: (), strategy: "perishables/cold-chain/outage"},
                #{start: (), strategy: "perishables/cold-chain/pareto-inventory"},
                #{start: (), strategy: "perishables/cold-chain/pareto-energy"},
                #{start: (), strategy: "perishables/cold-chain/storage-buffer"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "perishables/cold-chain/closed",
            objectives: [
                "perishables/cold-chain/saved",
                "perishables/cold-chain/delivered",
                "perishables/cold-chain/revenue",
                "perishables/cold-chain/energy",
                "perishables/cold-chain/fuel",
                "perishables/cold-chain/missed"
            ],
            start: "perishables/showcase",
            trials: 1
        },
        "perishables/stress": #{
            arms: [
                #{start: (), strategy: "perishables/cold-chain/outage"},
                #{start: (), strategy: "perishables/cold-chain/pareto-inventory"},
                #{start: (), strategy: "perishables/cold-chain/pareto-energy"},
                #{start: (), strategy: "perishables/cold-chain/storage-buffer"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "perishables/cold-chain/closed",
            objectives: [
                "perishables/cold-chain/saved",
                "perishables/cold-chain/delivered",
                "perishables/cold-chain/revenue",
                "perishables/cold-chain/energy",
                "perishables/cold-chain/fuel",
                "perishables/cold-chain/missed"
            ],
            start: "perishables/stress",
            trials: 1
        }
    },
    objectives: #{
        "perishables/claims": #{direction: "maximize", measure: #{State: #{function: "claims", script: "perishables"}}},
        "perishables/cold-chain/delivered": #{
            direction: "maximize",
            measure: #{State: #{function: "delivered_food", script: "perishables/cold-chain"}}
        },
        "perishables/cold-chain/energy": #{
            direction: "minimize",
            measure: #{State: #{function: "energy_used", script: "perishables/cold-chain"}}
        },
        "perishables/cold-chain/fuel": #{direction: "minimize", measure: #{State: #{function: "fuel_used", script: "perishables/cold-chain"}}},
        "perishables/cold-chain/missed": #{
            direction: "minimize",
            measure: #{State: #{function: "missed_demand", script: "perishables/cold-chain"}}
        },
        "perishables/cold-chain/revenue": #{direction: "maximize", measure: #{State: #{function: "revenue", script: "perishables/cold-chain"}}},
        "perishables/cold-chain/saved": #{direction: "maximize", measure: #{State: #{function: "saved", script: "perishables/cold-chain"}}},
        "perishables/energy": #{direction: "minimize", measure: #{State: #{function: "cooling_energy", script: "perishables"}}},
        "perishables/usable": #{direction: "maximize", measure: #{State: #{function: "usable_claims", script: "perishables"}}}
    },
    strategies: #{
        "perishables/cold-chain/outage": #{Plan: #{plan: #{function: "protect_cold", script: "perishables/cold-chain"}}},
        "perishables/cold-chain/pareto-energy": #{Plan: #{plan: #{function: "conserve_fuel", script: "perishables/cold-chain"}}},
        "perishables/cold-chain/pareto-inventory": #{Plan: #{plan: #{function: "process_first", script: "perishables/cold-chain"}}},
        "perishables/cold-chain/storage-buffer": #{Plan: #{plan: #{function: "rebalance_cold", script: "perishables/cold-chain"}}},
        "perishables/outage": #{Policy: #{policy: #{function: "outage", script: "perishables"}}},
        "perishables/pareto-energy": #{
            Pareto: #{
                by: ["perishables/energy", "perishables/usable"],
                candidates: "perishables/storage-plans",
                objectives: ["perishables/usable", "perishables/energy"]
            }
        },
        "perishables/pareto-inventory": #{
            Pareto: #{
                by: ["perishables/usable", "perishables/energy"],
                candidates: "perishables/storage-plans",
                objectives: ["perishables/usable", "perishables/energy"]
            }
        }
    },
    views: #{
        perishables: #{
            carry: (),
            formats: #{function: "format", script: "perishables/view"},
            labels: #{function: "label", script: "perishables/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "perishables/view"},
            title: "Perishable inventory"
        },
        "perishables/cold-chain": #{
            carry: (),
            formats: #{function: "format", script: "perishables/cold-chain/view"},
            labels: #{function: "label", script: "perishables/cold-chain/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "perishables/cold-chain/view"},
            title: "Cold-chain operations"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("world"), basket([amount(asset("solved"), 1)]));
    goals["perishables/cold-chain/closed"] = #{ontology: "perishables/cold-chain", goal: target};
}
{
    let target = goal();
    target = target.require(account("world"), basket([amount(asset("plan-solved"), 1)]));
    goals["perishables/plan-sealed"] = #{ontology: "perishables", goal: target};
}
{
    let target = goal();
    target = target.require(account("world"), basket([amount(asset("solved"), 1)]));
    goals["perishables/solved"] = #{ontology: "perishables", goal: target};
}
definitions.goals = goals;
definitions
