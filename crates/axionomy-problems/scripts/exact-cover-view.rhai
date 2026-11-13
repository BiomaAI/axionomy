// Exact cover: how people read the incidence problem. Labels name its
// elements, subsets and selections; the scene draws which subset contains
// which element, marking the subsets selected and those an earlier selection
// blocks, and counts what is covered, what is selected, the selections left
// and whether a cover can still be completed.

import "exact-cover/algorithm-x" as algorithm;

// A subset or element named in capitals, as the matrix writes it: `ab` is
// `AB`.
fn capitals(name) {
    name.to_upper()
}

fn label(item) {
    let parts = item.split("/");
    let category = parts[0];
    let name = parts[1];
    if category == "account" {
        return "Exact-cover problem";
    }
    if category == "role" {
        return capitals(name.sub_string(0, 1)) + name.sub_string(1);
    }
    if category == "asset" {
        return switch name {
            "uncovered" => `Element ${capitals(parts[2])} is uncovered`,
            "covered" => `Element ${capitals(parts[2])} is covered`,
            "available" => `Subset ${capitals(parts[2])} is available`,
            "selected" => `Subset ${capitals(parts[2])} selected`,
            "progress" => `${parts[2]} elements covered`,
            "active" => "Cover in progress",
            "solved" => "Exact cover found",
            _ => (),
        };
    }
    switch name {
        "select" => `Select subset ${capitals(parts[2])} after covering ${parts[3]} elements`,
        "finish" => "Accept the exact cover",
        _ => (),
    }
}

fn metric(key, label, value) {
    #{ key: key, label: label, value: value.to_string(), unit: () }
}

fn scene(state, frame) {
    let held = state.balances("problem");
    let sets = algorithm::memberships(state);
    let names = sets.keys();
    names.sort();
    let elements = algorithm::universe(held);
    let rows = names.map(|set| #{ key: `set/${set}`, label: capitals(set) });
    let columns = elements.map(|element| #{ key: `element/${element}`, label: capitals(element) });
    let cells = [];
    for set in names {
        let members = sets[set];
        let selected = `selected/${set}` in held;
        let blocked = !selected && members.some(|member| `covered/${member}` in held);
        for element in elements {
            if !members.contains(element) { continue; }
            cells.push(#{
                row: `set/${set}`,
                column: `element/${element}`,
                label: "×",
                classes: if selected { ["selected"] } else if blocked { ["blocked"] } else { [] },
            });
        }
    }
    let scene = scene::matrix("Which subset contains which element", rows, columns, cells);

    let choices = 0;
    for rate in state.rates("select/") {
        if state.applicable(exchange(rate, #{ problem: "problem" }, 1)) { choices += 1; }
    }
    let remaining = elements.filter(|element| `uncovered/${element}` in held);
    let available = names.filter(|set| `available/${set}` in held);
    let completion = "solved" in held || algorithm::cover(sets, remaining, available) != ();
    scene.metrics = [
        metric("covered", "Elements covered", elements.filter(|element| `covered/${element}` in held).len()),
        metric("selected", "Subsets selected", names.filter(|set| `selected/${set}` in held).len()),
        metric("choices", "Legal next selections", choices),
        metric("completion", "Completion possible", if completion { "yes" } else { "no" }),
    ];
    scene
}
