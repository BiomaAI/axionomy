#{
    format: "axionomy.package/1",
    name: "mission",
    version: #{major: 0, minor: 3, patch: 2},
    branches: #{
        "mission/micro/scenario": #{
            from: "mission/micro",
            exchanges: [#{bindings: #{nature: "nature"}, rate: "instantiate/north/0/safe", units: "1"}]
        }
    },
    script_ontologies: #{
        mission: ["mission"],
        "mission/expedition/view": ["mission/expedition"],
        "mission/expedition": ["mission/expedition"],
        "mission/view": ["mission"]
    }
}
