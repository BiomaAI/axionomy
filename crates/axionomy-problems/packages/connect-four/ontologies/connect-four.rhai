import "connect-four/board" as board;

let world = ontology();
for x in 0..=6 {
    world.declare(account(`column/${x}`));
    for y in 0..=5 { world.declare(board::cell_at(x, y)); }
}
for name in ["game", "result"] { world.declare(account(name)); }
for name in ["draw", "empty", "height", "piece/red", "piece/yellow", "turn/red", "turn/yellow", "winner/red", "winner/yellow"] {
    world.declare(asset(name));
}
for color in ["red", "yellow"] {
    for counter in board::line_counters(color, 9, 8) { world.declare(counter); }
}
for name in ["cell", "column", "full/0", "full/1", "full/2", "full/3", "full/4", "full/5", "full/6", "game", "result"] { world.declare(role(name)); }
for name in ["draw/{player}", "drop/red/{column}/{row}", "drop/yellow/{column}/{row}", "win/{player}/{column}/{row}"] { world.rate_pattern(name); }
world
