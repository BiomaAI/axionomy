//! Rates with variables, replayed from the executable Lean specification's
//! fixtures: economies whose rates are written as the kernel writes a
//! `paths` economy's, exchanges naming those rates in full, and what each
//! becomes; and rates that cannot make an economy, with why.

use crate::transactions::{assessment, failure, receipt, snapshot};
use axionomy::{
    Account, AccountPath, ApplyError, AssetPath, Basket, Economy, EconomyBuilder, Exchange,
    ModelIssue, Pattern, Quantity, Rate, RatePath,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// Paths for every identifier but roles, numbers as the fixtures write them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Mixed {}

impl axionomy::Ontology for Mixed {
    const NAME: &'static str = "mixed";
    type Account = AccountPath;
    type Asset = AssetPath;
    type Rate = RatePath;
    type Role = u64;
}

type Amounts = Vec<(AssetPath, String)>;

#[derive(Debug, Deserialize)]
struct ProposalCase {
    rate: RatePath,
    units: String,
    bindings: Vec<(u64, AccountPath)>,
    parameters: Vec<(axionomy::ParameterPath, String)>,
}

#[derive(Debug, Deserialize)]
struct Case {
    label: String,
    accounts: Vec<(AccountPath, Amounts)>,
    rates: Vec<(String, Value)>,
    proposal: ProposalCase,
    expected: Value,
}

#[derive(Debug, Deserialize)]
struct ModelCase {
    label: String,
    rates: Vec<(String, Value)>,
    issues: Vec<Value>,
}

#[derive(Deserialize)]
struct Fixtures {
    #[serde(rename = "parametricTransactions")]
    transactions: Vec<Case>,
    #[serde(rename = "parametricModels")]
    models: Vec<ModelCase>,
}

fn fixtures() -> Fixtures {
    serde_json::from_str(crate::FIXTURES).unwrap()
}

/// Each rate as the kernel reads one written down: its identifier, which may
/// name variables, and the rate in its JSON form.
fn rates(builder: EconomyBuilder<Mixed>, rates: &[(String, Value)]) -> EconomyBuilder<Mixed> {
    rates.iter().fold(builder, |builder, (key, rate)| {
        let key: Pattern<RatePath> = key.parse().unwrap();
        let rate: Rate<Mixed> =
            serde_json::from_value(rate.clone()).unwrap_or_else(|error| panic!("{key}: {error}"));
        builder.rate_at(key, rate)
    })
}

fn economy(case: &Case) -> Economy<Mixed> {
    let mut builder = rates(EconomyBuilder::new(), &case.rates);
    for (id, entries) in &case.accounts {
        let basket = Basket::try_from_entries(
            entries
                .iter()
                .map(|(asset, value)| (asset.clone(), value.parse::<Quantity>().unwrap())),
        )
        .unwrap();
        builder = builder.account(id.clone(), Account::new(basket));
    }
    builder.build().unwrap()
}

fn proposal(case: &ProposalCase) -> Exchange<Mixed> {
    let mut exchange = Exchange::new(case.rate.clone(), case.units.parse::<Quantity>().unwrap());
    for (role, account) in &case.bindings {
        exchange = exchange.bind(*role, account.clone());
    }
    for (name, value) in &case.parameters {
        exchange = exchange.parameter(name.clone(), value.parse::<Quantity>().unwrap());
    }
    exchange
}

#[test]
fn rates_with_variables_apply_their_instances_as_lean_applies_them() {
    let cases = fixtures().transactions;
    assert!(!cases.is_empty());
    for case in &cases {
        assert_ne!(
            case.expected.get("kind"),
            Some(&json!("panic")),
            "{}",
            case.label
        );
        let world = economy(case);
        let exchange = proposal(&case.proposal);
        let before = serde_json::to_value(&world).unwrap();
        assert_eq!(
            assessment(&world.assess(&exchange)),
            case.expected["assessment"],
            "{}: assessment",
            case.label
        );
        let mut branch = world.clone();
        match branch.apply(exchange) {
            Ok(applied) => {
                assert!(case.expected["error"].is_null(), "{}", case.label);
                assert_eq!(
                    receipt(&applied),
                    case.expected["receipt"],
                    "{}: receipt",
                    case.label
                );
                assert_eq!(
                    snapshot(&branch),
                    case.expected["after"],
                    "{}: after",
                    case.label
                );
            }
            Err(error) => {
                assert_eq!(
                    failure(&error),
                    case.expected["error"],
                    "{}: error",
                    case.label
                );
                assert_eq!(
                    snapshot(&branch),
                    case.expected["after"],
                    "{}: after",
                    case.label
                );
            }
        }
        assert_eq!(
            serde_json::to_value(&world).unwrap(),
            before,
            "{}",
            case.label
        );
    }
}

#[test]
fn rates_that_cannot_make_an_economy_are_refused_as_lean_refuses_them() {
    let cases = fixtures().models;
    assert!(!cases.is_empty());
    for case in &cases {
        let built = rates(EconomyBuilder::new(), &case.rates).build();
        let issues: Vec<Value> = match built {
            Ok(_) => Vec::new(),
            Err(error) => error.issues().iter().map(issue).collect(),
        };
        assert_eq!(issues, case.issues, "{}", case.label);
    }
}

fn issue(issue: &ModelIssue<Mixed>) -> Value {
    let named = |kind: &str, rate: &Pattern<RatePath>, variable: &axionomy::VariablePath| json!({"kind": kind, "rate": rate.text(), "variable": variable});
    match issue {
        ModelIssue::RepeatedVariable { rate, variable } => {
            named("repeated_variable", rate, variable)
        }
        ModelIssue::UndeclaredVariable { rate, variable } => {
            named("undeclared_variable", rate, variable)
        }
        ModelIssue::UnreadVariable { rate, variable } => named("unread_variable", rate, variable),
        ModelIssue::NotNatural { rate, variable } => named("not_natural", rate, variable),
        ModelIssue::RepeatedValue { rate, variable } => named("repeated_value", rate, variable),
        ModelIssue::OverlappingRates { rate, other } => {
            json!({"kind": "overlapping_rates", "rate": rate.text(), "other": other.text()})
        }
        ModelIssue::MalformedRelation { rate, variables } => {
            json!({"kind": "malformed_relation", "rate": rate.text(), "variables": variables})
        }
        ModelIssue::RepeatedRow { rate, variables } => {
            json!({"kind": "repeated_row", "rate": rate.text(), "variables": variables})
        }
        ModelIssue::DerivedReads { rate, variable } => named("derived_reads", rate, variable),
        ModelIssue::DuplicateAccount { .. } | ModelIssue::DuplicateRate { .. } => {
            panic!("no fixture names an identifier twice: {issue:?}")
        }
    }
}

/// A rate written down reads back the same, in JSON and in the store's
/// binary form, and a rate with no variables is written as it always was.
#[test]
fn a_rate_with_variables_is_written_down_and_read_back() {
    for case in fixtures().transactions.iter().take(1) {
        for (key, written) in &case.rates {
            let rate: Rate<Mixed> = serde_json::from_value(written.clone()).unwrap();
            assert_eq!(&serde_json::to_value(&rate).unwrap(), written, "{key}");
            let bytes = postcard::to_allocvec(&rate).unwrap();
            let read: Rate<Mixed> = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(serde_json::to_value(&read).unwrap(), *written, "{key}");
        }
    }
    let error: ApplyError<Mixed> = ApplyError::ZeroUnits;
    assert_eq!(failure(&error), json!({"kind": "zero_units"}));
}
