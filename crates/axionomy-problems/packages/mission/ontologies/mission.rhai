let world = ontology();

for name in ["agent/medic", "agent/scout", "mission", "nature", "success"] {
    world.declare(account(name));
}

for name in [
    "area-safe",
    "at/base",
    "at/north",
    "at/south",
    "awaiting-encounter",
    "awaiting-scan",
    "elapsed-time",
    "energy",
    "hazard-resolved",
    "hazard/injury",
    "hazard/safe",
    "injured",
    "intel/north",
    "intel/south",
    "medical-kit",
    "needs-treatment",
    "planning",
    "rescued",
    "seed/0",
    "seed/1",
    "seed/2",
    "seed/3",
    "sensor",
    "shared-intel/north",
    "shared-intel/south",
    "solved",
    "spent-energy",
    "time-remaining",
    "truth/north",
    "truth/south",
    "unresolved",
    "used-medical-kit",
    "used-sensor"
] {
    world.declare(asset(name));
}

for name in ["goal", "medic", "mission", "nature", "scout"] {
    world.declare(role(name));
}

for name in [
    "begin-scan",
    "encounter/{location}/injury",
    "encounter/{location}/safe",
    "finish",
    "instantiate/{truth}/{seed}/{hazard}",
    "move-direct/{location}",
    "move-together/{location}",
    "rescue/{location}",
    "resolve-scan/{truth}/{seed}/{report}",
    "share/{location}",
    "treat"
] {
    world.rate_pattern(name);
}

world
