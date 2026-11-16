// The cold chain: crates in batches, finite cold storage, heat each batch
// carries, and one fuel reserve shared by backup generation and transport.
// Each day a policy cools, processes, ships, discards and makes room in
// cold storage in its own order, then the day ends, every batch ages and
// the next day opens. What the economy holds does not say which of a day's
// decisions comes next, so each policy is a plan for every day of the
// outage schedule, made whole on a shadow of the balances it reads, each
// decision taken only where the kernel would accept it. The kernel applies
// the plan or refuses it.

// The batches, in the order the policies visit them.
fn batch_ids() { [0, 1, 2, 3, 4, 5] }

// The electricity the grid supplies as day `day` opens, as the law gives
// it: none on a day of the outage.
fn grid_supply(state, day) { state.values(`open-day/${day}`)["grid-supply"] }

// What the laws take and give that a plan follows, read from the economy:
// the most electricity the battery stores, cooling's electricity, backup
// generation's fuel and what it makes, what processing and delivering take
// a crate, a trip's fuel, a move's labor, the cold slots a crate takes in
// each place, the heat a warm day adds, and what each day opens with.
fn law(state) {
    let delivery = state.exchange_of("deliver/0/false/warehouse");
    #{
        battery: state.constant("battery"),
        cooling: state.consumed("cool", "world").energy,
        generator_fuel: state.consumed("generate", "world").fuel,
        generated: state.produced("generate", "world").energy,
        processing: state.consumed("process/0", "world"),
        delivery: state.consumed(delivery.rate(), "world"),
        trip_fuel: state.assess(delivery).accounts.world.consumed.fuel,
        move_labor: state.consumed("move/0/warehouse/fridge", "world").labor,
        slots: #{
            warehouse: state.values("discard/0/warehouse").refrigerated,
            fridge: state.values("discard/0/fridge").refrigerated,
        },
        warmth: state.produced("age-warm/0/warehouse/false", "batch").heat,
        day: state.produced("open-day/1", "world"),
    }
}

// How many of what costs `each` apiece `stock` pays for.
fn afforded(stock, each) { mul_div(stock, quantity(1), each, "floor") }

// What a policy reads of a batch: where it is ("" when dormant), its fresh,
// preserved and spoiled crates, its heat, whether it is dormant, and the
// cold space it occupies.
fn batch_shadow(state, id) {
    let account = `batch/${id}`;
    let at = if state.balance(account, "at/warehouse") > 0 {
        "warehouse"
    } else if state.balance(account, "at/fridge") > 0 {
        "fridge"
    } else {
        ""
    };
    #{
        at: at,
        fresh: state.balance(account, "fresh"),
        preserved: state.balance(account, "preserved"),
        spoiled: state.balance(account, "spoiled"),
        heat: state.balance(account, "heat"),
        dormant: state.balance(account, "dormant") > 0,
        occupancy: state.balance(account, "cold-occupancy"),
    }
}

// The shadow a plan follows: the state it starts from and its law, the
// world's stores, the buyer's money, every batch, and the exchanges
// planned so far.
fn shadow(state) {
    let batches = [];
    for id in batch_ids() { batches.push(batch_shadow(state, id)); }
    #{
        state: state,
        law: law(state),
        energy: state.balance("world", "energy"),
        fuel: state.balance("world", "fuel"),
        labor: state.balance("world", "labor"),
        freight: state.balance("world", "freight"),
        demand: state.balance("world", "demand"),
        cold_space: state.balance("world", "cold-space"),
        cooling: state.balance("world", "cooling-on") > 0,
        money: state.balance("buyer", "money"),
        batches: batches,
        plan: [],
    }
}

// The exchange of `units` of `rate`, each of its roles bound to the account
// the rate fixes, planned: always true.
fn planned(rate, units) {
    let fixed = this.state.exchange_of(rate);
    let bindings = #{};
    for role in this.state.roles(rate) { bindings[role] = fixed.account(role); }
    this.plan.push(exchange(rate, bindings, units));
    true
}

