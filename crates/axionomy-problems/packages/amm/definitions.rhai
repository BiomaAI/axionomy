let definitions = #{
    experiments: #{
        "amm/micro": #{
            arms: [
                #{start: (), strategy: "amm/day/market-day"},
                #{start: (), strategy: "amm/day/no-whale"},
                #{start: (), strategy: "amm/day/thin-liquidity"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "amm/day/met",
            objectives: ["amm/needs", "amm/utility", "amm/depth", "amm/unmet", "amm/defaults"],
            start: "amm/micro",
            trials: 1
        },
        "amm/showcase": #{
            arms: [
                #{start: (), strategy: "amm/market-day"},
                #{start: (), strategy: "amm/no-whale"},
                #{start: (), strategy: "amm/thin-liquidity"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 697},
            goal: "amm/closed",
            objectives: ["amm/needs", "amm/utility", "amm/depth", "amm/unmet", "amm/defaults"],
            start: "amm/showcase",
            trials: 1
        },
        "amm/stress": #{
            arms: [
                #{start: (), strategy: "amm/market-day"},
                #{start: (), strategy: "amm/no-whale"},
                #{start: (), strategy: "amm/thin-liquidity"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 697},
            goal: "amm/closed",
            objectives: ["amm/needs", "amm/utility", "amm/depth", "amm/unmet", "amm/defaults"],
            start: "amm/stress",
            trials: 1
        }
    },
    objectives: #{
        "amm/defaults": #{direction: "minimize", measure: #{State: #{function: "defaulted", script: "amm"}}},
        "amm/depth": #{direction: "maximize", measure: #{State: #{function: "depth", script: "amm"}}},
        "amm/needs": #{direction: "maximize", measure: #{State: #{function: "needs", script: "amm"}}},
        "amm/unmet": #{direction: "minimize", measure: #{State: #{function: "unmet", script: "amm"}}},
        "amm/utility": #{direction: "maximize", measure: #{State: #{function: "utility", script: "amm"}}}
    },
    strategies: #{
        "amm/day/market-day": #{Plan: #{plan: #{function: "market_day_plan", script: "amm"}}},
        "amm/day/no-whale": #{Plan: #{plan: #{function: "no_whale_plan", script: "amm"}}},
        "amm/day/thin-liquidity": #{Plan: #{plan: #{function: "thin_liquidity_plan", script: "amm"}}},
        "amm/market-day": #{Policy: #{policy: #{function: "market_day", script: "amm"}}},
        "amm/no-whale": #{Policy: #{policy: #{function: "no_whale", script: "amm"}}},
        "amm/thin-liquidity": #{Policy: #{policy: #{function: "thin_liquidity", script: "amm"}}}
    },
    views: #{
        amm: #{
            carry: #{function: "carry", script: "amm/view"},
            formats: #{function: "format", script: "amm/view"},
            labels: #{function: "label", script: "amm/view"},
            leaderboards: #{function: "leaderboards", script: "amm/view"},
            scene: #{function: "scene", script: "amm/view"},
            title: "Closed endogenous market"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("world"), basket([amount(asset("market-ended"), 1)]));
    goals["amm/closed"] = #{ontology: "amm", goal: target};
}
{
    let target = goal();
    target = target.require(account("actor/factory"), basket([
    amount(asset("satisfied-need/factory"), 1),
    amount(asset("settled-obligation/factory"), 1)
]));
    target = target.require(account("actor/generator"), basket([amount(asset("settled-obligation/generator"), 1)]));
    target = target.require(account("actor/household"), basket([amount(asset("satisfied-need/household"), 1)]));
    target = target.require(account("actor/speculator"), basket([amount(asset("informed/speculator"), 1)]));
    goals["amm/day/met"] = #{ontology: "amm", goal: target};
}
definitions.goals = goals;
definitions
