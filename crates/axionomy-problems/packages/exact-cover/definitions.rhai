let definitions = #{
    candidates: #{"exact-cover/selections": #{Instances: #{under: ()}}},
    experiments: #{
        "exact-cover/micro": #{
            arms: [
                #{start: (), strategy: "exact-cover/breadth-first"},
                #{start: (), strategy: "exact-cover/algorithm-x"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "exact-cover/covered",
            objectives: [],
            start: "exact-cover/micro",
            trials: 1
        },
        "exact-cover/micro/unsatisfiable": #{
            arms: [
                #{start: (), strategy: "exact-cover/breadth-first"},
                #{start: (), strategy: "exact-cover/algorithm-x"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "exact-cover/covered",
            objectives: [],
            start: "exact-cover/micro/unsatisfiable",
            trials: 1
        },
        "exact-cover/showcase": #{
            arms: [
                #{start: (), strategy: "exact-cover/breadth-first"},
                #{start: (), strategy: "exact-cover/algorithm-x"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "exact-cover/covered",
            objectives: [],
            start: "exact-cover/showcase",
            trials: 1
        },
        "exact-cover/showcase/unsatisfiable": #{
            arms: [
                #{start: (), strategy: "exact-cover/breadth-first"},
                #{start: (), strategy: "exact-cover/algorithm-x"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "exact-cover/covered",
            objectives: [],
            start: "exact-cover/showcase/unsatisfiable",
            trials: 1
        },
        "exact-cover/stress": #{
            arms: [
                #{start: (), strategy: "exact-cover/breadth-first"},
                #{start: (), strategy: "exact-cover/algorithm-x"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "exact-cover/covered",
            objectives: [],
            start: "exact-cover/stress",
            trials: 1
        },
        "exact-cover/stress/unsatisfiable": #{
            arms: [
                #{start: (), strategy: "exact-cover/breadth-first"},
                #{start: (), strategy: "exact-cover/algorithm-x"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "exact-cover/covered",
            objectives: [],
            start: "exact-cover/stress/unsatisfiable",
            trials: 1
        }
    },
    strategies: #{
        "exact-cover/algorithm-x": #{Plan: #{plan: #{function: "algorithm_x", script: "exact-cover"}}},
        "exact-cover/breadth-first": #{BreadthFirst: #{candidates: "exact-cover/selections"}}
    },
    views: #{
        "exact-cover": #{
            carry: (),
            formats: (),
            labels: #{function: "label", script: "exact-cover/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "exact-cover/view"},
            title: "Exact cover"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("problem"), basket([amount(asset("solved"), 1)]));
    goals["exact-cover/covered"] = #{ontology: "exact-cover", goal: target};
}
definitions.goals = goals;
definitions
