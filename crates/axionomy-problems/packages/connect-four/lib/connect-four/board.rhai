// Counter coordinates include the off-board starts of four-cell lines.
fn line_counters(color, last_x, last_y) {
    let counters = [];
    for direction in ["across", "up", "rising", "falling"] {
        for x in 0..=last_x {
            for y in 0..=last_y {
                counters.push(asset(`line/${color}/${direction}/${x}/${y}`));
            }
        }
    }
    counters
}

fn cell_at(x, y) { account(`cell/${x}/${y}`) }
