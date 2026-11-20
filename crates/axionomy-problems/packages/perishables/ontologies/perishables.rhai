let world = ontology();

for name in ["cohort/ambient", "cohort/refrigerated", "storage/fridge", "storage/warehouse", "world"] {
    world.declare(account(name));
}

for name in [
    "active",
    "ambient",
    "before/ambient-expiry",
    "before/cold-expiry",
    "before/warmed-expiry",
    "claim/ambient",
    "claim/refrigerated",
    "cohort-identity/ambient",
    "cohort-identity/refrigerated",
    "cold",
    "consumed/ambient",
    "consumed/refrigerated",
    "cooling-energy",
    "fresh/ambient/ambient/ambient-expiry",
    "fresh/refrigerated/cold/cold-expiry",
    "fresh/refrigerated/warmed-after-outage/warmed-expiry",
    "location-identity/fridge",
    "location-identity/warehouse",
    "now/ambient-expiry",
    "now/cold-expiry",
    "now/harvest",
    "now/warmed-expiry",
    "plan-solved",
    "planning",
    "powered",
    "reached/ambient-expiry",
    "reached/cold-expiry",
    "reached/harvest",
    "reached/warmed-expiry",
    "rotten/ambient",
    "rotten/refrigerated",
    "solved",
    "spent-cooling-energy",
    "unpowered",
    "world-identity"
] {
    world.declare(asset(name));
}

for name in [
    "cohort",
    "destination-cohort",
    "destination-storage",
    "holder",
    "source-cohort",
    "source-storage",
    "storage",
    "world"
] {
    world.declare(role(name));
}

for name in [
    "advance/{from}/{to}",
    "eat/{cohort}/{exposure}",
    "finish",
    "lose-power",
    "move-to-fridge",
    "seal-storage-plan",
    "spoil/{cohort}/{exposure}"
] {
    world.rate_pattern(name);
}

world
