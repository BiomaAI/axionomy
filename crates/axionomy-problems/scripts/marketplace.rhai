// Multi-party marketplace: which settlements, contracts, dispatches,
// deliveries, cancellations and clock advances a market may make next, how
// a planner scores a market, and what its participants gain. Micro
// settles each order in one exchange; Showcase and Stress fund contracts that
// are dispatched and delivered over time.

// The id an account's path ends with: `a` for `buyer/a`.
fn id(account) { account.split("/")[1] }

// The market's orders, in identifier order: each has an account of its own.
fn order_ids(state) { state.accounts("order/").map(|account| id(account)) }

fn settlements(state) { state.moves("discount-order") + state.moves("settle-order") }

fn recorded_value(held, kind) {
    let value = 0;
    for asset in held.keys() {
        let parts = asset.split("/");
        if parts[0] == kind { value += parse_int(parts[1]) * held[asset].to_int(); }
    }
    value
}

// The instance agreeing to each order at its price, by order: Micro's
// settlement, `settle-order/<order>`, or a market's contract not conceded,
// `contract/<order>/false`.
fn agreements(state) {
    let found = #{};
    for rate in state.instances("settle-order") + state.instances("contract") {
        if !rate.ends_with("/true") {
            found[rate.split("/")[1]] = rate;
        }
    }
    found
}

// What `agreement` reads of its order's terms: the item it trades, its
// gross price, the utility its buyer and seller gain at that price and, in
// a market, its schedule; with `rate`, the agreement.
fn terms_of(state, agreement) {
    let terms = state.values(agreement);
    terms.rate = agreement;
    terms
}

// Every order's terms, by order.
fn deals(state) {
    let deals = #{};
    for agreement in agreements(state).values() {
        let terms = terms_of(state, agreement);
        deals[terms.order] = terms;
    }
    deals
}

// The exchange of `rate` for `order`, binding the buyer, seller and carrier
// of `party`, the order's account, the platform and the tax authority.
fn bound(rate, order, party) {
    let bindings = #{
        buyer: party.buyer,
        seller: party.seller,
        carrier: party.carrier,
        order: `order/${order}`,
        platform: "platform",
    };
    bindings["tax-authority"] = "tax-authority";
    exchange(rate, bindings, 1)
}

// `proposals` order by order, each buyer with each seller with each carrier
// in the order of their accounts, at the order's price and then ten credits
// less. Of plans scoring alike the beam keeps the last it finds, so this
// order decides which.
fn in_turn(proposals) {
    proposals.sort(|a, b| {
        let x = turn(a);
        let y = turn(b);
        if x < y { -1 } else if x > y { 1 } else { 0 }
    });
    proposals
}
fn turn(proposal) {
    let rate = proposal.rate().split("/");
    let party = ["buyer", "seller", "carrier"].map(|role| proposal.account(role));
    `${rate[1]}/${party[0]}/${party[1]}/${party[2]}/${rate[2]}`
}

// Showcase and Stress: what a funded market may do next, as a planner
// proposes it; nothing once every order has settled. A dispatch or a
// delivery that applies goes first, alone: the first, order by order.
// Otherwise every contract and cancellation that applies, as the rates
// offer them, and advancing the clock to the next arrival, expiry or
// delivery to come, if it applies.
fn fulfillment(state) {
    let orders = order_ids(state);
    if orders.all(|order| state.balance(`order/${order}`, `settled-order/${order}`) > 0) {
        return [];
    }
    let first = state.moves("dispatch") + state.moves("deliver");
    for order in orders {
        for proposal in first {
            if id(proposal.rate()) == order {
                return [proposal];
            }
        }
    }
    let proposals = in_turn(state.moves("contract")) + in_turn(state.moves("cancel"));
    let agreements = agreements(state);
    let now = state.balance("platform", "now");
    // The market's day ends where no time remains.
    let end = now + state.balance("platform", "time-remaining");
    let next = ();
    for order in orders {
        let held = state.balances(`order/${order}`);
        let times = [];
        if `open-order/${order}` in held {
            times.push(terms_of(state, agreements[order]).arrival);
        }
        if `contract/${order}` in held {
            times.push(state.balance(`order/${order}`, "lease-until") + 1);
        }
        if `in-shipment/${order}` in held {
            times.push(state.balance(`order/${order}`, "ready-at"));
        }
        for time in times {
            if time > now && time <= end && (next == () || time < next) {
                next = time;
            }
        }
    }
    if next != () {
        let advance = state.exchange_of(`advance/${next}`);
        if state.applicable(advance) {
            proposals.push(advance);
        }
    }
    proposals
}

