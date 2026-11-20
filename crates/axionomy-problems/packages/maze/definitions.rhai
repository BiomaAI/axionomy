let definitions = #{
    candidates: #{"maze/moves": #{Instances: #{under: ()}}},
    experiments: #{
        "maze/micro": #{
            arms: [
                #{start: (), strategy: "maze/breadth-first"},
                #{start: (), strategy: "maze/dijkstra"},
                #{start: (), strategy: "maze/a-star"},
                #{start: (), strategy: "maze/pareto-energy"},
                #{start: (), strategy: "maze/pareto-time"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "maze/solved",
            objectives: ["maze/energy", "maze/time"],
            start: "maze/micro",
            trials: 1
        },
        "maze/showcase": #{
            arms: [
                #{start: (), strategy: "maze/breadth-first"},
                #{start: (), strategy: "maze/dijkstra"},
                #{start: (), strategy: "maze/a-star"},
                #{start: (), strategy: "maze/pareto-energy"},
                #{start: (), strategy: "maze/pareto-time"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "maze/solved",
            objectives: ["maze/energy", "maze/time"],
            start: "maze/showcase",
            trials: 1
        },
        "maze/stress": #{
            arms: [
                #{start: (), strategy: "maze/breadth-first"},
                #{start: (), strategy: "maze/dijkstra"},
                #{start: (), strategy: "maze/a-star"},
                #{start: (), strategy: "maze/pareto-energy"},
                #{start: (), strategy: "maze/pareto-time"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "maze/solved",
            objectives: ["maze/energy", "maze/time"],
            start: "maze/stress",
            trials: 1
        }
    },
    objectives: #{
        "maze/energy": #{direction: "minimize", measure: #{State: #{function: "spent_energy", script: "maze"}}},
        "maze/time": #{direction: "minimize", measure: #{State: #{function: "spent_time", script: "maze"}}}
    },
    strategies: #{
        "maze/a-star": #{
            Cheapest: #{
                candidates: "maze/moves",
                cost: #{function: "energy", script: "maze"},
                heuristic: #{function: "heuristic", script: "maze"}
            }
        },
        "maze/breadth-first": #{BreadthFirst: #{candidates: "maze/moves"}},
        "maze/dijkstra": #{Cheapest: #{candidates: "maze/moves", cost: #{function: "energy", script: "maze"}, heuristic: ()}},
        "maze/pareto-energy": #{
            Pareto: #{
                by: ["maze/energy", "maze/time"],
                candidates: "maze/moves",
                objectives: ["maze/energy", "maze/time"]
            }
        },
        "maze/pareto-time": #{
            Pareto: #{
                by: ["maze/time", "maze/energy"],
                candidates: "maze/moves",
                objectives: ["maze/time", "maze/energy"]
            }
        }
    },
    views: #{
        maze: #{
            carry: (),
            formats: #{function: "format", script: "maze/view"},
            labels: #{function: "label", script: "maze/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "maze/view"},
            title: "Key-door maze"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("agent"), basket([amount(asset("solved"), 1)]));
    goals["maze/solved"] = #{ontology: "maze", goal: target};
}
definitions.goals = goals;
definitions