// Each decision on the shadow: true, the exchange planned and its effects
// followed, where the kernel accepts it; false, and nothing planned, where
// it does not, as when its units are none.

// Backup generation: its fuel for what it makes, while the battery has
// room for that.
fn generated() {
    if this.energy + this.law.generated > this.law.battery
        || this.fuel < this.law.generator_fuel {
        return false;
    }
    this.fuel = this.fuel - this.law.generator_fuel;
    this.energy = this.energy + this.law.generated;
    this.planned("generate", 1)
}

// Cooling for the day, for its electricity.
fn chilled() {
    if this.cooling || this.energy < this.law.cooling { return false; }
    this.cooling = true;
    this.energy = this.energy - this.law.cooling;
    this.planned("cool", 1)
}

// Processing `amount` fresh crates of a batch in the warehouse, for the
// electricity and labor each takes.
fn processed(id, amount) {
    let batch = this.batches[id];
    if amount == 0 || batch.at != "warehouse" || batch.fresh < amount { return false; }
    let energy = amount * this.law.processing.energy;
    let labor = amount * this.law.processing.labor;
    if this.energy < energy || this.labor < labor { return false; }
    this.batches[id].fresh = batch.fresh - amount;
    this.batches[id].preserved = batch.preserved + amount;
    this.energy = this.energy - energy;
    this.labor = this.labor - labor;
    this.planned(`process/${id}`, amount)
}

// Delivering `amount` of a batch's fresh or preserved crates where it is:
// the freight and demand each takes, the buyer paying the price the law
// gives a crate of the kind, a trip's fuel, and the crates' cold slots
// freed.
fn delivered(id, preserved, at, amount) {
    let batch = this.batches[id];
    let stock = if preserved { batch.preserved } else { batch.fresh };
    if amount == 0 || batch.at != at || stock < amount || this.fuel < this.law.trip_fuel {
        return false;
    }
    let freight = amount * this.law.delivery.freight;
    let demand = amount * this.law.delivery.demand;
    let freed = amount * this.law.slots[at];
    if this.freight < freight || this.demand < demand || batch.occupancy < freed { return false; }
    let price = this.state.values(`deliver/${id}/${preserved}/${at}`).price;
    if this.money < amount * price { return false; }
    if preserved {
        this.batches[id].preserved = stock - amount;
    } else {
        this.batches[id].fresh = stock - amount;
    }
    this.batches[id].occupancy = batch.occupancy - freed;
    this.cold_space = this.cold_space + freed;
    this.money = this.money - amount * price;
    this.freight = this.freight - freight;
    this.demand = this.demand - demand;
    this.fuel = this.fuel - this.law.trip_fuel;
    this.planned(`deliver/${id}/${preserved}/${at}`, amount)
}

// Discarding `amount` of a batch's spoiled crates, freeing their cold slots.
fn discarded(id, at, amount) {
    if amount == 0 { return false; }
    let batch = this.batches[id];
    let freed = amount * this.law.slots[at];
    if batch.spoiled < amount || batch.occupancy < freed { return false; }
    this.batches[id].spoiled = batch.spoiled - amount;
    this.batches[id].occupancy = batch.occupancy - freed;
    this.cold_space = this.cold_space + freed;
    this.planned(`discard/${id}/${at}`, amount)
}

// Moving a whole batch from the warehouse into cold storage, for a move's
// labor, while there is room for every crate it holds.
fn moved_cold(id) {
    let batch = this.batches[id];
    let amount = batch.fresh + batch.preserved + batch.spoiled;
    let taken = amount * this.law.slots.fridge;
    if batch.at != "warehouse" || amount < 1 || this.labor < this.law.move_labor
        || this.cold_space < taken {
        return false;
    }
    this.batches[id].at = "fridge";
    this.batches[id].occupancy = batch.occupancy + taken;
    this.cold_space = this.cold_space - taken;
    this.labor = this.labor - this.law.move_labor;
    this.planned(`move/${id}/warehouse/fridge`, 1)
}

