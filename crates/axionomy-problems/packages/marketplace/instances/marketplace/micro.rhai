let model = #{ontology: "marketplace", laws: ["marketplace/micro"]};
let accounts = [];
accounts.push(opening(account("buyer/a"), basket([
    amount(asset("money"), 190),
    amount(asset("purchase-intent/a"), 1),
    amount(asset("purchase-intent/b"), 1)
])));
accounts.push(opening(account("buyer/b"), basket([amount(asset("money"), 100), amount(asset("purchase-intent/a"), 1)])));
accounts.push(opening(account("buyer/c"), basket([amount(asset("money"), 75), amount(asset("purchase-intent/a"), 1)])));
accounts.push(opening(account("carrier/a"), basket([amount(asset("shipping-capacity"), 2)])));
accounts.push(opening(account("carrier/b"), basket([])));
accounts.push(opening(account("order/a"), basket([amount(asset("open-order/a"), 1)])));
accounts.push(opening(account("order/b"), basket([amount(asset("open-order/b"), 1)])));
accounts.push(opening(account("platform"), basket([])));
accounts.push(opening(account("seller/a"), basket([amount(asset("item/widget"), 1), amount(asset("sale-offer/widget"), 1)])));
accounts.push(opening(account("seller/b"), basket([amount(asset("item/widget"), 1), amount(asset("sale-offer/widget"), 1)])));
accounts.push(opening(account("seller/c"), basket([amount(asset("item/gadget"), 1), amount(asset("sale-offer/gadget"), 1)])));
accounts.push(opening(account("tax-authority"), basket([])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("money-accounting");
    rule.weight(asset("money"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("item-accounting");
    rule.weight(asset("item/widget"), 1);
    rule.weight(asset("item/gadget"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("buyer-order-lifecycle");
    for name in ["purchase-intent/a", "purchase-receipt/a", "purchase-intent/b", "purchase-receipt/b"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("seller-order-lifecycle");
    for name in ["sale-offer/widget", "sale-offer/gadget", "completed-sale/a", "completed-sale/b"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
{
    let rule = invariant("shipping-capacity-accounting");
    rule.weight(asset("shipping-capacity"), 1);
    rule.weight(asset("used-shipping-capacity"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("order-lifecycle");
    for name in ["open-order/a", "settled-order/a", "open-order/b", "settled-order/b"] {
        rule.weight(asset(name), 1);
    }
    rules.push(rule.finish());
}
model.invariants = rules;
model
