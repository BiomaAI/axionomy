import "connect-four/board" as board;

let model = #{
    ontology: "connect-four",
    laws: ["connect-four/common", "connect-four/showcase-and-stress"],
    tables: #{
        "drop/red/{column}/{row}": #{variables: [#{name: "column", range: ["0", "6"]}, #{name: "row", range: ["0", "5"]}]},
        "drop/yellow/{column}/{row}": #{variables: [#{name: "column", range: ["0", "6"]}, #{name: "row", range: ["0", "5"]}]},
        "win/{player}/{column}/{row}": #{
            variables: [
                #{name: "column", range: ["0", "6"]},
                #{name: "row", range: ["0", "5"]},
                #{name: "player", values: ["red", "yellow"]}
            ]
        }
    }
};
let accounts = [];
for x in 0..=6 {
    for y in 0..=5 {
        accounts.push(opening(board::cell_at(x, y), basket([amount(asset("empty"), 1)])));
    }
}
for x in 0..=6 {
    accounts.push(opening(account(`column/${x}`), basket([])));
}
accounts.push(opening(account("game"), basket([amount(asset("turn/red"), 1)])));
accounts.push(opening(account("result"), basket([])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("cell-occupancy");
    for name in ["empty", "piece/red", "piece/yellow"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("column-heights");
    rule.weight(asset("height"), 1);
    rule.weight(asset("piece/red"), -1);
    rule.weight(asset("piece/yellow"), -1);
    rules.push(rule.finish());
}
{
    let rule = invariant("game-phase");
    for name in ["turn/red", "turn/yellow", "winner/red", "winner/yellow", "draw"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("line-counts");
    for color in ["red", "yellow"] {
        rule.weight(asset(`piece/${color}`), -16);
        for counter in board::line_counters(color, 9, 8) {
            rule.weight(counter, 1);
        }
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
