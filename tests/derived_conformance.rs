//! Derived public traits against Lean-generated structural and encoding contracts.
use axionomy::{
    Account, AccountError, ApplyError, AssessmentStatus, AssetAmount, Basket, BasketError, Economy,
    EconomyBuilder, Exchange, ExchangeAssessment, Goal, LinearInvariant, ModelIssue,
    ObservationKey, Quantity, QuantityComparison, QuantityError, QuantityExpression, Rate,
    RateError, StateFingerprint, Trace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        struct $name(u64);
    };
}
identifier!(Asset);
identifier!(Role);
identifier!(Holder);
identifier!(RuleId);

type Expression = QuantityExpression<Role, Asset>;
type Proposal = Exchange<RuleId, Role, Holder>;

fn fixtures() -> Value {
    serde_json::from_str::<Value>(include_str!("../verification/conformance.json"))
        .unwrap()["derived"]
        .clone()
}
fn items(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
fn number(value: &Value) -> u64 {
    value.as_u64().unwrap()
}
fn scalar(value: &Value) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}
fn cloned<T: Clone>(value: &T) -> T {
    value.clone()
}

fn check_serialized_clone<T: Clone + Serialize>(value: &T) {
    // Observable regression evidence for types without PartialEq. Serialization
    // correctness is a separate contract; Lean proves preservation of the fields.
    assert_eq!(
        serde_json::to_string(&cloned(value)).unwrap(),
        serde_json::to_string(value).unwrap()
    );
}
fn check_equal_clone_from<T: Clone + PartialEq + std::fmt::Debug>(target: &T, source: &T) {
    let mut result = target.clone();
    result.clone_from(source);
    assert_eq!(&result, source);
}
fn check_clone_from<T: Clone + Serialize>(target: &T, source: &T) {
    let mut result = target.clone();
    result.clone_from(source);
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(source).unwrap()
    );
}
fn expression(value: &Value) -> Expression {
    match value["kind"].as_str().unwrap() {
        "constant" => Expression::constant(scalar(&value["value"])),
        "units" => Expression::units(),
        "parameter" => Expression::parameter(value["name"].as_str().unwrap()),
        "balance" => {
            Expression::balance(Role(number(&value["role"])), Asset(number(&value["asset"])))
        }
        kind => {
            let left = expression(&value["left"]);
            let right = expression(&value["right"]);
            match kind {
                "add" => Expression::plus(left, right),
                "subtract" => Expression::subtract(left, right),
                "multiply" => Expression::multiply(left, right),
                "divide_floor" => Expression::divide_floor(left, right),
                "minimum" => Expression::minimum(left, right),
                "maximum" => Expression::maximum(left, right),
                _ => panic!("unknown expression {kind}"),
            }
        }
    }
}
fn proposal(value: &Value) -> Proposal {
    let mut result = Proposal::new(
        RuleId(number(&value["rate"])),
        Quantity::new(scalar(&value["units"])),
    );
    for binding in items(&value["bindings"]) {
        result = result.bind(Role(number(&binding[0])), Holder(number(&binding[1])));
    }
    for parameter in items(&value["parameters"]) {
        result = result.parameter(
            parameter[0].as_str().unwrap(),
            Quantity::new(scalar(&parameter[1])),
        );
    }
    result
}
fn amounts(value: &Value) -> Basket<Asset> {
    Basket::try_from_entries(
        items(value)
            .iter()
            .map(|entry| (Asset(number(&entry[0])), Quantity::new(scalar(&entry[1])))),
    )
    .unwrap()
}
fn check_pairs<T: PartialEq + Clone + std::fmt::Debug>(values: &[T], pairs: &Value) {
    for pair in items(pairs) {
        let left = usize::try_from(number(&pair[0])).unwrap();
        let right = usize::try_from(number(&pair[1][0])).unwrap();
        assert_eq!(
            values[left] == values[right],
            pair[1][1].as_bool().unwrap(),
            "pair {left}, {right}"
        );
        assert_eq!(
            values[left] != values[right],
            !pair[1][1].as_bool().unwrap()
        );
        let mut target = values[left].clone();
        target.clone_from(&values[right]);
        assert_eq!(target, values[right]);
    }
}

#[derive(Default)]
struct Bytes(Vec<u8>);
impl Hasher for Bytes {
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    fn finish(&self) -> u64 {
        self.0
            .iter()
            .fold(14_695_981_039_346_656_037, |state, byte| {
                (state ^ u64::from(*byte)).wrapping_mul(1_099_511_628_211)
            })
    }
}
fn hashed(value: &impl Hash) -> Bytes {
    let mut result = Bytes::default();
    value.hash(&mut result);
    result
}