// Splitting `units` fresh crates of a batch in the warehouse into a dormant
// batch, which inherits its heat, leaving at least one behind.
fn split_into(source, destination, units) {
    let parent = this.batches[source];
    let child = this.batches[destination];
    if units == 0 || parent.at != "warehouse" || !child.dormant || parent.fresh < units + 1 {
        return false;
    }
    this.batches[source].fresh = parent.fresh - units;
    this.batches[destination].dormant = false;
    this.batches[destination].at = "warehouse";
    this.batches[destination].fresh = child.fresh + units;
    this.batches[destination].heat = child.heat + parent.heat;
    this.planned(`split/${source}/${destination}/warehouse`, units)
}

// The policy's steps, each over every batch in order.

// Cooling, when cold storage holds fresh crates: with `backup`, generating
// first when the battery cannot pay for it and the fuel left pays for
// generating and a trip after.
fn cool(backup) {
    let cold_fresh = quantity(0);
    for batch in this.batches {
        if batch.at == "fridge" { cold_fresh = cold_fresh + batch.fresh; }
    }
    if cold_fresh == 0 { return; }
    if backup && this.energy < this.law.cooling
        && this.fuel >= this.law.generator_fuel + this.law.trip_fuel {
        this.generated();
    }
    this.chilled();
}

// Processing every warehouse batch's fresh crates that labor and
// electricity allow.
fn process() {
    for id in batch_ids() {
        if this.batches[id].at == "warehouse" {
            let amount = min(this.batches[id].fresh, min(
                afforded(this.labor, this.law.processing.labor),
                afforded(this.energy, this.law.processing.energy),
            ));
            this.processed(id, amount);
        }
    }
}

// Shipping what freight and demand allow: fresh crates before preserved,
// each kind by urgency, highest first, and otherwise in batch order. A
// batch's urgency is ten for each unit of its heat, plus a hundred in cold
// storage when rebalancing, or else one in the warehouse.
fn ship(policy) {
    let shipments = [];
    for id in batch_ids() {
        let batch = this.batches[id];
        if batch.at == "" { continue; }
        let placed = if policy == "rebalance-cold" {
            if batch.at == "fridge" { 100 } else { 0 }
        } else if batch.at == "warehouse" { 1 } else { 0 };
        for preserved in [false, true] {
            shipments.push(#{
                id: id,
                preserved: preserved,
                at: batch.at,
                urgency: batch.heat * 10 + placed,
                order: shipments.len(),
            });
        }
    }
    shipments.sort(|a, b| {
        if a.preserved != b.preserved { return if a.preserved { 1 } else { -1 }; }
        if a.urgency != b.urgency { return if a.urgency > b.urgency { -1 } else { 1 }; }
        a.order - b.order
    });
    for shipment in shipments {
        let batch = this.batches[shipment.id];
        let stock = if shipment.preserved { batch.preserved } else { batch.fresh };
        if stock == 0 { continue; }
        let amount = min(stock, min(
            afforded(this.freight, this.law.delivery.freight),
            afforded(this.demand, this.law.delivery.demand),
        ));
        this.delivered(shipment.id, shipment.preserved, shipment.at, amount);
    }
}

// Discarding every batch's spoiled crates.
fn discard_spoiled() {
    for id in batch_ids() {
        let batch = this.batches[id];
        if batch.at != "" { this.discarded(id, batch.at, batch.spoiled); }
    }
}

