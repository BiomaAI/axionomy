let definitions = #{
    candidates: #{"workshop/recipes": #{Instances: #{under: ()}}},
    experiments: #{
        "workshop/micro": #{
            arms: [
                #{start: (), strategy: "workshop/breadth-first"},
                #{start: (), strategy: "workshop/minimum-waste"},
                #{start: (), strategy: "workshop/pareto-waste"},
                #{start: (), strategy: "workshop/pareto-time"},
                #{start: (), strategy: "workshop/pareto-revenue"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "workshop/delivered",
            objectives: ["workshop/waste", "workshop/time", "workshop/revenue"],
            start: "workshop/micro",
            trials: 1
        },
        "workshop/showcase": #{
            arms: [
                #{start: (), strategy: "workshop/breadth-first"},
                #{start: (), strategy: "workshop/minimum-waste"},
                #{start: (), strategy: "workshop/pareto-waste"},
                #{start: (), strategy: "workshop/pareto-time"},
                #{start: (), strategy: "workshop/pareto-revenue"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "workshop/delivered",
            objectives: ["workshop/waste", "workshop/time", "workshop/revenue"],
            start: "workshop/showcase",
            trials: 1
        },
        "workshop/stress": #{
            arms: [
                #{start: (), strategy: "workshop/breadth-first"},
                #{start: (), strategy: "workshop/minimum-waste"},
                #{start: (), strategy: "workshop/pareto-waste"},
                #{start: (), strategy: "workshop/pareto-time"},
                #{start: (), strategy: "workshop/pareto-revenue"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "workshop/delivered",
            objectives: ["workshop/waste", "workshop/time", "workshop/revenue"],
            start: "workshop/stress",
            trials: 1
        }
    },
    objectives: #{
        "workshop/revenue": #{direction: "maximize", measure: #{State: #{function: "revenue", script: "workshop"}}},
        "workshop/time": #{direction: "minimize", measure: #{State: #{function: "spent_time", script: "workshop"}}},
        "workshop/waste": #{direction: "minimize", measure: #{State: #{function: "waste", script: "workshop"}}}
    },
    strategies: #{
        "workshop/breadth-first": #{BreadthFirst: #{candidates: "workshop/recipes"}},
        "workshop/minimum-waste": #{
            BestFirst: #{candidates: "workshop/recipes", cost: #{function: "waste", script: "workshop"}, heuristic: ()}
        },
        "workshop/pareto-revenue": #{
            Pareto: #{
                by: ["workshop/revenue"],
                candidates: "workshop/recipes",
                objectives: ["workshop/waste", "workshop/time", "workshop/revenue"]
            }
        },
        "workshop/pareto-time": #{
            Pareto: #{
                by: ["workshop/time", "workshop/waste"],
                candidates: "workshop/recipes",
                objectives: ["workshop/waste", "workshop/time", "workshop/revenue"]
            }
        },
        "workshop/pareto-waste": #{
            Pareto: #{
                by: ["workshop/waste", "workshop/time"],
                candidates: "workshop/recipes",
                objectives: ["workshop/waste", "workshop/time", "workshop/revenue"]
            }
        }
    },
    views: #{
        workshop: #{
            carry: (),
            formats: #{function: "format", script: "workshop/view"},
            labels: #{function: "label", script: "workshop/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "workshop/view"},
            title: "Stoichiometric workshop"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("workshop"), basket([amount(asset("solved"), 1)]));
    goals["workshop/delivered"] = #{ontology: "workshop", goal: target};
}
definitions.goals = goals;
definitions
