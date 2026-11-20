let world = ontology();

for name in ["counter", "stockroom"] {
    world.declare(account(name));
}

for name in ["meal"] {
    world.declare(asset(name));
}

for name in ["recipient", "supplier"] {
    world.declare(role(name));
}

for name in ["deliver"] {
    world.rate_pattern(name);
}

world
