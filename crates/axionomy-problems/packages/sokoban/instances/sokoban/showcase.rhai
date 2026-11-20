let model = #{
    ontology: "sokoban",
    laws: ["sokoban/showcase", "sokoban/showcase-and-stress"],
    tables: #{
        "latch/{circuit}/{at}": #{
            variables: [#{name: "circuit", values: ["0"]}, #{name: "at", values: ["42"]}],
            relations: [#{rows: [["0", "42"]], variables: ["circuit", "at"]}]
        }
    },
    constants: #{"board-height": "7", "board-width": "9"}
};
let accounts = [];
accounts.push(opening(account("cell/0"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/1"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/2"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/3"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/4"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/5"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/6"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/7"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/8"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/9"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/10"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/11"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/12"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/13"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/14"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/15"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/16"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/17"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/18"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/19"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/20"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/21"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/22"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/23"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/24"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/25"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/26"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/27"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/28"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/29"), basket([amount(asset("crate/0"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/30"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/31"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/32"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/33"), basket([amount(asset("crate/1"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/34"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/35"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/36"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/37"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/38"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("floor"), 1),
    amount(asset("player"), 1)
])));
accounts.push(opening(account("cell/39"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/40"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/41"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/42"), basket([amount(asset("empty"), 1), amount(asset("floor"), 1)])));
accounts.push(opening(account("cell/43"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/44"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/45"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/46"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/47"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/48"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/49"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/50"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/51"), basket([
    amount(asset("dead-square"), 1),
    amount(asset("empty"), 1),
    amount(asset("floor"), 1)
])));
accounts.push(opening(account("cell/52"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/53"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/54"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/55"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/56"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/57"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/58"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/59"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/60"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/61"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("cell/62"), basket([amount(asset("wall"), 1)])));
accounts.push(opening(account("success"), basket([
    amount(asset("active"), 1),
    amount(asset("away-from-exit"), 1),
    amount(asset("latch-ready/0"), 1),
    amount(asset("released/0"), 1)
])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("circuit-pressure/0");
    rule.weight(asset("pressed/0"), 1);
    rule.weight(asset("released/0"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("circuit-latch/0");
    rule.weight(asset("latch-ready/0"), 1);
    rule.weight(asset("latched/0"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("player-exit-position");
    rule.weight(asset("at-exit"), 1);
    rule.weight(asset("away-from-exit"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-crate/0");
    rule.weight(asset("crate/0"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-crate/1");
    rule.weight(asset("crate/1"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-player");
    rule.weight(asset("player"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("one-occupant-per-floor");
    for name in ["player", "empty", "crate/0", "crate/1"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("puzzle-lifecycle");
    rule.weight(asset("active"), 1);
    rule.weight(asset("solved"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
