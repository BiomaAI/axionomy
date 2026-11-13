// The Living Market: energy and settlement credit trade through an exact
// constant-product pool, and the actors' needs, obligations, production and
// liquidity move the price it discovers. Micro is one market day, every
// actor's errands in a fixed order, each quoted against the reserves the
// errands before it leave, so its strategies are plans. Showcase and Stress
// are seasons: each day, rounds visit every actor still trading, and each
// decides from the live economy or ends its trading, turn by turn.

// The actors, in the order a day's rounds visit them.
fn actors() {
    ["founder", "generator", "factory", "household", "speculator", "adaptive-lp", "arbitrageur", "whale"]
}

// Every share the pool may issue, and the decisions each actor has a day.
fn share_limit() { 1000000 }
fn daily_decisions() { 6 }

// `amount` divided by `divisor`, rounded down or up.
fn divided(amount, divisor) { mul_div(amount, quantity(1), quantity(divisor), "floor") }
fn divided_up(amount, divisor) { mul_div(amount, quantity(1), quantity(divisor), "ceil") }

// `amount` less `taken`, or nothing when that is more.
fn less(amount, taken) { if amount > taken { amount - taken } else { quantity(0) } }

// What the pool returns from its reserve `returned` for `input` paid into
// its reserve `paid`: the constant product after the 0.3% fee, rounded down
// for the trader.
fn quote(paid, returned, input) {
    let after_fee = input * 997;
    mul_div(returned, after_fee, paid * 1000 + after_fee, "floor")
}

// The trader's protection: 98% of what the pool quotes, rounded down.
fn guarded(output) { mul_div(output, quantity(98), quantity(100), "floor") }

// The pool's energy for `credit`, and its credit for `energy`.
fn energy_for(state, credit) {
    quote(state.balance("pool", "credit"), state.balance("pool", "energy"), credit)
}
fn credit_for(state, energy) {
    quote(state.balance("pool", "energy"), state.balance("pool", "credit"), energy)
}

// The shares the pool has issued.
fn issued(state) { quantity(share_limit()) - state.balance("pool", "unissued-lp-share") }

// Credit per energy, scaled by 1,000 and rounded down; nothing without energy.
fn price_milli(state) {
    let energy = state.balance("pool", "energy");
    if energy == 0 { return quantity(0); }
    mul_div(state.balance("pool", "credit"), quantity(1000), energy, "floor")
}

// What an actor holds of `asset`.
fn held(state, actor, asset) { state.balance(`actor/${actor}`, asset) }

// `proposal` when it applies, and nothing otherwise.
fn allowed(state, proposal) { if state.applicable(proposal) { proposal } else { () } }

