let model = #{
    ontology: "lending",
    laws: ["lending/common", "lending/stress-bill"],
    tables: #{
        "collect-revenue/{installment}": #{
            variables: [#{name: "installment", range: ["0", "2"]}],
            relations: [
                #{
                    gives: ["amount", "due"],
                    rows: [["0", "10000", "5"], ["1", "12000", "7"], ["2", "18000", "9"]],
                    variables: ["installment"]
                }
            ]
        }
    },
    constants: #{
        "base-rate": "2000000000000000",
        kink: "800000000000000000",
        "liquidation-bonus": "80000000000000000",
        "liquidation-threshold": "750000000000000000",
        "loan-to-value": "600000000000000000",
        "price-drop": "20",
        "reserve-factor": "100000000000000000",
        "shock-after": "6",
        "slope-above-kink": "500000000000000000",
        "slope-below-kink": "25000000000000000"
    }
};
let accounts = [];
accounts.push(opening(account("borrower"), basket([amount(asset("cash"), 4000), amount(asset("collateral"), 1600)])));
accounts.push(opening(account("clock"), basket([amount(asset("period"), 12)])));
accounts.push(opening(account("customer"), basket([
    amount(asset("cash"), 40000),
    amount(asset("purchase-order/0"), 1),
    amount(asset("purchase-order/1"), 1),
    amount(asset("purchase-order/2"), 1)
])));
accounts.push(opening(account("depositor/0"), basket([amount(asset("cash"), 8000)])));
accounts.push(opening(account("depositor/1"), basket([amount(asset("cash"), 8000)])));
accounts.push(opening(account("depositor/2"), basket([amount(asset("cash"), 8000)])));
accounts.push(opening(account("depositor/3"), basket([amount(asset("cash"), 8000)])));
accounts.push(opening(account("keeper"), basket([amount(asset("cash"), 80000)])));
accounts.push(opening(account("market"), basket([
    amount(asset("borrow-index"), 1000000000000000000),
    amount(asset("cash"), 1000),
    amount(asset("collateral-price"), 50),
    amount(asset("locked-share"), 1000),
    amount(asset("share-supply"), 1000)
])));
accounts.push(opening(account("oracle"), basket([amount(asset("pending-shock"), 1)])));
accounts.push(opening(account("supplier"), basket([amount(asset("invoice"), 1)])));
model.accounts = accounts;
let rules = [];
{
    let rule = invariant("cash");
    rule.weight(asset("cash"), 1);
    rules.push(rule.finish());
}
{
    let rule = invariant("vault-shares");
    rule.weight(asset("share"), 1);
    rule.weight(asset("locked-share"), 1);
    rule.weight(asset("share-supply"), -1);
    rules.push(rule.finish());
}
{
    let rule = invariant("debt-shares");
    rule.weight(asset("debt-share"), 1);
    rule.weight(asset("debt-share-supply"), -1);
    rules.push(rule.finish());
}
{
    let rule = invariant("collateral");
    rule.weight(asset("collateral"), 1);
    rule.weight(asset("posted-collateral"), 1);
    rules.push(rule.finish());
}
model.invariants = rules;
model
