// Workshop: what a plan of recipes has scrapped, taken and earned, as the
// objectives measure it.

fn waste(state) { state.balance("workshop", "waste") }
fn spent_time(state) { state.balance("workshop", "spent-time") }
fn revenue(state) { state.balance("workshop", "credit") }
