let model = #{ontology: "maze", laws: ["maze/showcase-and-stress"]};
let accounts = [];
accounts.push(opening(account("agent"), basket([
    amount(asset("active"), 1),
    amount(asset("at/start"), 1),
    amount(asset("energy"), 18),
    amount(asset("health"), 3),
    amount(asset("time"), 14)
])));
let world = [
    amount(asset("distance/atrium"), 5),
    amount(asset("distance/bridge"), 2),
    amount(asset("distance/canal"), 4),
    amount(asset("distance/detour"), 7),
    amount(asset("distance/gallery"), 3),
    amount(asset("distance/garden"), 8),
    amount(asset("distance/gate"), 2),
    amount(asset("distance/key-room"), 5),
    amount(asset("distance/library"), 4),
    amount(asset("distance/market"), 6),
    amount(asset("distance/start"), 6),
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
    ["atrium", "gallery"],
    ["atrium", "library"],
    ["bridge", "canal"],
    ["bridge", "exit"],
    ["canal", "bridge"],
    ["detour", "exit"],
    ["gallery", "gate"],
    ["gallery", "market"],
    ["garden", "market"],
    ["garden", "workshop"],
    ["gate", "vault"],
    ["key-room", "library"],
    ["library", "gallery"],
    ["library", "key-room"],
    ["market", "canal"],
    ["market", "gallery"],
    ["start", "atrium"],
    ["start", "detour"],
    ["start", "garden"],
    ["start", "tunnel"],
    ["tunnel", "bridge"],
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
        "at/atrium",
        "at/bridge",
        "at/canal",
        "at/detour",
        "at/exit",
        "at/gallery",
        "at/garden",
        "at/gate",
        "at/key-room",
        "at/library",
        "at/market",
        "at/start",
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
