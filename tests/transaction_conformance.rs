//! Whole-transaction regression bridge to the executable Lean specification.
//! The universal implementation guarantee comes from the Aeneas refinements.
use axionomy::{
    Account, AccountAssessment, AccountDelta, AccountShortfall, ApplyError, Basket, Economy,
    EconomyBuilder, Exchange, ExchangeAssessment, LinearInvariant, Quantity, QuantityComparison,
    QuantityExpression, Rate, Receipt, Trace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

macro_rules! identifier {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize,
        )]
        #[serde(transparent)]
        struct $name(u64);
    };
}
identifier!(Holder);
identifier!(Asset);
identifier!(RuleId);
identifier!(Role);

type World = Economy<Holder, Asset, RuleId, Role>;
type Proposal = Exchange<RuleId, Role, Holder>;
type Failure = ApplyError<RuleId, Role, Holder, Asset>;
type Amounts = Vec<(Asset, String)>;
type Expression = QuantityExpression<Role, Asset>;

#[derive(Debug, Deserialize)]
struct Computed {
    role: Role,
    asset: Asset,
    quantity: Expression,
}

#[derive(Debug, Deserialize)]
struct Condition {
    label: String,
    left: Expression,
    comparison: String,
    right: Expression,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleCase {
    consume: Vec<(Role, Amounts)>,
    produce: Vec<(Role, Amounts)>,
    preserve: Vec<(Role, Amounts)>,
    computed_consume: Vec<Computed>,
    computed_produce: Vec<Computed>,
    computed_preserve: Vec<Computed>,
    conditions: Vec<Condition>,
    distinct: Vec<(Role, Role)>,
}

#[derive(Debug, Deserialize)]
struct ProposalCase {
    rate: RuleId,
    units: String,
    bindings: Vec<(Role, Holder)>,
    parameters: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct InvariantCase {
    label: String,
    weights: Vec<(Asset, String)>,
}

#[derive(Debug, Deserialize)]
struct Case {
    label: String,
    accounts: Vec<(Holder, Amounts)>,
    rule: RuleCase,
    proposal: ProposalCase,
    invariants: Vec<InvariantCase>,
    expected: Value,
}

#[derive(Deserialize)]
struct Fixtures {
    transactions: Vec<Case>,
    executions: Vec<ExecutionCase>,
}

#[derive(Debug, Deserialize)]
struct ExecutionCase {
    base: Case,
    trace: Vec<ProposalCase>,
    expected: Value,
}

fn basket(entries: &Amounts) -> Basket<Asset> {
    Basket::try_from_entries(
        entries
            .iter()
            .map(|(asset, value)| (*asset, Quantity::new(value.parse::<u64>().unwrap()))),
    )
    .unwrap()
}

fn balances(basket: &Basket<Asset>) -> Value {
    json!(
        basket
            .iter()
            .map(|(asset, value)| (asset, value.to_string()))
            .collect::<Vec<_>>()
    )
}

fn snapshot(world: &World) -> Value {
    json!(
        world
            .accounts()
            .map(|(id, holder)| (id, balances(holder.balances())))
            .collect::<Vec<_>>()
    )
}

fn deltas(entries: &[AccountDelta<Holder, Asset>]) -> Value {
    json!(
        entries
            .iter()
            .map(|entry| json!({
                "account": entry.account(), "consumed": balances(entry.consumed()),
                "produced": balances(entry.produced()), "preserved": balances(entry.preserved()),
            }))
            .collect::<Vec<_>>()
    )
}

fn shortfalls(entries: &[AccountShortfall<Holder, Asset>]) -> Value {
    json!(
        entries
            .iter()
            .map(|entry| json!({
                "account": entry.account(), "missing": balances(entry.missing()),
            }))
            .collect::<Vec<_>>()
    )
}

fn assessed_accounts(entries: &[AccountAssessment<Holder, Asset>]) -> Value {
    json!(entries.iter().map(|entry| json!({
        "account": entry.account(), "available": balances(entry.available()), "required": balances(entry.required()),
        "consumed": balances(entry.consumed()), "produced": balances(entry.produced()), "preserved": balances(entry.preserved()),
    })).collect::<Vec<_>>())
}

fn failure(error: &Failure) -> Value {
    match error {
        ApplyError::MissingRate { rate } => json!({"kind": "missing_rate", "rate": rate}),
        ApplyError::MissingBinding { role } => json!({"kind": "missing_binding", "role": role}),
        ApplyError::UnknownBinding { role } => json!({"kind": "unknown_binding", "role": role}),
        ApplyError::RolesMustDiffer { left, right } => {
            json!({"kind": "roles_must_differ", "left": left, "right": right})
        }
        ApplyError::MissingAccount { account } => {
            json!({"kind": "missing_account", "account": account})
        }
        ApplyError::ZeroUnits => json!({"kind": "zero_units"}),
        ApplyError::MissingParameter { name } => {
            json!({"kind": "missing_parameter", "parameter": name})
        }
        ApplyError::UnknownParameter { name } => {
            json!({"kind": "unknown_parameter", "parameter": name})
        }
        ApplyError::RateOverflow { rate, asset } => {
            json!({"kind": "rate_overflow", "rate": rate, "asset": asset})
        }
        ApplyError::RateLawEvaluation { rate, message } => {
            json!({"kind": "law_evaluation", "rate": rate, "message": message})
        }
        ApplyError::RateConditionFailed { rate, condition } => {
            json!({"kind": "condition_failed", "rate": rate, "label": condition})
        }
        ApplyError::Infeasible {
            shortfalls: entries,
        } => json!({"kind": "infeasible", "shortfalls": shortfalls(entries)}),
        ApplyError::BalanceOverflow { account, asset } => {
            json!({"kind": "balance_overflow", "account": account, "asset": asset})
        }
        ApplyError::InvariantOverflow { invariant } => {
            json!({"kind": "invariant_overflow", "label": invariant})
        }
        ApplyError::InvariantViolation {
            invariant,
            before,
            after,
        } => json!({
            "kind": "invariant_violation", "label": invariant, "before": before.to_string(), "after": after.to_string(),
        }),
    }
}

fn assessment(value: &ExchangeAssessment<Holder, Asset, RuleId, Role>) -> Value {
    match value {
        ExchangeAssessment::Applicable {
            accounts,
            projected_deltas,
        } => json!({
            "kind": "applicable", "accounts": assessed_accounts(accounts), "deltas": deltas(projected_deltas),
        }),
        ExchangeAssessment::Infeasible {
            accounts,
            shortfalls: missing,
        } => json!({
            "kind": "infeasible", "accounts": assessed_accounts(accounts), "shortfalls": shortfalls(missing),
        }),
        ExchangeAssessment::Invalid { issues } => {
            json!({"kind": "invalid", "issues": issues.iter().map(failure).collect::<Vec<_>>()})
        }
    }
}

fn proposal(value: &Proposal) -> Value {
    json!({
        "rate": value.rate(), "units": value.units().to_string(),
        "bindings": value.bindings().iter().collect::<Vec<_>>(),
        "parameters": value.parameters().iter().map(|(key, amount)| (key, amount.to_string())).collect::<Vec<_>>(),
    })
}

fn build(case: &Case) -> (World, Proposal) {
    let mut rule = Rate::new();
    for (role, entries) in &case.rule.consume {
        rule = rule.try_consume(*role, basket(entries)).unwrap();
    }
    for (role, entries) in &case.rule.produce {
        rule = rule.try_produce(*role, basket(entries)).unwrap();
    }
    for (role, entries) in &case.rule.preserve {
        rule = rule.try_preserve(*role, basket(entries)).unwrap();
    }
    for amount in &case.rule.computed_consume {
        rule = rule.consume_computed(amount.role, amount.asset, amount.quantity.clone());
    }
    for amount in &case.rule.computed_produce {
        rule = rule.produce_computed(amount.role, amount.asset, amount.quantity.clone());
    }
    for amount in &case.rule.computed_preserve {
        rule = rule.preserve_computed(amount.role, amount.asset, amount.quantity.clone());
    }
    for condition in &case.rule.conditions {
        let comparison = match condition.comparison.as_str() {
            "equal" => QuantityComparison::Equal,
            "greater_or_equal" => QuantityComparison::GreaterThanOrEqual,
            "less_or_equal" => QuantityComparison::LessThanOrEqual,
            other => panic!("unknown comparison {other}"),
        };
        rule = rule.condition(
            &condition.label,
            condition.left.clone(),
            comparison,
            condition.right.clone(),
        );
    }
    for (left, right) in &case.rule.distinct {
        rule = rule.distinct(*left, *right);
    }
    let mut builder = EconomyBuilder::new().rate(RuleId(7), rule);
    for (id, entries) in &case.accounts {
        builder = builder.account(*id, Account::new(basket(entries)));
    }
    for invariant in &case.invariants {
        let mut value = LinearInvariant::new(&invariant.label);
        for (asset, weight) in &invariant.weights {
            value = value.weight(*asset, weight.parse::<i64>().unwrap());
        }
        builder = builder.invariant(value);
    }
    (builder.build().unwrap(), build_proposal(&case.proposal))
}

fn build_proposal(case: &ProposalCase) -> Proposal {
    let mut action = Exchange::new(case.rate, Quantity::new(case.units.parse::<u64>().unwrap()));
    for (role, account) in &case.bindings {
        action = action.bind(*role, *account);
    }
    for (key, value) in &case.parameters {
        action = action.parameter(key, Quantity::new(value.parse::<u64>().unwrap()));
    }
    action
}

fn receipt(value: &Receipt<RuleId, Role, Holder, Asset>) -> Value {
    json!({"proposal": proposal(value.exchange()), "deltas": deltas(value.deltas())})
}

#[test]
fn complete_transactions_match_lean() {
    let fixtures: Fixtures =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    assert_eq!(fixtures.transactions.len(), 596);
    for (index, case) in fixtures.transactions.iter().enumerate() {
        assert_ne!(
            case.expected.get("kind"),
            Some(&json!("panic")),
            "Lean specification panicked: {case:?}"
        );
        let (mut world, action) = build(case);
        let before = serde_json::to_value(&world).unwrap();
        match world.simulate(action.clone()) {
            Ok((branch, actual)) => {
                assert!(case.expected["error"].is_null(), "simulation: {case:?}");
                assert_eq!(
                    snapshot(&branch),
                    case.expected["after"],
                    "simulation: {case:?}"
                );
                assert_eq!(
                    receipt(&actual),
                    case.expected["receipt"],
                    "simulation: {case:?}"
                );
            }
            Err(error) => assert_eq!(
                failure(&error),
                case.expected["error"],
                "simulation: {case:?}"
            ),
        }
        assert_eq!(
            serde_json::to_value(&world).unwrap(),
            before,
            "simulation mutated source: {case:?}"
        );
        let assessed = world.assess(&action);
        assert_eq!(
            assessment(&assessed),
            case.expected["assessment"],
            "case {index}: {}",
            case.label
        );
        assert_eq!(
            serde_json::to_value(&world).unwrap(),
            before,
            "assessment mutated state: {case:?}"
        );
        assert_eq!(world.is_applicable(&action), assessed.is_applicable());
        let invalid = Exchange::new(RuleId(7), Quantity::new(0));
        assert_eq!(
            world.applicable([action.clone(), invalid, action.clone()]),
            if assessed.is_applicable() {
                vec![action.clone(), action.clone()]
            } else {
                vec![]
            }
        );
        match world.apply(action.clone()) {
            Ok(receipt) => {
                assert!(
                    case.expected["error"].is_null(),
                    "case {index}: {}",
                    case.label
                );
                assert_eq!(receipt.exchange(), &action);
                let actual = json!({"proposal": proposal(receipt.exchange()), "deltas": deltas(receipt.deltas())});
                assert_eq!(
                    actual, case.expected["receipt"],
                    "case {index}: {}",
                    case.label
                );
                assert_eq!(
                    deltas(receipt.deltas()),
                    case.expected["assessment"]["deltas"]
                );
            }
            Err(error) => {
                assert!(
                    case.expected["receipt"].is_null(),
                    "case {index}: {}",
                    case.label
                );
                assert_eq!(
                    failure(&error),
                    case.expected["error"],
                    "case {index}: {}",
                    case.label
                );
                assert_eq!(
                    serde_json::to_value(&world).unwrap(),
                    before,
                    "rejected exchange mutated state: {case:?}"
                );
            }
        }
        assert_eq!(
            snapshot(&world),
            case.expected["after"],
            "case {index}: {}",
            case.label
        );
        let after = serde_json::to_value(&world).unwrap();
        assert_eq!(after["rates"], before["rates"]);
        assert_eq!(after["invariants"], before["invariants"]);
    }
}

#[test]
fn complete_replays_match_lean() {
    let fixtures: Fixtures =
        serde_json::from_str(include_str!("../verification/conformance.json")).unwrap();
    assert_eq!(fixtures.executions.len(), 246);
    for (index, case) in fixtures.executions.iter().enumerate() {
        assert_ne!(
            case.expected.get("kind"),
            Some(&json!("panic")),
            "Lean replay panicked: {case:?}"
        );
        let (source, _) = build(&case.base);
        let before = serde_json::to_value(&source).unwrap();
        let mut trace = Trace::default();
        let actions = case.trace.iter().map(build_proposal).collect::<Vec<_>>();
        for action in &actions {
            trace.push(action.clone());
        }
        assert_eq!(trace.exchanges(), actions.as_slice());
        assert_eq!(trace.clone().into_exchanges(), actions);
        let mut branch = source.fork();
        assert_eq!(serde_json::to_value(&branch).unwrap(), before);
        match branch.replay(&trace) {
            Ok(receipts) => {
                assert!(case.expected["error"].is_null(), "case {index}: {case:?}");
                assert_eq!(
                    json!(receipts.iter().map(receipt).collect::<Vec<_>>()),
                    case.expected["receipts"],
                    "case {index}: {case:?}"
                );
            }
            Err(error) => {
                assert!(
                    case.expected["receipts"].is_null(),
                    "case {index}: {case:?}"
                );
                assert_eq!(
                    failure(&error),
                    case.expected["error"],
                    "case {index}: {case:?}"
                );
            }
        }
        assert_eq!(
            snapshot(&branch),
            case.expected["after"],
            "case {index}: {case:?}"
        );
        let after = serde_json::to_value(&branch).unwrap();
        assert_eq!(after["rates"], before["rates"]);
        assert_eq!(after["invariants"], before["invariants"]);
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
        match source.replayed(&trace) {
            Ok(after) => {
                assert!(case.expected["error"].is_null(), "case {index}: {case:?}");
                assert_eq!(
                    snapshot(&after),
                    case.expected["after"],
                    "case {index}: {case:?}"
                );
            }
            Err(error) => assert_eq!(
                failure(&error),
                case.expected["error"],
                "case {index}: {case:?}"
            ),
        }
        assert_eq!(
            serde_json::to_value(&source).unwrap(),
            before,
            "isolated replay mutated source: {case:?}"
        );
    }
}