// How a planner scores a market for `preference`: every settled
// order first, then less time spent, then the participants' utility, weighted
// twenty to one toward buyers or sellers, or evenly for volume. A contract
// signed but not yet delivered counts toward settling, with the utility its
// price promises the side favored, or both sides for volume.
fn planned(state, preference) {
    let buyers = 0;
    for buyer in state.accounts("buyer/") {
        buyers += state.balance(buyer, "utility").to_int();
    }
    let sellers = 0;
    for seller in state.accounts("seller/") {
        sellers += state.balance(seller, "utility").to_int();
    }
    let value = switch preference {
        "buyers" => buyers * 20 + sellers,
        "sellers" => sellers * 20 + buyers,
        _ => buyers + sellers,
    };
    let settled = 0;
    for order in order_ids(state) {
        let held = state.balances(`order/${order}`);
        if `settled-order/${order}` in held {
            settled += 1;
        }
        if `contract/${order}` in held || `in-shipment/${order}` in held {
            let deal = state.values(`contract/${order}/false`);
            let buyer = deal["buyer-utility"].to_int();
            let seller = deal["seller-utility"].to_int();
            let cut = deal.gross.to_int() - recorded_value(held, "agreed-price");
            value += 40000;
            value += switch preference {
                "buyers" => (buyer + cut) * 20,
                "sellers" => (seller - cut) * 20,
                _ => buyer + seller,
            };
        }
    }
    value + settled * 100000 - state.balance("platform", "now").to_int() * 100
}

fn volume(state) { planned(state, "volume") }
fn favor_buyers(state) { planned(state, "buyers") }
fn favor_sellers(state) { planned(state, "sellers") }

// The order's full price less its recorded settlement.
fn forgone_of(state, deal) {
    deal.gross.to_int() - recorded_value(state.balances(`order/${deal.order}`), "settled-value")
}

// The gross value a clearing forgoes: what every order would pay at its gross
// price, less the value its settlements record.
fn forgone(state) {
    let total = 0;
    for deal in deals(state).values() {
        total += forgone_of(state, deal);
    }
    total
}

// What the orders settled so far forgo, which no later settlement wins back:
// no clearing that continues from here forgoes less.
fn forgone_settled(state) {
    let total = 0;
    for deal in deals(state).values() {
        if state.balance(`order/${deal.order}`, `settled-order/${deal.order}`) > 0 {
            total += forgone_of(state, deal);
        }
    }
    total
}

// What each participant gains, as the settlement terms declare it.
fn utility_of(state, account) { state.balance(account, "utility") }
fn buyer_a(state) { utility_of(state, "buyer/a") }
fn buyer_b(state) { utility_of(state, "buyer/b") }
fn buyer_c(state) { utility_of(state, "buyer/c") }
fn seller_a(state) { utility_of(state, "seller/a") }
fn seller_b(state) { utility_of(state, "seller/b") }
fn seller_c(state) { utility_of(state, "seller/c") }

// What Micro's front is ranked by: the utility its buyers, A and B, gain
// together, and its sellers A and B.
fn front_buyers(state) { buyer_a(state) + buyer_b(state) }
fn front_sellers(state) { seller_a(state) + seller_b(state) }

// The deliveries made by their deadline, and the time the market has taken.
fn on_time(state) {
    let total = 0;
    for order in order_ids(state) {
        total += state.balance(`order/${order}`, "on-time").to_int();
    }
    total
}
fn elapsed(state) { state.balance("platform", "now") }
