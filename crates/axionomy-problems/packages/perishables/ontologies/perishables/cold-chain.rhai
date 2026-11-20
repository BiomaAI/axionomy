let world = ontology();

for name in ["batch/0", "batch/1", "batch/2", "batch/3", "batch/4", "batch/5", "buyer", "world"] {
    world.declare(account(name));
}

for name in [
    "active",
    "aging",
    "at/fridge",
    "at/warehouse",
    "cold-occupancy",
    "cold-space",
    "cooling-off",
    "cooling-on",
    "day",
    "decisions",
    "delivered",
    "demand",
    "disposed",
    "dormant",
    "energy",
    "freight",
    "fresh",
    "fuel",
    "future-demand",
    "future-freight",
    "future-labor",
    "generator-fuel",
    "grid-potential",
    "heat",
    "labor",
    "missed-demand",
    "money",
    "origin/0",
    "origin/1",
    "pending-age/0",
    "pending-age/1",
    "pending-age/2",
    "pending-age/3",
    "pending-age/4",
    "pending-age/5",
    "preserved",
    "satisfied-demand",
    "solved",
    "spent-energy",
    "spent-labor",
    "spilled-energy",
    "spoiled",
    "transport-fuel",
    "unused-freight",
    "unused-labor",
    "used-freight"
] {
    world.declare(asset(name));
}

for name in ["batch", "buyer", "destination", "source", "world"] {
    world.declare(role(name));
}

for name in [
    "age-cold/{batch}",
    "age-empty/{batch}",
    "age-warm/{batch}/{at}/false",
    "age-warm/{batch}/{at}/true",
    "close",
    "cool",
    "deliver/{batch}/{preserved}/{at}",
    "discard/{batch}/{at}",
    "end-day",
    "generate",
    "move/{batch}/{from}/{to}",
    "open-day/{day}",
    "process/{batch}",
    "split/{source}/{destination}/{at}"
] {
    world.rate_pattern(name);
}

world
