// Multi-party marketplace: which settlements, contracts, dispatches,
// deliveries, cancellations and clock advances a market may make next, how
// the study's planner scores a market, and what its participants gain. Micro
// settles each order in one exchange; Showcase and Stress fund contracts that
// are dispatched and delivered over time.

// What an order's rates encode: the item it trades, its gross price, and the
// utility its buyer and seller gain when it settles at that price.
fn terms(order) {
    switch order {
        "a" => #{ item: "widget", gross: 100, buyer: 30, seller: 20 },
        "b" => #{ item: "gadget", gross: 90, buyer: 25, seller: 18 },
        "c" => #{ item: "widget", gross: 110, buyer: 34, seller: 17 },
        "d" => #{ item: "gadget", gross: 95, buyer: 22, seller: 24 },
        "e" => #{ item: "widget", gross: 105, buyer: 28, seller: 21 },
        "f" => #{ item: "gadget", gross: 115, buyer: 31, seller: 19 },
    }
}

// The market's orders, one for each settlement rate, in identifier order.
fn order_ids(state) {
    let orders = [];
    for rate in state.rates("settle-order/") {
        orders.push(rate.sub_string(13));
    }
    orders
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

// The buyers, sellers and carriers who might take part in `order`, each in
// the order of the accounts, every buyer with every seller with every
// carrier: buyers holding the order's purchase intent, sellers offering its
// item and carriers with capacity, which a settlement or contract consumes.
// Whether one applies is the kernel's to say; this only skips parties it
// would refuse.
fn parties(state, order) {
    let item = terms(order).item;
    let buyers = [];
    for buyer in state.accounts("buyer/") {
        if state.balance(buyer, `purchase-intent/${order}`) > 0 {
            buyers.push(buyer);
        }
    }
    let sellers = [];
    for seller in state.accounts("seller/") {
        let held = state.balances(seller);
        if `sale-offer/${item}` in held && `item/${item}` in held {
            sellers.push(seller);
        }
    }
    let carriers = [];
    for carrier in state.accounts("carrier/") {
        if state.balance(carrier, "shipping-capacity") > 0 {
            carriers.push(carrier);
        }
    }
    let found = [];
    for buyer in buyers {
        for seller in sellers {
            for carrier in carriers {
                found.push(#{ buyer: buyer, seller: seller, carrier: carrier });
            }
        }
    }
    found
}

// Micro: every order's settlement that applies, order by order, then by
// buyer, seller and carrier, as the study derives them from the accounts.
fn settlements(state) {
    let proposals = [];
    for order in order_ids(state) {
        for party in parties(state, order) {
            let proposal = bound(`settle-order/${order}`, order, party);
            if state.applicable(proposal) {
                proposals.push(proposal);
            }
        }
    }
    proposals
}

// Whom `order`'s reservation binds: the first buyer who locked the purchase,
// the first seller who reserved the sale and the first carrier holding the
// booking; nothing unless there are all three.
fn booking(state, order) {
    let party = #{};
    for buyer in state.accounts("buyer/") {
        if state.balance(buyer, `purchase-locked/${order}`) > 0 {
            party.buyer = buyer;
            break;
        }
    }
    for seller in state.accounts("seller/") {
        if state.balance(seller, `sale-reserved/${order}`) > 0 {
            party.seller = seller;
            break;
        }
    }
    for carrier in state.accounts("carrier/") {
        if state.balance(carrier, `booking/${order}`) > 0 {
            party.carrier = carrier;
            break;
        }
    }
    if party.len() == 3 { party } else { () }
}

// Showcase and Stress: what a funded market may do next, as the study's
// planner proposes it; nothing once every order has settled. A dispatch or a
// delivery that applies goes first, alone: the first, order by order, of a
// reservation's dispatch, then its delivery at the order's price on time and
// late, then at the conceded price on time and late. Otherwise every
// contract an open order may sign, at its price and then ten credits less,
// with each buyer, seller and carrier in turn; a reservation's cancellation,
// at either price, once it has expired; and advancing the clock to the next
// arrival, expiry or delivery to come, whichever apply.
fn fulfillment(state) {
    let orders = order_ids(state);
    let settled = 0;
    let open = #{};
    let booked = #{};
    for order in orders {
        let held = state.balances(`order/${order}`);
        if `settled-order/${order}` in held {
            settled += 1;
        }
        if `open-order/${order}` in held {
            open[order] = true;
        }
        let party = booking(state, order);
        if party != () {
            booked[order] = #{ party: party, held: held };
        }
    }
    if settled == orders.len() {
        return [];
    }
    for order in orders {
        if order in booked {
            let party = booked[order].party;
            for rate in [
                `dispatch/${order}`,
                `deliver/${order}/false/false`,
                `deliver/${order}/false/true`,
                `deliver/${order}/true/false`,
                `deliver/${order}/true/true`,
            ] {
                let proposal = bound(rate, order, party);
                if state.applicable(proposal) {
                    return [proposal];
                }
            }
        }
    }
    let proposed = [];
    for order in orders {
        if order in open {
            for party in parties(state, order) {
                proposed.push(bound(`settle-order/${order}`, order, party));
                proposed.push(bound(`discounted-contract/${order}`, order, party));
            }
        }
    }
    let now = state.balance("platform", "now");
    let next = ();
    for order in orders {
        let times = [];
        if order in open {
            times.push(state.balance(`order/${order}`, "arrival"));
        }
        if order in booked {
            let party = booked[order].party;
            let held = booked[order].held;
            proposed.push(bound(`cancel/${order}/false`, order, party));
            proposed.push(bound(`cancel/${order}/true`, order, party));
            if `contract/${order}` in held {
                times.push(state.balance(`order/${order}`, "lease-until") + 1);
            }
            if `in-shipment/${order}` in held {
                times.push(state.balance(`order/${order}`, "ready-at"));
            }
        }
        for time in times {
            if time > now && time <= 32 && (next == () || time < next) {
                next = time;
            }
        }
    }
    if next != () {
        proposed.push(exchange(`advance/${next}`, #{ platform: "platform" }, 1));
    }
    let proposals = [];
    for proposal in proposed {
        if state.applicable(proposal) {
            proposals.push(proposal);
        }
    }
    proposals
}

// How the study's planner scores a market for `preference`: every settled
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
            let deal = terms(order);
            let cut = if `agreed-price/${deal.gross - 10}` in held { 10 } else { 0 };
            value += 40000;
            value += switch preference {
                "buyers" => (deal.buyer + cut) * 20,
                "sellers" => (deal.seller - cut) * 20,
                _ => deal.buyer + deal.seller,
            };
        }
    }
    value + settled * 100000 - state.balance("platform", "now").to_int() * 100
}

fn volume(state) { planned(state, "volume") }
fn favor_buyers(state) { planned(state, "buyers") }
fn favor_sellers(state) { planned(state, "sellers") }

// The value `order`'s settlement records: at its gross price, ten credits
// less, or twenty less once a concession and a late rebate both apply.
fn settled_value(state, order) {
    let gross = terms(order).gross;
    let value = 0;
    for price in [gross, gross - 10, gross - 20] {
        value += state.balance(`order/${order}`, `settled-value/${price}`).to_int() * price;
    }
    value
}

// The gross value a clearing forgoes: what every order would pay at its gross
// price, less the value its settlements record.
fn forgone(state) {
    let total = 0;
    for order in order_ids(state) {
        total += terms(order).gross - settled_value(state, order);
    }
    total
}

// What the orders settled so far forgo, which no later settlement wins back:
// no clearing that continues from here forgoes less.
fn forgone_settled(state) {
    let total = 0;
    for order in order_ids(state) {
        if state.balance(`order/${order}`, `settled-order/${order}`) > 0 {
            total += terms(order).gross - settled_value(state, order);
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
