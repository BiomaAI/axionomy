let definitions = #{
    candidates: #{"pantry/deliveries": #{Instances: #{under: "deliver"}}},
    strategies: #{"pantry/shortest": #{BreadthFirst: #{candidates: "pantry/deliveries"}}},
    objectives: #{"pantry/served": #{direction: "maximize", measure: #{State: #{script: "pantry", function: "served"}}}},
    experiments: #{
        "pantry/morning": #{
            start: "pantry/morning",
            goal: "pantry/served",
            arms: [#{strategy: "pantry/shortest", start: ()}],
            objectives: ["pantry/served"],
            effort: #{seed: 17, budget: 32, turns: 3},
            trials: 1
        }
    },
    views: #{pantry: #{title: "Pantry deliveries"}}
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("counter"), basket([amount(asset("meal"), 2)]));
    goals["pantry/served"] = #{ontology: "pantry", goal: target};
}
definitions.goals = goals;
definitions
