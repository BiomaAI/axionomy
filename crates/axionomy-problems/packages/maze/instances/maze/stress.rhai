let model = #{ontology: "maze", laws: ["maze/showcase-and-stress", "maze/stress"]};
let accounts = [];
accounts.push(opening(account("agent"), basket([
    amount(asset("active"), 1),
    amount(asset("at/start"), 1),
    amount(asset("energy"), 24),
    amount(asset("health"), 3),
    amount(asset("time"), 18)
])));
let world = [
    amount(asset("distance/archive"), 5),
    amount(asset("distance/atrium"), 5),
    amount(asset("distance/bridge"), 2),
    amount(asset("distance/canal"), 4),
    amount(asset("distance/chapel"), 4),
    amount(asset("distance/detour"), 7),
    amount(asset("distance/docks"), 8),
    amount(asset("distance/foundry"), 6),
    amount(asset("distance/gallery"), 3),
    amount(asset("distance/garden"), 8),
    amount(asset("distance/gate"), 2),
    amount(asset("distance/key-room"), 5),
    amount(asset("distance/library"), 4),
    amount(asset("distance/market"), 6),
    amount(asset("distance/observatory"), 3),
    amount(asset("distance/ridge"), 8),
    amount(asset("distance/ruins"), 6),
    amount(asset("distance/scriptorium"), 6),
    amount(asset("distance/start"), 6),
    amount(asset("distance/tower"), 5),
    amount(asset("distance/tunnel"), 6),
    amount(asset("distance/vault"), 1),
    amount(asset("distance/workshop"), 6),
    amount(asset("key"), 1),
    amount(asset("locked"), 1),
    amount(asset("medicine"), 2),
    amount(asset("power-transit"), 1),
    amount(asset("shield"), 2),
    amount(asset("target/exit"), 1)
];
for edge in [
    ["archive", "gallery"],
    ["archive", "scriptorium"],
    ["atrium", "archive"],
    ["atrium", "gallery"],
    ["atrium", "library"],
    ["bridge", "canal"],
    ["bridge", "exit"],
    ["canal", "bridge"],
    ["chapel", "bridge"],
    ["detour", "exit"],
    ["docks", "foundry"],
    ["foundry", "canal"],
    ["foundry", "tower"],
    ["gallery", "gate"],
    ["gallery", "market"],
    ["garden", "market"],
    ["garden", "workshop"],
    ["gate", "vault"],
    ["key-room", "library"],
    ["key-room", "scriptorium"],
    ["library", "gallery"],
    ["library", "key-room"],
    ["market", "canal"],
    ["market", "docks"],
    ["market", "gallery"],
    ["observatory", "exit"],
    ["ridge", "ruins"],
    ["ruins", "chapel"],
    ["ruins", "garden"],
    ["scriptorium", "archive"],
    ["scriptorium", "key-room"],
    ["start", "atrium"],
    ["start", "detour"],
    ["start", "garden"],
    ["start", "tunnel"],
    ["tower", "observatory"],
    ["tunnel", "bridge"],
    ["tunnel", "ridge"],
    ["tunnel", "workshop"],
    ["vault", "exit"],
    ["workshop", "canal"],
    ["workshop", "garden"]
] {
    world.push(amount(asset(`edge/${edge[0]}/${edge[1]}`), 1));
}
accounts.push(opening(account("world"), basket(world)));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("one-agent-position");
    for name in [
        "at/archive",
        "at/atrium",
        "at/bridge",
        "at/canal",
        "at/chapel",
        "at/detour",
        "at/docks",
        "at/exit",
        "at/foundry",
        "at/gallery",
        "at/garden",
        "at/gate",
        "at/key-room",
        "at/library",
        "at/market",
        "at/observatory",
        "at/ridge",
        "at/ruins",
        "at/scriptorium",
        "at/start",
        "at/tower",
        "at/tunnel",
        "at/vault",
        "at/workshop"
    ] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("energy-is-only-spent");
    rule.weight(asset("energy"), 1);
    rule.weight(asset("spent-energy"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("time-is-only-spent");
    rule.weight(asset("time"), 1);
    rule.weight(asset("spent-time"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("maze-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-shared-power-circuit");
    rule.weight(asset("power-transit"), 1);
    rule.weight(asset("power-vault"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("health-and-injury");
    rule.weight(asset("health"), 1);
    rule.weight(asset("injury"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-protective-equipment");
    rule.weight(asset("shield"), 1);
    rule.weight(asset("used-shield"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("finite-medicine");
    rule.weight(asset("medicine"), 1);
    rule.weight(asset("used-medicine"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
