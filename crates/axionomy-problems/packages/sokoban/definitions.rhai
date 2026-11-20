let definitions = #{
    candidates: #{"sokoban/moves": #{Instances: #{under: ()}}},
    experiments: #{
        "sokoban/micro": #{
            arms: [#{start: (), strategy: "sokoban/a-star"}, #{start: (), strategy: "sokoban/breadth-first"}],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "sokoban/solved",
            objectives: [],
            start: "sokoban/micro",
            trials: 1
        },
        "sokoban/showcase": #{
            arms: [#{start: (), strategy: "sokoban/a-star"}, #{start: (), strategy: "sokoban/breadth-first"}],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "sokoban/solved",
            objectives: [],
            start: "sokoban/showcase",
            trials: 1
        },
        "sokoban/stress": #{
            arms: [#{start: (), strategy: "sokoban/a-star"}, #{start: (), strategy: "sokoban/breadth-first"}],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "sokoban/solved",
            objectives: [],
            start: "sokoban/stress",
            trials: 1
        }
    },
    strategies: #{
        "sokoban/a-star": #{
            Cheapest: #{
                candidates: "sokoban/moves",
                cost: #{function: "step", script: "sokoban"},
                heuristic: #{function: "crate_goal_distance", script: "sokoban"}
            }
        },
        "sokoban/breadth-first": #{BreadthFirst: #{candidates: "sokoban/moves"}}
    },
    views: #{
        sokoban: #{
            carry: (),
            formats: (),
            labels: #{function: "label", script: "sokoban/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "sokoban/view"},
            title: "Sokoban"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("success"), basket([amount(asset("solved"), 1)]));
    goals["sokoban/solved"] = #{ontology: "sokoban", goal: target};
}
definitions.goals = goals;
definitions
