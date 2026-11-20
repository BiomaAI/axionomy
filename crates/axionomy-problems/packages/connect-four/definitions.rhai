let definitions = #{
    candidates: #{"connect-four/moves": #{Instances: #{under: ()}}},
    experiments: #{
        "connect-four/micro": #{
            arms: [
                #{start: (), strategy: "connect-four/mcts-64/self-play"},
                #{start: (), strategy: "connect-four/mcts-64/center-opponent"},
                #{start: (), strategy: "connect-four/mcts-64/search-opponent"}
            ],
            effort: #{budget: 1, seed: 17, turns: 43},
            goal: "connect-four/red-wins",
            objectives: ["connect-four/red", "connect-four/yellow"],
            start: "connect-four/micro",
            trials: 1
        },
        "connect-four/showcase": #{
            arms: [
                #{start: (), strategy: "connect-four/mcts-16/self-play"},
                #{start: (), strategy: "connect-four/mcts-16/center-opponent"},
                #{start: (), strategy: "connect-four/mcts-16/search-opponent"}
            ],
            effort: #{budget: 1, seed: 17, turns: 43},
            goal: "connect-four/red-wins",
            objectives: ["connect-four/red", "connect-four/yellow"],
            start: "connect-four/showcase",
            trials: 1
        },
        "connect-four/stress": #{
            arms: [
                #{start: (), strategy: "connect-four/mcts-32/self-play"},
                #{start: (), strategy: "connect-four/mcts-32/center-opponent"},
                #{start: (), strategy: "connect-four/mcts-32/search-opponent"}
            ],
            effort: #{budget: 1, seed: 17, turns: 43},
            goal: "connect-four/red-wins",
            objectives: ["connect-four/red", "connect-four/yellow"],
            start: "connect-four/stress",
            trials: 1
        }
    },
    objectives: #{
        "connect-four/red": #{direction: "maximize", measure: #{State: #{function: "red", script: "connect-four"}}},
        "connect-four/yellow": #{direction: "maximize", measure: #{State: #{function: "yellow", script: "connect-four"}}}
    },
    strategies: #{
        "connect-four/mcts-16/center-opponent": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 16,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{Policy: #{function: "center_opponent", script: "connect-four"}}
                ]
            }
        },
        "connect-four/mcts-16/search-opponent": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 16,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{
                        Minimax: #{
                            candidates: "connect-four/moves",
                            depth: 5,
                            evaluate: #{function: "evaluate", script: "connect-four"}
                        }
                    }
                ]
            }
        },
        "connect-four/mcts-16/self-play": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 16,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 16,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    }
                ]
            }
        },
        "connect-four/mcts-32/center-opponent": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 32,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{Policy: #{function: "center_opponent", script: "connect-four"}}
                ]
            }
        },
        "connect-four/mcts-32/search-opponent": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 32,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{
                        Minimax: #{
                            candidates: "connect-four/moves",
                            depth: 5,
                            evaluate: #{function: "evaluate", script: "connect-four"}
                        }
                    }
                ]
            }
        },
        "connect-four/mcts-32/self-play": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 32,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 32,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    }
                ]
            }
        },
        "connect-four/mcts-64/center-opponent": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 64,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{Policy: #{function: "center_opponent", script: "connect-four"}}
                ]
            }
        },
        "connect-four/mcts-64/search-opponent": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 64,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{
                        Minimax: #{
                            candidates: "connect-four/moves",
                            depth: 5,
                            evaluate: #{function: "evaluate", script: "connect-four"}
                        }
                    }
                ]
            }
        },
        "connect-four/mcts-64/self-play": #{
            Game: #{
                actor: #{function: "mover", script: "connect-four"},
                nature: (),
                players: [
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 64,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    },
                    #{
                        Mcts: #{
                            candidates: "connect-four/moves",
                            chance: (),
                            depth: 2,
                            estimate: #{function: "estimate", script: "connect-four"},
                            iterations: 64,
                            payoffs: #{function: "outcome", script: "connect-four"},
                            rollout: (),
                            salt: 0
                        }
                    }
                ]
            }
        }
    },
    views: #{
        "connect-four": #{
            carry: (),
            formats: (),
            labels: #{function: "label", script: "connect-four/view"},
            leaderboards: (),
            scene: #{function: "scene", script: "connect-four/view"},
            title: "Connect Four"
        }
    }
};
let goals = #{};
{
    let target = goal();
    target = target.require(account("result"), basket([amount(asset("winner/red"), 1)]));
    goals["connect-four/red-wins"] = #{ontology: "connect-four", goal: target};
}
definitions.goals = goals;
definitions
