#{
    format: "axionomy.package/1",
    name: "perishables",
    version: #{major: 0, minor: 3, patch: 2},
    script_ontologies: #{
        "perishables/view": ["perishables"],
        "perishables/cold-chain/view": ["perishables/cold-chain"],
        "perishables/cold-chain": ["perishables/cold-chain"],
        perishables: ["perishables"]
    }
}
