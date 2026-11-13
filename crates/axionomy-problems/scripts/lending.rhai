// Lending: a borrower finances a bill that falls due before its revenue
// arrives. Searches try the borrower's moves, then the keeper's, the
// oracle's and the clock's, charging each closed period's interest; the
// over-borrowed scenario plays turn by turn, and the first-depositor attack
// is a plan on a fresh market. The kernel applies or refuses every exchange.

// The fixed-point scale of the borrow index and every market term.
fn wad() { quantity(1_000_000_000_000_000_000) }

// Cash the borrower owes now: its debt shares at the borrow index, rounded
// up.
fn owed(state) {
    mul_div(state.balance("borrower", "debt-share"), state.balance("market", "borrow-index"), wad(), "ceil")
}

// The exchanges, each bound to the accounts its roles name.
fn with_market(name, bindings, units) {
    bindings.market = "market";
    exchange(name, bindings, units)
}
fn for_borrower(name, units) { with_market(name, #{ borrower: "borrower" }, units) }

fn post_collateral(units) { for_borrower("post-collateral", units) }
fn withdraw_collateral(units) { for_borrower("withdraw-collateral", units) }
fn borrow(cash) { for_borrower("borrow", cash) }
fn repay(cash) { for_borrower("repay", cash) }
fn repay_shares(debt_shares) { for_borrower("repay-shares", debt_shares) }
fn pay_bill() {
    exchange("pay-bill", #{ borrower: "borrower", supplier: "supplier", clock: "clock" }, 1)
}
fn collect_revenue(installment) {
    exchange(`collect-revenue/${installment}`,
        #{ borrower: "borrower", customer: "customer", clock: "clock" }, 1)
}
fn accrue() {
    with_market("accrue", #{ borrower: "borrower", clock: "clock", oracle: "oracle" }, 1)
}
fn reprice() { with_market("reprice", #{ clock: "clock", oracle: "oracle" }, 1) }
fn liquidate() { with_market("liquidate", #{ borrower: "borrower", keeper: "keeper" }, 1) }
fn deposit(depositor, cash, min_shares) {
    with_market("deposit", #{ depositor: `depositor/${depositor}` }, cash)
        .parameter("min-shares", quantity(min_shares))
}
fn redeem(depositor, shares, min_cash) {
    with_market("redeem", #{ depositor: `depositor/${depositor}` }, shares)
        .parameter("min-cash", quantity(min_cash))
}
fn donate(depositor, cash) {
    with_market("donate", #{ depositor: `depositor/${depositor}` }, cash)
}

// Search granularity, not semantics: the collateral one posting step moves,
// which each instance sets for the collateral in its economy, the borrower's
// and any a keeper has seized: half of Micro's 40, a quarter of Showcase's
// 400 and of Stress's 1,600.
fn collateral_lot(state) {
    let total = state.balance("borrower", "collateral")
        + state.balance("borrower", "posted-collateral")
        + state.balance("keeper", "collateral");
    switch total.to_int() {
        40 => 20,
        400 => 100,
        1600 => 400,
        _ => throw `no collateral lot is set for ${total} units of collateral`,
    }
}

// All of the borrower's free collateral, and one lot when that is less.
fn lots(state) {
    let free = state.balance("borrower", "collateral");
    if free.is_zero() { return []; }
    let lot = collateral_lot(state);
    if free > lot { [free, lot] } else { [free] }
}

// The borrower's moves, then the keeper, oracle and clock, those that apply.
// The bill is paid at once, so the borrower borrows exactly its shortfall
// while it is unpaid; afterwards it repays everything it owes, or all the
// cash it has, and takes collateral back once the loan is closed.
// Collateral moves all at once or in lots.
fn candidates(state) {
    let cash = state.balance("borrower", "cash");
    let posted = state.balance("borrower", "posted-collateral");
    let debt_shares = state.balance("borrower", "debt-share");
    let bill_paid = state.balance("borrower", "bill-paid") > 0;
    let proposals = [];
    if !bill_paid || debt_shares > 0 {
        for units in lots(state) { proposals.push(post_collateral(units)); }
    }
    if !bill_paid {
        let bill = state.consumed("pay-bill", "borrower")["cash"];
        if cash < bill { proposals.push(borrow(bill - cash)); }
        proposals.push(pay_bill());
    } else if debt_shares > 0 {
        if cash >= owed(state) {
            proposals.push(repay_shares(debt_shares));
        } else if cash > 0 {
            proposals.push(repay(cash));
        }
    } else if posted > 0 {
        proposals.push(withdraw_collateral(posted));
    }
    let installments = state.rates("collect-revenue/").len();
    for installment in 0..installments { proposals.push(collect_revenue(installment)); }
    proposals.push(liquidate());
    proposals.push(reprice());
    proposals.push(accrue());
    proposals.filter(|proposal| state.applicable(proposal))
}

// The interest an exchange charges the borrower: only closing a period
// does, by what the borrower owes after it more than before.
fn interest(before, step, after) {
    if step.rate() != "accrue" { return quantity(0); }
    let now = owed(after);
    let was = owed(before);
    if now > was { now - was } else { quantity(0) }
}

// What the borrower keeps, as the objectives measure it: its collateral,
// free or posted, and its cash.
fn collateral_kept(state) {
    state.balance("borrower", "collateral") + state.balance("borrower", "posted-collateral")
}
fn borrower_cash(state) { state.balance("borrower", "cash") }

// Over-borrowed: post all the collateral, borrow as much as the pool and
// the loan-to-value limit allow, pay the bill, then let the clock run. The
// keeper liquidates the loan as soon as it may, and the scenario ends.
fn over_borrowed(state, random) {
    if state.balance("keeper", "collateral") > 0 { return (); }
    if state.balance("borrower", "bill-paid").is_zero() {
        let free = state.balance("borrower", "collateral");
        if free > 0 { return post_collateral(free); }
        if state.balance("borrower", "debt-share").is_zero() {
            let value = state.balance("borrower", "posted-collateral")
                * state.balance("market", "collateral-price");
            let limit = mul_div(value, state.balance("market", "loan-to-value"), wad(), "floor");
            let debt = owed(state);
            let room = if limit > debt { limit - debt } else { quantity(0) };
            let room = room.min(state.balance("market", "cash"));
            if room > 0 {
                let loan = borrow(room);
                if state.applicable(loan) { return loan; }
            }
        }
        return pay_bill();
    }
    for proposal in [liquidate(), reprice(), accrue()] {
        if state.applicable(proposal) { return proposal; }
    }
    ()
}

// The first-depositor attack on a fresh market: the attacker deposits one
// unit, donates the rest of its cash to inflate the share price, the victim
// deposits all its cash, and the attacker redeems its share.
fn first_depositor_attack(state) {
    let donation = state.balance("depositor/0", "cash") - 1;
    let victim = state.balance("depositor/1", "cash");
    [deposit(0, 1, 1), donate(0, donation), deposit(1, victim, 1), redeem(0, 1, 0)]
}