// The exchanges an actor proposes.
fn buy(actor, credit, minimum) {
    exchange("buy-energy", #{ pool: "pool", trader: `actor/${actor}` }, credit)
        .parameter("minimum-output", minimum)
}
fn sale(actor, energy, minimum) {
    exchange("sell-energy", #{ pool: "pool", trader: `actor/${actor}` }, energy)
        .parameter("minimum-output", minimum)
}
fn add_liquidity(actor, energy, credit, minimum) {
    exchange("add-liquidity", #{ pool: "pool", "liquidity-provider": `actor/${actor}` }, 1)
        .parameter("energy-amount", energy)
        .parameter("credit-amount", credit)
        .parameter("minimum-lp-shares", minimum)
}
fn remove_liquidity(actor, shares) {
    exchange("remove-liquidity", #{ pool: "pool", "liquidity-provider": `actor/${actor}` }, shares)
}
fn fund(actor, shares) {
    exchange("fund-liquidity", #{ pool: "pool", "liquidity-provider": `actor/${actor}` }, shares)
}
fn production(actor, units) { exchange("produce-energy", #{ producer: `actor/${actor}` }, units) }
fn consumption(actor) { exchange(`use-energy/${actor}`, #{ consumer: `actor/${actor}` }, 1) }
fn settlement(actor) {
    exchange(`settle-obligation/${actor}`, #{ consumer: `actor/${actor}`, treasury: "treasury" }, 1)
}
fn learning(actor) {
    exchange(`learn-shortage/${actor}`, #{ information: "information", observer: `actor/${actor}` }, 1)
}
fn end_trading(actor) { exchange(`end-trading/${actor}`, #{ consumer: `actor/${actor}` }, 1) }

// Micro: one market day, planned whole.

// The pool as a plan follows it: its reserves and the shares it has issued.
fn reserves(state) {
    #{
        energy: state.balance("pool", "energy"),
        credit: state.balance("pool", "credit"),
        issued: issued(state)
    }
}

// The pool after a trader pays `input` into `paid` (`energy` or `credit`)
// for the other reserve; what the trader receives.
fn swapped(paid, returned, input) {
    let output = quote(this[paid], this[returned], input);
    this[paid] = this[paid] + input;
    this[returned] = this[returned] - output;
    output
}

// The pool after a provider burns `shares`, paid its part of each reserve,
// rounded down.
fn withdrawn(shares) {
    let energy = mul_div(this.energy, shares, this.issued, "floor");
    let credit = mul_div(this.credit, shares, this.issued, "floor");
    this.energy = this.energy - energy;
    this.credit = this.credit - credit;
    this.issued = this.issued - shares;
}

// The pool after a provider deposits `energy` and `credit` for the shares
// the smaller side earns, rounded down.
fn deposited(energy, credit) {
    let minted = min(
        mul_div(energy, this.issued, this.energy, "floor"),
        mul_div(credit, this.issued, this.credit, "floor")
    );
    this.energy = this.energy + energy;
    this.credit = this.credit + credit;
    this.issued = this.issued + minted;
}

// An actor's guarded purchase with `credit`, and sale of `energy`.
fn bought_with(actor, credit) {
    let output = this.swapped("credit", "energy", credit);
    buy(actor, credit, guarded(output))
}
fn sold_for(actor, energy) {
    let output = this.swapped("energy", "credit", energy);
    sale(actor, energy, guarded(output))
}

// An actor's guarded purchase of at least `required` energy: `preferred`
// credit, raised by 100 until the pool returns enough.
fn bought_at_least(actor, preferred, required) {
    let credit = preferred;
    while quote(this.credit, this.energy, credit) < required { credit = credit + 100; }
    this.bought_with(actor, credit)
}

// The day's errands in order: liquidity first (the adaptive provider's
// tenth of each reserve, or, in thin liquidity, the founder's withdrawal of
// half the shares), the generator's production and sale, the factory's and
// household's purchases for their needs, the speculator learning of the
// shortage and buying, the whale's purchase but in `no-whale`, the
// generator's second production and sale, the arbitrageur's sale and the
// generator settling its obligation.
fn day_plan(state, scenario) {
    let pool = reserves(state);
    let scale = if pool.energy >= 40000 { 4 } else { 1 };
    let plan = [];
    if scenario == "thin-liquidity" {
        let shares = divided(pool.issued, 2);
        plan.push(remove_liquidity("founder", shares));
        pool.withdrawn(shares);
    } else {
        let energy = divided(pool.energy, 10);
        let credit = divided(pool.credit, 10);
        plan.push(add_liquidity("adaptive-lp", energy, credit, energy));
        pool.deposited(energy, credit);
    }
    plan.push(production("generator", 1500 * scale));
    plan.push(pool.sold_for("generator", quantity(2500 * scale)));
    plan.push(pool.bought_at_least("factory", quantity(26000 * scale), 1500));
    plan.push(consumption("factory"));
    plan.push(settlement("factory"));
    plan.push(pool.bought_at_least("household", quantity(5000 * scale), 350));
    plan.push(consumption("household"));
    plan.push(learning("speculator"));
    plan.push(pool.bought_with("speculator", quantity(16000 * scale)));
    if scenario != "no-whale" {
        plan.push(pool.bought_with("whale", quantity(45000 * scale)));
    }
    plan.push(production("generator", 1800 * scale));
    plan.push(pool.sold_for("generator", quantity(1800 * scale)));
    plan.push(pool.sold_for("arbitrageur", quantity(1200 * scale)));
    plan.push(settlement("generator"));
    plan
}

// The market day, without the whale, and with thin liquidity.
fn market_day_plan(state) { day_plan(state, "market-day") }
fn no_whale_plan(state) { day_plan(state, "no-whale") }
fn thin_liquidity_plan(state) { day_plan(state, "thin-liquidity") }

// Showcase and Stress: a season, turn by turn.

// The season's day, counting from zero.
fn today(state) { state.balance("world", "day").to_int() }

// The fewest credit buying enough energy to hold `target`, within `budget`,
// found by bisection over the pool's quotes; nothing when the budget cannot.
fn buy_to(state, actor, target, budget) {
    let missing = less(target, held(state, actor, "energy"));
    if missing == 0 || budget == 0 || energy_for(state, budget) < missing { return (); }
    let low = 1;
    let high = budget.to_int();
    while low < high {
        let middle = low + (high - low) / 2;
        if energy_for(state, quantity(middle)) >= missing { high = middle; } else { low = middle + 1; }
    }
    allowed(state, buy(actor, quantity(low), missing))
}

// A sale of up to `units` of the actor's energy at the pool's full quote.
fn sell(state, actor, units) {
    let amount = min(units, held(state, actor, "energy"));
    if amount == 0 { return (); }
    allowed(state, sale(actor, amount, credit_for(state, amount)))
}

// What an actor decides from the live reserves, its stock, credit, needs,
// obligations and encoded closing-price memory; nothing ends its trading.
fn decision(state, actor) {
    let remaining = held(state, actor, "decisions");
    if held(state, actor, "trading") == 0 || remaining == 0 { return (); }
    let scale = state.balance("world", "scale");
    let price = price_milli(state);
    let reference = held(state, actor, "reference-price");
    let energy = held(state, actor, "energy");
    let cash = held(state, actor, "credit");
    if (actor == "factory" || actor == "generator")
        && held(state, actor, `obligation/${actor}`) > 0
        && (remaining <= 2 || (actor == "factory" && held(state, actor, "need/factory") == 0)) {
        let settle = settlement(actor);
        if state.applicable(settle) { return settle; }
    }
    switch actor {
        "founder" => (),
        "generator" => {
            if price > 12500 && cash > 10000 * scale {
                let invest = exchange(
                    "invest-production",
                    #{ producer: "actor/generator", world: "world", treasury: "treasury" },
                    1
                );
                if state.applicable(invest) { return invest; }
            }
            if energy > 800 * scale || (energy > 0 && (price > 9000 || cash < 4000 * scale)) {
                let sold = sell(state, actor, 1000 * scale);
                if sold != () { return sold; }
            }
            let amount = min(
                min(held(state, actor, "production-allowance"), held(state, actor, "solar-potential")),
                less(held(state, actor, "storage-capacity"), energy)
            );
            if amount > 0 && price >= 7000 {
                allowed(state, production(actor, amount))
            } else {
                allowed(state, settlement(actor))
            }
        }
        "factory" => {
            let deliver = exchange("deliver-goods", #{ producer: "actor/factory", treasury: "treasury" }, 1);
            if state.applicable(deliver) { return deliver; }
            if held(state, actor, "need/factory") > 0 {
                let consume = consumption(actor);
                if state.applicable(consume) { return consume; }
                // Production can be declined when input prices exceed the
                // procurement revenue.
                if price <= 19000 {
                    return buy_to(state, actor, 1500 * scale, less(cash, 2000 * scale));
                }
            }
            allowed(state, settlement(actor))
        }
        "household" => {
            let income = exchange("collect-income", #{ consumer: "actor/household", treasury: "treasury" }, 1);
            if state.applicable(income) { return income; }
            if held(state, actor, "need/household") > 0 {
                let consume = consumption(actor);
                if state.applicable(consume) { return consume; }
                return buy_to(state, actor, 350 * scale, cash);
            }
            if price < 8000 { buy_to(state, actor, 700 * scale, divided(cash, 2)) } else { () }
        }
        "speculator" => {
            let learn = learning(actor);
            if state.applicable(learn) { return learn; }
            let dear = divided(reference * 11, 10);
            if energy > 0 && price > dear { return sell(state, actor, 500 * scale); }
            if held(state, actor, "informed/speculator") > 0 && price < dear {
                buy_to(state, actor, min(energy + 500 * scale, 2500 * scale), divided(cash, 3))
            } else if price < divided(reference * 9, 10) {
                buy_to(state, actor, 1000 * scale, divided(cash, 3))
            } else {
                ()
            }
        }
        "arbitrageur" => {
            if price > divided(reference * 11, 10) {
                sell(state, actor, 500 * scale)
            } else if price < divided(reference * 9, 10) {
                buy_to(state, actor, 2000 * scale, divided(cash, 3))
            } else {
                ()
            }
        }
        "whale" => {
            let period = today(state);
            if period % 3 == 2 && price > reference { return sell(state, actor, 1000 * scale); }
            if period % 3 != 2 && price < 18000 {
                buy_to(state, actor, min(energy + 500 * scale, 5000 * scale), divided(cash, 4))
            } else {
                ()
            }
        }
        "adaptive-lp" => {
            let shares = held(state, actor, "lp-share");
            let high = divided(reference * 12, 10);
            let low = divided(reference * 8, 10);
            if shares > 0 && (price > high || price < low) {
                return allowed(state, remove_liquidity(actor, divided_up(shares, 2)));
            }
            if price >= low && price <= high {
                allowed(state, fund(actor, max(divided(issued(state), 20), quantity(1))))
            } else {
                ()
            }
        }
    }
}

// The turns an actor still trading has taken today. The founder decides
// nothing in a season, ending its trading at its first turn, so while it
// trades it has taken none: thin liquidity's opening withdrawal is not a
// turn of a round.
fn turns(state, actor) {
    if actor == "founder" { 0 } else { daily_decisions() - held(state, actor, "decisions").to_int() }
}

// The day's close once every actor has ended trading: the next day's
// entitlements, or, on the last day, the market's close.
fn day_end(state, day) {
    let last = day + 1 == state.balance("world", "days").to_int();
    let bindings = #{ world: "world", pool: "pool" };
    for actor in actors() { bindings[`participant/${actor}`] = `actor/${actor}`; }
    if !last { bindings["information"] = "information"; }
    exchange(if last { "close-market" } else { `advance-day/${day}` }, bindings, 1)
}

// One turn of a season. Thin liquidity opens with the founder withdrawing
// half the issued shares. Each day opens by commissioning capacity that has
// come due, the only time it can; then rounds visit every actor still
// trading, round `r` from actor `day + r` onwards, each deciding or ending
// its trading; and the day closes once none trades. An actor still trading has taken a turn each
// round, so the round is the fewest turns any has taken, and the next to act
// is the first from the round's first actor that has taken no more.
fn season_turn(state, scenario) {
    let day = today(state);
    if scenario == "thin-liquidity" && day == 0 && held(state, "founder", "liquidity-decision") > 0 {
        return remove_liquidity("founder", divided(issued(state), 2));
    }
    let due = exchange("commission-production", #{ producer: "actor/generator", world: "world" }, 1);
    if state.applicable(due) { return due; }
    let everyone = actors();
    let trading = [];
    let round = ();
    for actor in everyone {
        if held(state, actor, "trading") > 0 {
            trading.push(actor);
            let taken = turns(state, actor);
            if round == () || taken < round { round = taken; }
        }
    }
    if trading.is_empty() { return day_end(state, day); }
    let first = (day + round) % everyone.len();
    for offset in 0..everyone.len() {
        let actor = everyone[(first + offset) % everyone.len()];
        if trading.contains(actor) && turns(state, actor) == round {
            let decided = if scenario == "no-whale" && actor == "whale" { () } else { decision(state, actor) };
            return if decided == () { end_trading(actor) } else { decided };
        }
    }
    ()
}

// The market season, without the whale trading, and with thin liquidity.
fn market_day(state, random) { season_turn(state, "market-day") }
fn no_whale(state, random) { season_turn(state, "no-whale") }
fn thin_liquidity(state, random) { season_turn(state, "thin-liquidity") }

// The objectives, as the study measures them.

// Needs satisfied and missed, the factory's and the household's.
fn needs(state) {
    held(state, "factory", "satisfied-need/factory") + held(state, "household", "satisfied-need/household")
}
fn unmet(state) {
    held(state, "factory", "missed-need/factory") + held(state, "household", "missed-need/household")
}

// Utility every actor realised.
fn utility(state) {
    let total = quantity(0);
    for account in state.accounts("actor/") { total = total + state.balance(account, "utility"); }
    total
}

// The pool's energy: the market's depth.
fn depth(state) { state.balance("pool", "energy") }

// Obligations missed, the factory's and the generator's.
fn defaulted(state) {
    held(state, "factory", "defaulted-obligation/factory")
        + held(state, "generator", "defaulted-obligation/generator")
}