#[test]
fn recursive_expression_equality_and_clone_match_lean() {
    let data = fixtures();
    let values = items(&data["expressions"])
        .iter()
        .map(expression)
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 30);
    assert_eq!(items(&data["expressionPairs"]).len(), 900);
    check_pairs(&values, &data["expressionPairs"]);
    let computed = values
        .iter()
        .map(|value| {
            let rule = Rate::new().consume_computed(Role(0), Asset(0), cloned(value));
            cloned(&rule.computed_consumed()[0])
        })
        .collect::<Vec<_>>();
    let conditions = values
        .iter()
        .map(|value| {
            let rule = Rate::new().condition(
                "β",
                cloned(value),
                QuantityComparison::Equal,
                Expression::units(),
            );
            cloned(&rule.conditions()[0])
        })
        .collect::<Vec<_>>();
    check_pairs(&computed, &data["expressionPairs"]);
    check_pairs(&conditions, &data["expressionPairs"]);
    for value in values {
        assert_eq!(cloned(&value), value);
    }
}

#[test]
fn proposals_and_ordered_traces_match_lean() {
    let data = fixtures();
    let proposals = items(&data["proposals"])
        .iter()
        .map(proposal)
        .collect::<Vec<_>>();
    assert_eq!(items(&data["proposalPairs"]).len(), 100);
    check_pairs(&proposals, &data["proposalPairs"]);
    for value in &proposals {
        assert_eq!(&cloned(value), value);
    }
    let traces = items(&data["traces"])
        .iter()
        .map(|indices| {
            let mut trace = Trace::new();
            trace.extend(
                items(indices)
                    .iter()
                    .map(|index| cloned(&proposals[usize::try_from(number(index)).unwrap()])),
            );
            trace
        })
        .collect::<Vec<_>>();
    assert_eq!(items(&data["tracePairs"]).len(), 64);
    check_pairs(&traces, &data["tracePairs"]);
    for value in traces {
        assert_eq!(cloned(&value), value);
    }
}

#[test]
fn key_equality_clone_and_scoped_hash_stream_match_lean() {
    let data = fixtures();
    let keys = items(&data["keys"])
        .iter()
        .map(|case| {
            // Deserialization only supplies public test inputs here. Its validation
            // and schema contract are a separate verification milestone.
            let key: ObservationKey<Holder, Asset> = serde_json::from_value(json!({
                "visible_accounts": case["visible"], "existing_accounts": case["existing"],
                "balances": items(&case["balances"]).iter().map(|entry| {
                    (number(&entry[0]), number(&entry[1][0]), scalar(&entry[1][1]))
                }).collect::<Vec<_>>()
            }))
            .unwrap();
            assert_eq!(cloned(&key), key);
            let bytes = hashed(&key);
            assert_eq!(json!(bytes.0), case["bytes"]);
            assert_eq!(bytes.finish(), scalar(&case["hash"]));
            key
        })
        .collect::<Vec<_>>();
    assert_eq!(items(&data["keyPairs"]).len(), 121);
    check_pairs(&keys, &data["keyPairs"]);
}

#[test]
fn quantities_amounts_and_fingerprints_match_lean() {
    let data = fixtures();
    assert_eq!(items(&data["numbers"]).len(), 36);
    for case in items(&data["numbers"]) {
        let left = Quantity::new(scalar(&case["left"]));
        let right = Quantity::new(scalar(&case["right"]));
        let expected = case["eq"].as_bool().unwrap();
        assert_eq!(left == right, expected);
        assert_eq!(left != right, !expected);
        check_clone_from(&left, &right);
        assert_eq!(cloned(&left), left);
        let ordering = case["cmp"].as_i64().unwrap().cmp(&0);
        assert_eq!(left.cmp(&right), ordering);
        assert_eq!(left.partial_cmp(&right), Some(ordering));
        assert_eq!(
            json!([
                left.lt(&right),
                left.le(&right),
                left.gt(&right),
                left.ge(&right)
            ]),
            case["predicates"]
        );
        assert_eq!(json!(hashed(&left).0), case["bytes"]);
        let fingerprint: StateFingerprint = serde_json::from_value(json!(left.get())).unwrap();
        let other: StateFingerprint = serde_json::from_value(json!(right.get())).unwrap();
        assert_eq!(fingerprint == other, expected);
        assert_eq!(fingerprint != other, !expected);
        check_clone_from(&fingerprint, &other);
        assert_eq!(cloned(&fingerprint), fingerprint);
        assert_eq!(fingerprint.cmp(&other), ordering);
        assert_eq!(fingerprint.partial_cmp(&other), Some(ordering));
        assert_eq!(
            json!([
                fingerprint.lt(&other),
                fingerprint.le(&other),
                fingerprint.gt(&other),
                fingerprint.ge(&other)
            ]),
            case["predicates"]
        );
        assert_eq!(json!(hashed(&fingerprint).0), case["bytes"]);
        let amount = AssetAmount::new(Asset(7), left);
        let same_asset = AssetAmount::new(Asset(7), right);
        let other_asset = AssetAmount::new(Asset(8), right);
        assert_eq!(amount == same_asset, expected);
        assert_eq!(amount != same_asset, !expected);
        check_clone_from(&amount, &other_asset);
        assert_ne!(amount, other_asset);
        assert_eq!(cloned(&amount), amount);
        let mut expected_bytes = hashed(&Asset(7)).0;
        expected_bytes.extend_from_slice(&hashed(&left).0);
        assert_eq!(hashed(&amount).0, expected_bytes);
    }
}

