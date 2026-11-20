let world = ontology();

for name in ["agent", "nature"] {
    world.declare(account(name));
}

for name in [
    "at/base",
    "at/north",
    "at/south",
    "awaiting-observation",
    "belief/east",
    "belief/north",
    "belief/south",
    "committed",
    "energy",
    "evacuated",
    "planning",
    "rescued",
    "seed/0",
    "seed/1",
    "seed/2",
    "seed/3",
    "sensor",
    "solved",
    "spent-energy",
    "truth/north",
    "truth/south",
    "unresolved",
    "used-sensor"
] {
    world.declare(asset(name));
}

for name in ["actor", "nature"] {
    world.declare(role(name));
}

for name in [
    "begin-observe",
    "finish",
    "instantiate/{truth}/{seed}",
    "move/base/{to}",
    "rescue/{location}",
    "resolve-observation/{truth}/{seed}/{report}"
] {
    world.rate_pattern(name);
}

world
