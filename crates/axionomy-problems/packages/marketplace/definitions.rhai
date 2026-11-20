let definitions = #{
    candidates: #{
        "marketplace/fulfillment": #{Script: #{function: "fulfillment", script: "marketplace"}},
        "marketplace/settlements": #{Script: #{function: "settlements", script: "marketplace"}}
    },
    experiments: #{
        "marketplace/micro": #{
            arms: [
                #{start: (), strategy: "marketplace/settlement/market-clearing"},
                #{start: (), strategy: "marketplace/settlement/pareto-buyers"},
                #{start: (), strategy: "marketplace/settlement/pareto-sellers"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "marketplace/micro/settled",
            objectives: [
                "marketplace/buyer-a",
                "marketplace/buyer-b",
                "marketplace/seller-a",
                "marketplace/seller-b",
                "marketplace/buyers",
                "marketplace/sellers"
            ],
            start: "marketplace/micro",
            trials: 1
        },
        "marketplace/showcase": #{
            arms: [
                #{start: (), strategy: "marketplace/market-clearing"},
                #{start: (), strategy: "marketplace/pareto-buyers"},
                #{start: (), strategy: "marketplace/pareto-sellers"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "marketplace/showcase/settled",
            objectives: [
                "marketplace/buyer-a",
                "marketplace/buyer-b",
                "marketplace/seller-a",
                "marketplace/seller-b",
                "marketplace/buyer-c",
                "marketplace/seller-c",
                "marketplace/ontime",
                "marketplace/elapsed"
            ],
            start: "marketplace/showcase",
            trials: 1
        },
        "marketplace/stress": #{
            arms: [
                #{start: (), strategy: "marketplace/market-clearing"},
                #{start: (), strategy: "marketplace/pareto-buyers"},
                #{start: (), strategy: "marketplace/pareto-sellers"}
            ],
            effort: #{budget: 10000000, seed: 17, turns: 0},
            goal: "marketplace/stress/settled",
            objectives: [
                "marketplace/buyer-a",
                "marketplace/buyer-b",
                "marketplace/seller-a",
                "marketplace/seller-b",
                "marketplace/buyer-c",
                "marketplace/seller-c",
                "marketplace/ontime",
                "marketplace/elapsed"
            ],
            start: "marketplace/stress",
            trials: 1
        }
    },
    objectives: #{
        "marketplace/buyer-a": #{direction: "maximize", measure: #{State: #{function: "buyer_a", script: "marketplace"}}},
        "marketplace/buyer-b": #{direction: "maximize", measure: #{State: #{function: "buyer_b", script: "marketplace"}}},
        "marketplace/buyer-c": #{direction: "maximize", measure: #{State: #{function: "buyer_c", script: "marketplace"}}},
        "marketplace/buyers": #{direction: "maximize", measure: #{State: #{function: "front_buyers", script: "marketplace"}}},
        "marketplace/elapsed": #{direction: "minimize", measure: #{State: #{function: "elapsed", script: "marketplace"}}},
        "marketplace/ontime": #{direction: "maximize", measure: #{State: #{function: "on_time", script: "marketplace"}}},
        "marketplace/seller-a": #{direction: "maximize", measure: #{State: #{function: "seller_a", script: "marketplace"}}},
        "marketplace/seller-b": #{direction: "maximize", measure: #{State: #{function: "seller_b", script: "marketplace"}}},
        "marketplace/seller-c": #{direction: "maximize", measure: #{State: #{function: "seller_c", script: "marketplace"}}},
        "marketplace/sellers": #{direction: "maximize", measure: #{State: #{function: "front_sellers", script: "marketplace"}}}
    },
    strategies: #{
        "marketplace/market-clearing": #{
            Beam: #{
                candidates: "marketplace/fulfillment",
                depth: 39,
                score: #{function: "volume", script: "marketplace"},
                tie: "last",
                width: 24
            }
        },
        "marketplace/pareto-buyers": #{
            Beam: #{
                candidates: "marketplace/fulfillment",
                depth: 39,
                score: #{function: "favor_buyers", script: "marketplace"},
                tie: "last",
                width: 24
            }
        },
        "marketplace/pareto-sellers": #{
            Beam: #{
                candidates: "marketplace/fulfillment",
                depth: 39,
                score: #{function: "favor_sellers", script: "marketplace"},
                tie: "last",
                width: 24
            }
        },
        "marketplace/settlement/market-clearing": #{
            BranchAndBound: #{
                bound: #{function: "forgone_settled", script: "marketplace"},
                candidates: "marketplace/settlements",
                cost: #{function: "forgone", script: "marketplace"}
            }
        },
        "marketplace/settlement/pareto-buyers": #{
            Pareto: #{
                by: ["marketplace/buyers", "marketplace/buyer-b", "marketplace/seller-b"],
                candidates: "marketplace/settlements",
                objectives: [
                    "marketplace/buyer-a",
                    "marketplace/buyer-b",
                    "marketplace/seller-a",
                    "marketplace/seller-b",
                    "marketplace/buyers",
                    "marketplace/sellers"
                ]
            }
        },
        "marketplace/settlement/pareto-sellers": #{
            Pareto: #{
                by: ["marketplace/sellers", "marketplace/seller-b", "marketplace/buyer-b"],
                candidates: "marketplace/settlements",
                objectives: [
                    "marketplace/buyer-a",
                    "marketplace/buyer-b",
                    "marketplace/seller-a",
                    "marketplace/seller-b",
                    "marketplace/buyers",
                    "marketplace/sellers"
                ]
            }
        }
    },
    views: #{
        marketplace: #{
            carry: (),
            formats: #{function: "format", script: "marketplace/view"},
            labels: #{function: "label", script: "marketplace/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "marketplace/view"},
            title: "Multi-party marketplace"
        }
    }
};
fn settled(orders) {
    let target = goal();
    for id in orders {
        let received = basket([amount(asset(`settled-order/${id}`), 1)]);
        target = target.require(account(`order/${id}`), received);
    }
    #{ontology: "marketplace", goal: target}
}
definitions.goals = #{
    "marketplace/micro/settled": settled(["a", "b"]),
    "marketplace/showcase/settled": settled(["a", "b", "c", "d"]),
    "marketplace/stress/settled": settled(["a", "b", "c", "d", "e", "f"])
};
definitions
