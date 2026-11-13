// Workshop: which recipes to run to make what the orders ask for. Every rate
// is bound to the shop, a delivery also to its order's customer; only those
// that apply are tried, in order.

fn candidates(state) {
    let rates = state.rates("");
    rates
        .map(|rate| {
            let bindings = #{ shop: "workshop" };
            if rate.starts_with("deliver/") {
                bindings.customer = `customer/${rate.sub_string(8)}`;
            }
            exchange(rate, bindings, 1)
        })
        .filter(|proposal| state.applicable(proposal))
}

// What a plan has scrapped, taken and earned, as the objectives measure it.
fn waste(state) { state.balance("workshop", "waste") }
fn spent_time(state) { state.balance("workshop", "spent-time") }
fn revenue(state) { state.balance("workshop", "credit") }
