let world = ontology();

for name in ["agent/a", "agent/b", "bridge"] {
    world.declare(account(name));
}

for name in [
    "active",
    "arrival",
    "at/east",
    "at/west",
    "bid/1",
    "bid/2",
    "can-bid",
    "capacity-free",
    "completed-trip",
    "credit",
    "crossed",
    "crossing-right",
    "deadline",
    "detouring",
    "energy",
    "escrow",
    "first-turn",
    "in-transit",
    "late",
    "lease-until",
    "missed-reservation",
    "now",
    "on-detour",
    "on-time",
    "priority-benefit",
    "ready-at",
    "round",
    "rounds-remaining",
    "second-turn",
    "solved",
    "spent-credit",
    "spent-energy",
    "submitted",
    "time-remaining",
    "waiting"
] {
    world.declare(asset(name));
}

for name in ["agent-a", "agent-b", "bridge", "loser", "traveler", "winner"] {
    world.declare(role(name));
}

for name in [
    "advance/{to}",
    "claim-first/{agent}",
    "claim-second/{agent}",
    "complete/{agent}/false/false",
    "complete/{agent}/false/true",
    "complete/{agent}/true/false",
    "complete/{agent}/true/true",
    "cross/{agent}",
    "detour/{agent}/false",
    "detour/{agent}/true",
    "expire-reservation/{agent}",
    "finish",
    "reset-round",
    "resolve/a/{winning-bid}/{losing-bid}",
    "resolve/b/{winning-bid}/{losing-bid}",
    "submit-bid/{agent}/{amount}",
    "withdraw-bid/{agent}/{amount}",
    "yield-to-waiting/{agent}"
] {
    world.rate_pattern(name);
}

world