// Making room in cold storage while labor lasts: a warehouse batch that fits
// moves whole; one whose fresh crates alone overflow the room splits as
// many as fit into the first dormant batch, which moves. A child inherits
// its parent's heat, and cold storage never resets it.
fn make_room() {
    for id in batch_ids() {
        if this.batches[id].at != "warehouse" || this.labor < this.law.move_labor { continue; }
        let fresh = this.batches[id].fresh;
        let room = afforded(this.cold_space, this.law.slots.fridge);
        if fresh == 0 || room == 0 { continue; }
        if fresh + this.batches[id].preserved <= room {
            this.moved_cold(id);
        } else if fresh > room {
            for child in batch_ids() {
                if !this.batches[child].dormant { continue; }
                if this.split_into(id, child, room) { this.moved_cold(child); }
                break;
            }
        }
    }
}

// The day's end: every batch ages, an empty one without effect, a fresh
// one in cooled storage kept cold, any other warmed a warm day's heat,
// spoiling once it was warm before; then the next day opens with its labor,
// freight, demand and as much of the grid's electricity as the battery
// takes, none during an outage, or, on the last day, the chain closes.
fn end_day(state, day, days) {
    this.planned("end-day", 1);
    for id in batch_ids() {
        let batch = this.batches[id];
        if batch.fresh == 0 {
            this.planned(`age-empty/${id}`, 1);
        } else if batch.at == "fridge" && this.cooling {
            this.planned(`age-cold/${id}`, 1);
        } else {
            let spoil = batch.heat >= this.law.warmth;
            this.planned(`age-warm/${id}/${batch.at}/${spoil}`, 1);
            this.batches[id].heat = batch.heat + this.law.warmth;
            if spoil {
                this.batches[id].spoiled = batch.spoiled + batch.fresh;
                this.batches[id].fresh = quantity(0);
            }
        }
    }
    if day + 1 == days {
        this.planned("close", 1);
        return;
    }
    let next = day + 1;
    this.energy = this.energy + min(grid_supply(state, next), this.law.battery - this.energy);
    this.labor = this.law.day.labor;
    this.freight = this.law.day.freight;
    this.demand = this.law.day.demand;
    this.cooling = false;
    this.planned(`open-day/${next}`, 1);
}

// A policy's plan for every day: protect-cold cools first, with backup;
// process-first processes before shipping and cools after, with backup;
// conserve-fuel ships every other day and cools without backup;
// rebalance-cold cools first, with backup, and processes last, after making
// room.
fn day_plan(state, policy) {
    let chain = shadow(state);
    let days = state.constant("days").to_int();
    let cools_first = policy == "protect-cold" || policy == "rebalance-cold";
    for day in 0..days {
        if cools_first { chain.cool(true); }
        if policy == "process-first" { chain.process(); }
        if policy != "conserve-fuel" || day % 2 == 0 { chain.ship(policy); }
        if policy != "process-first" && policy != "rebalance-cold" { chain.process(); }
        if !cools_first { chain.cool(policy == "process-first"); }
        chain.discard_spoiled();
        chain.make_room();
        if policy == "rebalance-cold" { chain.process(); }
        chain.end_day(state, day, days);
    }
    chain.plan
}

fn protect_cold(state) { day_plan(state, "protect-cold") }
fn process_first(state) { day_plan(state, "process-first") }
fn conserve_fuel(state) { day_plan(state, "conserve-fuel") }
fn rebalance_cold(state) { day_plan(state, "rebalance-cold") }

// The objectives an experiment measures.

// Every account's balance of `asset`.
fn total(state, asset) {
    let sum = quantity(0);
    for account in state.accounts("") { sum = sum + state.balance(account, asset); }
    sum
}

fn saved(state) { total(state, "fresh") + total(state, "preserved") + total(state, "delivered") }
fn delivered_food(state) { total(state, "delivered") }
fn revenue(state) { state.balance("world", "money") }
fn energy_used(state) { total(state, "spent-energy") }
fn fuel_used(state) { total(state, "generator-fuel") + total(state, "transport-fuel") }
fn missed_demand(state) { total(state, "missed-demand") }
