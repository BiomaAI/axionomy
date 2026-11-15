// Perishables: interchangeable claims belong to two cohorts, and time and
// power act on a cohort's one condition fact, so one exchange ages every
// claim of a cohort at once. Micro commits stock to the fridge before the
// ambient cohort expires, then plays the outage that follows: each timed
// condition spoils its cohort once its moment comes, unless the fact it
// spoils is gone. The kernel applies or refuses every exchange.

// The claims one storage commitment moves to the fridge: Micro moves
// ten at a time.
fn transfer() { 10 }

// The cohorts, and each timed condition in the order its moment comes, as
// the law gives it: each spoiling's values, the cohort, its exposure, where
// the cohort is stored and the moment it spoils.
fn cohorts() { ["ambient", "refrigerated"] }
fn timed_conditions(state) {
    let conditions = [];
    for spoiling in state.instances("spoil") { conditions.push(state.values(spoiling)); }
    conditions
}

// The exchanges, each bound to the accounts its roles name. Storage-wide
// exchanges bind the warehouse, the fridge, both cohorts and the world.
fn storage_wide(rate, units) {
    exchange(rate, #{
        "source-storage": "storage/warehouse",
        "destination-storage": "storage/fridge",
        "source-cohort": "cohort/ambient",
        "destination-cohort": "cohort/refrigerated",
        world: "world",
    }, units)
}
fn move_to_fridge(units) { storage_wide("move-to-fridge", units) }
fn seal_storage_plan() { storage_wide("seal-storage-plan", 1) }
fn finish() { storage_wide("finish", 1) }
fn advance(from, to) { exchange(`advance/${from}/${to}`, #{ world: "world" }, 1) }
fn lose_power() {
    exchange("lose-power", #{ storage: "storage/fridge", cohort: "cohort/refrigerated", world: "world" }, 1)
}
fn spoil(condition) {
    exchange(`spoil/${condition.cohort}/${condition.exposure}`, #{
        storage: `storage/${condition.storage}`,
        cohort: `cohort/${condition.cohort}`,
        world: "world",
    }, 1)
}

// The storage plans: commit another transfer to the fridge, advance to the
// ambient cohort's expiry, spoil it once due, or seal the plan; those that
// apply, in that order.
fn storage_plans(state) {
    [
        move_to_fridge(transfer()),
        advance("harvest", "ambient-expiry"),
        spoil(state.values("spoil/ambient/ambient")),
        seal_storage_plan(),
    ].filter(|proposal| state.applicable(proposal))
}

// The outage, turn by turn. A timed condition whose moment has come spoils
// its cohort first, the earliest due first; one whose fact is gone no longer
// applies, and is passed over. Otherwise, at harvest, the transfer once, then
// the ambient cohort's expiry; there, the fridge loses power, and the clock
// runs on to the warmed and the cold cohorts' expiry, where the scenario
// finishes.
fn outage(state, random) {
    for condition in timed_conditions(state) {
        let effect = spoil(condition);
        if state.applicable(effect) { return effect; }
    }
    if state.balance("world", "now/harvest") > 0 {
        let moved = state.balance("storage/fridge", "spent-cooling-energy") > 0;
        if !moved && transfer() > 0 { return move_to_fridge(transfer()); }
        return advance("harvest", "ambient-expiry");
    }
    if state.balance("world", "now/ambient-expiry") > 0 {
        if state.balance("storage/fridge", "powered") > 0 { return lose_power(); }
        return advance("ambient-expiry", "warmed-expiry");
    }
    if state.balance("world", "now/warmed-expiry") > 0 {
        return advance("warmed-expiry", "cold-expiry");
    }
    finish()
}

// The objectives an experiment measures.

// Every claim of a cohort, wherever it is held.
fn cohort_claims(state, cohort) {
    let total = quantity(0);
    for account in state.accounts("") { total = total + state.balance(account, `claim/${cohort}`); }
    total
}

// Every claim, of either cohort.
fn claims(state) {
    let total = quantity(0);
    for cohort in cohorts() { total = total + cohort_claims(state, cohort); }
    total
}

// The claims still usable: those of each cohort whose condition is fresh
// before its moment. Where a storage plan is sealed the ambient cohort has
// spoiled, so these are the refrigerated claims the plan keeps.
fn usable_claims(state) {
    let total = quantity(0);
    for condition in timed_conditions(state) {
        let fresh = `fresh/${condition.cohort}/${condition.exposure}/${condition.due}`;
        if state.balance(`cohort/${condition.cohort}`, fresh) == 1
            && state.balance("world", `before/${condition.due}`) == 1 {
            total = total + cohort_claims(state, condition.cohort);
        }
    }
    total
}

// The cooling energy the fridge has spent.
fn cooling_energy(state) { state.balance("storage/fridge", "spent-cooling-energy") }
