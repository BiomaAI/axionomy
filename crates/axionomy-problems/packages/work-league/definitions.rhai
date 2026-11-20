let definitions = #{
    experiments: #{
        "work-league/micro": #{
            arms: [
                #{start: (), strategy: "work-league/serial/mixed-field"},
                #{start: "work-league/micro/throughput-field", strategy: "work-league/serial/throughput-field"},
                #{
                    start: "work-league/micro/sustainable-field",
                    strategy: "work-league/serial/sustainable-field"
                }
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "work-league/serial/completed",
            objectives: ["work-league/value", "work-league/completed", "work-league/waste"],
            start: "work-league/micro",
            trials: 1
        },
        "work-league/showcase": #{
            arms: [
                #{start: (), strategy: "work-league/mixed-field"},
                #{start: "work-league/showcase/throughput-field", strategy: "work-league/throughput-field"},
                #{start: "work-league/showcase/sustainable-field", strategy: "work-league/sustainable-field"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 2608},
            goal: "work-league/season-ended",
            objectives: ["work-league/value", "work-league/completed", "work-league/waste"],
            start: "work-league/showcase",
            trials: 1
        },
        "work-league/stress": #{
            arms: [
                #{start: (), strategy: "work-league/mixed-field"},
                #{start: "work-league/stress/throughput-field", strategy: "work-league/throughput-field"},
                #{start: "work-league/stress/sustainable-field", strategy: "work-league/sustainable-field"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 2608},
            goal: "work-league/season-ended",
            objectives: ["work-league/value", "work-league/completed", "work-league/waste"],
            start: "work-league/stress",
            trials: 1
        }
    },
    objectives: #{
        "work-league/completed": #{direction: "maximize", measure: #{State: #{function: "completed", script: "work-league"}}},
        "work-league/value": #{direction: "maximize", measure: #{State: #{function: "contract_value", script: "work-league"}}},
        "work-league/waste": #{direction: "minimize", measure: #{State: #{function: "residual_waste", script: "work-league"}}}
    },
    strategies: #{
        "work-league/mixed-field": #{Policy: #{policy: #{function: "season", script: "work-league"}}},
        "work-league/serial/mixed-field": #{Plan: #{plan: #{function: "serial", script: "work-league"}}},
        "work-league/serial/sustainable-field": #{Plan: #{plan: #{function: "serial", script: "work-league"}}},
        "work-league/serial/throughput-field": #{Plan: #{plan: #{function: "serial", script: "work-league"}}},
        "work-league/sustainable-field": #{Policy: #{policy: #{function: "season", script: "work-league"}}},
        "work-league/throughput-field": #{Policy: #{policy: #{function: "season", script: "work-league"}}}
    },
    views: #{
        "work-league": #{
            carry: (),
            formats: #{function: "format", script: "work-league/view"},
            labels: #{function: "label", script: "work-league/view"},
            leaderboards: #{function: "leaderboards", script: "work-league/view"},
            scene: #{function: "scene", script: "work-league/view"},
            title: "Competitive multi-agent work allocation"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("world"), basket([amount(asset("season-ended"), 1)]));
    goals["work-league/season-ended"] = #{ontology: "work-league", goal: target};
}
{
    let target = goal();
    target = target.require(account("job/1"), basket([amount(asset("completed"), 1)]));
    target = target.require(account("job/2"), basket([amount(asset("completed"), 1)]));
    target = target.require(account("job/3"), basket([amount(asset("completed"), 1)]));
    target = target.require(account("job/4"), basket([amount(asset("completed"), 1)]));
    goals["work-league/serial/completed"] = #{ontology: "work-league", goal: target};
}
definitions.goals = goals;
definitions