#[test]
fn basket_account_and_account_error_equality_match_lean() {
    let data = fixtures();
    let baskets = items(&data["baskets"])
        .iter()
        .map(amounts)
        .collect::<Vec<_>>();
    let accounts = baskets
        .iter()
        .map(|basket| Account::new(cloned(basket)))
        .collect::<Vec<_>>();
    let errors = baskets
        .iter()
        .map(|basket| AccountError::InsufficientBalance {
            shortfall: cloned(basket),
        })
        .collect::<Vec<_>>();
    assert_eq!(items(&data["basketPairs"]).len(), 49);
    check_pairs(&baskets, &data["basketPairs"]);
    check_pairs(&accounts, &data["basketPairs"]);
    check_pairs(&errors, &data["basketPairs"]);
    for ((basket, account), error) in baskets.iter().zip(&accounts).zip(&errors) {
        assert_eq!(&cloned(basket), basket);
        assert_eq!(&cloned(account), account);
        assert_eq!(&cloned(error), error);
        let overflow = AccountError::Overflow { asset: Asset(7) };
        assert_ne!(error, &overflow);
        assert_eq!(cloned(&overflow), overflow);
        assert_ne!(overflow, AccountError::Overflow { asset: Asset(8) });
    }
}

#[test]
fn record_error_and_model_clone_paths_preserve_all_fields() {
    type World = Economy<Holder, Asset, RuleId, Role>;
    type Failure = ApplyError<RuleId, Role, Holder, Asset>;
    let data = fixtures();
    for case in items(&data["numbers"]) {
        let left = scalar(&case["left"]);
        let right = scalar(&case["right"]);
        let equal = case["eq"].as_bool().unwrap();
        let basket_error = BasketError::DuplicateAsset { asset: Asset(left) };
        assert_eq!(cloned(&basket_error), basket_error);
        let other_basket_error = BasketError::DuplicateAsset {
            asset: Asset(right),
        };
        assert_eq!(basket_error != other_basket_error, !equal);
        check_equal_clone_from(&basket_error, &other_basket_error);
        assert_eq!(
            basket_error
                == BasketError::DuplicateAsset {
                    asset: Asset(right)
                },
            equal
        );
        let rate_error = RateError::BasketOverflow {
            role: Role(left),
            asset: Asset(right),
        };
        assert_eq!(cloned(&rate_error), rate_error);
        let other_rate_error = RateError::BasketOverflow {
            role: Role(right),
            asset: Asset(left),
        };
        assert_eq!(rate_error != other_rate_error, !equal);
        check_equal_clone_from(&rate_error, &other_rate_error);
        assert_eq!(
            rate_error
                == RateError::BasketOverflow {
                    role: Role(right),
                    asset: Asset(right)
                },
            equal
        );
        assert_eq!(
            rate_error
                == RateError::BasketOverflow {
                    role: Role(left),
                    asset: Asset(left)
                },
            equal
        );
        let issue: ModelIssue<Holder, RuleId> = ModelIssue::DuplicateAccount {
            account: Holder(left),
        };
        assert_eq!(cloned(&issue), issue);
        assert_eq!(
            issue
                == ModelIssue::DuplicateAccount {
                    account: Holder(right)
                },
            equal
        );
        assert_ne!(issue, ModelIssue::DuplicateRate { rate: RuleId(left) });
        check_clone_from(
            &issue,
            &ModelIssue::DuplicateRate {
                rate: RuleId(right),
            },
        );
        let issue: ModelIssue<Holder, RuleId> = ModelIssue::DuplicateRate { rate: RuleId(left) };
        assert_eq!(cloned(&issue), issue);
        assert_eq!(
            issue
                == ModelIssue::DuplicateRate {
                    rate: RuleId(right)
                },
            equal
        );
    }
    let comparisons = [
        QuantityComparison::Equal,
        QuantityComparison::GreaterThanOrEqual,
        QuantityComparison::LessThanOrEqual,
    ];
    let statuses = [
        AssessmentStatus::Applicable,
        AssessmentStatus::Infeasible,
        AssessmentStatus::Invalid,
    ];
    for (i, left) in comparisons.iter().enumerate() {
        assert_eq!(cloned(left), *left);
        for (j, right) in comparisons.iter().enumerate() {
            assert_eq!(left == right, i == j);
            assert_eq!(left != right, i != j);
            check_clone_from(left, right);
        }
    }
    for (i, left) in statuses.iter().enumerate() {
        assert_eq!(cloned(left), *left);
        for (j, right) in statuses.iter().enumerate() {
            assert_eq!(left == right, i == j);
            assert_eq!(left != right, i != j);
            check_clone_from(left, right);
        }
    }
    assert_eq!(cloned(&QuantityError::Negative), QuantityError::Negative);
    assert!(!QuantityError::Negative.ne(&QuantityError::Negative));
    check_equal_clone_from(&QuantityError::Negative, &QuantityError::Negative);
    let full = amounts(&data["baskets"][4]);
    let requirement = amounts(&data["baskets"][1]);
    let rule = Rate::new()
        .consume(Role(0), requirement.clone())
        .produce(Role(1), requirement.clone())
        .preserve(Role(0), amounts(&data["baskets"][2]))
        .distinct(Role(0), Role(1))
        .consume_computed(Role(0), Asset(0), Expression::constant(0))
        .produce_computed(Role(1), Asset(0), Expression::constant(0))
        .preserve_computed(Role(0), Asset(1), Expression::constant(0))
        .condition(
            "β",
            Expression::balance(Role(0), Asset(0)),
            QuantityComparison::GreaterThanOrEqual,
            Expression::units(),
        );
    check_serialized_clone(&rule);
    check_clone_from(&Rate::new(), &rule);
    for amount in rule
        .computed_consumed()
        .iter()
        .chain(rule.computed_produced())
        .chain(rule.computed_preserved())
    {
        assert_eq!(&cloned(amount), amount);
    }
    for condition in rule.conditions() {
        assert_eq!(&cloned(condition), condition);
    }
    let invariant = LinearInvariant::new("β")
        .weight(Asset(0), -3)
        .weight(Asset(1), 7);
    let copy = cloned(&invariant);
    check_clone_from(&LinearInvariant::new("old"), &invariant);
    assert_eq!(copy.name(), invariant.name());
    assert_eq!(
        copy.weights().collect::<Vec<_>>(),
        invariant.weights().collect::<Vec<_>>()
    );
    let goal = Goal::new()
        .require(Holder(0), full.clone())
        .require(Holder(1), Basket::new());
    assert_eq!(cloned(&goal).requirements(), goal.requirements());
    check_clone_from(&Goal::new(), &goal);
    let mut world: World = EconomyBuilder::new()
        .account(Holder(0), Account::new(full))
        .account(Holder(1), Account::default())
        .rate(RuleId(0), rule.clone())
        .build()
        .unwrap();
    check_serialized_clone(&world);
    check_clone_from(&EconomyBuilder::new().build().unwrap(), &world);
    let submitted = Proposal::new(RuleId(0), Quantity::new(1))
        .bind(Role(0), Holder(0))
        .bind(Role(1), Holder(1));
    let assessment = world.assess(&submitted);
    assert_eq!(assessment.status(), AssessmentStatus::Applicable);
    check_serialized_clone(&assessment);
    for account in assessment.accounts() {
        check_serialized_clone(account);
        check_clone_from(&assessment.accounts()[0], account);
    }
    for delta in assessment.projected_deltas().unwrap() {
        check_serialized_clone(delta);
        check_clone_from(&assessment.projected_deltas().unwrap()[0], delta);
    }
    let receipt = world.apply(submitted).unwrap();
    check_serialized_clone(&receipt);
    let mut empty_world: World = EconomyBuilder::new()
        .rate(RuleId(7), Rate::new())
        .build()
        .unwrap();
    let empty_receipt = empty_world
        .apply(Proposal::new(RuleId(7), Quantity::new(2)))
        .unwrap();
    check_clone_from(&empty_receipt, &receipt);
    for delta in receipt.deltas() {
        check_serialized_clone(delta);
    }
    let poor: World = EconomyBuilder::new()
        .account(Holder(0), Account::default())
        .rate(RuleId(0), Rate::new().consume(Role(0), requirement))
        .build()
        .unwrap();
    let infeasible =
        poor.assess(&Proposal::new(RuleId(0), Quantity::new(1)).bind(Role(0), Holder(0)));
    assert_eq!(infeasible.status(), AssessmentStatus::Infeasible);
    check_serialized_clone(&infeasible);
    check_clone_from(&assessment, &infeasible);
    for shortfall in infeasible.shortfalls() {
        check_serialized_clone(shortfall);
        let larger =
            poor.assess(&Proposal::new(RuleId(0), Quantity::new(2)).bind(Role(0), Holder(0)));
        check_clone_from(&larger.shortfalls()[0], shortfall);
    }
    let failures: Vec<Failure> = vec![
        Failure::MissingRate { rate: RuleId(7) },
        Failure::MissingBinding { role: Role(8) },
        Failure::UnknownBinding { role: Role(9) },
        Failure::RolesMustDiffer {
            left: Role(3),
            right: Role(4),
        },
        Failure::MissingAccount { account: Holder(5) },
        Failure::ZeroUnits,
        Failure::MissingParameter { name: "β".into() },
        Failure::UnknownParameter { name: "γ".into() },
        Failure::RateOverflow {
            rate: RuleId(6),
            asset: Asset(7),
        },
        Failure::RateLawEvaluation {
            rate: RuleId(8),
            message: "β".into(),
        },
        Failure::RateConditionFailed {
            rate: RuleId(9),
            condition: "γ".into(),
        },
        Failure::Infeasible {
            shortfalls: infeasible.shortfalls().to_vec(),
        },
        Failure::BalanceOverflow {
            account: Holder(10),
            asset: Asset(11),
        },
        Failure::InvariantOverflow {
            invariant: "β".into(),
        },
        Failure::InvariantViolation {
            invariant: "γ".into(),
            before: i128::MIN,
            after: i128::MAX,
        },
    ];
    for failure in &failures {
        check_serialized_clone(failure);
        check_clone_from(&Failure::ZeroUnits, failure);
    }
    let invalid: ExchangeAssessment<Holder, Asset, RuleId, Role> =
        ExchangeAssessment::Invalid { issues: failures };
    check_serialized_clone(&invalid);
    let bad: Result<World, _> = EconomyBuilder::new()
        .account(Holder(0), Account::default())
        .account(Holder(0), Account::default())
        .rate(RuleId(0), rule.clone())
        .rate(RuleId(0), rule)
        .build();
    let error = bad.unwrap_err();
    assert_eq!(cloned(&error), error);
    assert!(!error.ne(&error));
    let other_error = EconomyBuilder::<Holder, Asset, RuleId, Role>::new()
        .account(Holder(9), Account::default())
        .account(Holder(9), Account::default())
        .build()
        .unwrap_err();
    assert!(error.ne(&other_error));
    check_clone_from(&other_error, &error);
    assert_eq!(error.issues().len(), 2);
}

