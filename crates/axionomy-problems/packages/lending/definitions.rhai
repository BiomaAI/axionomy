let definitions = #{
    candidates: #{"lending/moves": #{Script: #{function: "candidates", script: "lending"}}},
    experiments: #{
        "lending/micro": #{
            arms: [
                #{start: (), strategy: "lending/least-interest"},
                #{start: (), strategy: "lending/fewest-steps"},
                #{start: (), strategy: "lending/over-borrowed"},
                #{start: "lending/micro/genesis", strategy: "lending/first-depositor-attack"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 64},
            goal: "lending/micro/repaid",
            objectives: ["lending/interest", "lending/collateral", "lending/cash"],
            start: "lending/micro",
            trials: 1
        },
        "lending/showcase": #{
            arms: [
                #{start: (), strategy: "lending/least-interest"},
                #{start: (), strategy: "lending/fewest-steps"},
                #{start: (), strategy: "lending/over-borrowed"},
                #{start: "lending/showcase/genesis", strategy: "lending/first-depositor-attack"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 64},
            goal: "lending/showcase/repaid",
            objectives: ["lending/interest", "lending/collateral", "lending/cash"],
            start: "lending/showcase",
            trials: 1
        },
        "lending/stress": #{
            arms: [
                #{start: (), strategy: "lending/least-interest"},
                #{start: (), strategy: "lending/fewest-steps"},
                #{start: (), strategy: "lending/over-borrowed"},
                #{start: "lending/stress/genesis", strategy: "lending/first-depositor-attack"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 64},
            goal: "lending/stress/repaid",
            objectives: ["lending/interest", "lending/collateral", "lending/cash"],
            start: "lending/stress",
            trials: 1
        }
    },
    objectives: #{
        "lending/cash": #{direction: "maximize", measure: #{State: #{function: "borrower_cash", script: "lending"}}},
        "lending/collateral": #{direction: "maximize", measure: #{State: #{function: "collateral_kept", script: "lending"}}},
        "lending/interest": #{direction: "minimize", measure: #{Steps: #{function: "interest", script: "lending"}}}
    },
    strategies: #{
        "lending/fewest-steps": #{BreadthFirst: #{candidates: "lending/moves"}},
        "lending/first-depositor-attack": #{Plan: #{plan: #{function: "first_depositor_attack", script: "lending"}}},
        "lending/least-interest": #{
            Cheapest: #{candidates: "lending/moves", cost: #{function: "interest", script: "lending"}, heuristic: ()}
        },
        "lending/over-borrowed": #{Policy: #{policy: #{function: "over_borrowed", script: "lending"}}}
    },
    views: #{
        lending: #{
            carry: (),
            formats: #{function: "format", script: "lending/view"},
            labels: #{function: "label", script: "lending/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "lending/view"},
            title: "Lending market"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("borrower"), basket([amount(asset("bill-paid"), 1), amount(asset("collateral"), 40)]));
    goals["lending/micro/repaid"] = #{ontology: "lending", goal: target};
}
{
    let target = goal();
    target = target.require(account("borrower"), basket([amount(asset("bill-paid"), 1), amount(asset("collateral"), 400)]));
    goals["lending/showcase/repaid"] = #{ontology: "lending", goal: target};
}
{
    let target = goal();
    target = target.require(account("borrower"), basket([amount(asset("bill-paid"), 1), amount(asset("collateral"), 1600)]));
    goals["lending/stress/repaid"] = #{ontology: "lending", goal: target};
}
definitions.goals = goals;
definitions