#[cfg(feature = "bigint")]
#[test]
fn generic_non_copy_numeric_payloads_preserve_values() {
    use num_bigint::BigUint;
    let number =
        Quantity::try_from_scalar((BigUint::from(1u64) << 192) + BigUint::from(17u64)).unwrap();
    assert_eq!(cloned(&number), number);
    let amount = AssetAmount::new(Asset(1), number.clone());
    assert_eq!(cloned(&amount), amount);
    let expression: QuantityExpression<Role, Asset, BigUint> = QuantityExpression::Constant {
        value: number.clone(),
    };
    assert_eq!(cloned(&expression), expression);
    let basket = Basket::try_from_entries([(Asset(1), number.clone())]).unwrap();
    assert_eq!(cloned(&basket), basket);
    let account = Account::new(basket.clone());
    assert_eq!(cloned(&account), account);
    let error = AccountError::InsufficientBalance { shortfall: basket };
    assert_eq!(cloned(&error), error);
    let proposal: Exchange<RuleId, Role, Holder, BigUint> =
        Exchange::new(RuleId(1), number).bind(Role(2), Holder(3));
    assert_eq!(cloned(&proposal), proposal);
    let mut trace = Trace::new();
    trace.push(proposal);
    assert_eq!(cloned(&trace), trace);
}
