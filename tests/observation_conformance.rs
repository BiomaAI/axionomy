//! Regression bridge from the proved observation/key specifications to public Rust APIs.
use axionomy::{Account, Basket, Economy, EconomyBuilder, Goal, Quantity};
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
type Amounts = Vec<(Asset, String)>;

#[derive(Deserialize)]
struct Case {
    accounts: Vec<(Holder, Amounts)>,
    requested: Vec<Holder>,
    goal: Vec<(Holder, Amounts)>,
    expected: Value,
}

#[derive(Deserialize)]
struct Fixtures {
    observations: Vec<Case>,
}

fn fixtures() -> Vec<Case> {
    serde_json::from_str::<Fixtures>(include_str!("../verification/conformance.json"))
        .expect("Lean-generated fixture document")
        .observations
}

fn basket(entries: &Amounts) -> Basket<Asset> {
    Basket::try_from_entries(entries.iter().map(|(asset, value)| {
        (
            *asset,
            Quantity::new(value.parse::<u64>().expect("bounded Lean quantity")),
        )
    }))
    .expect("distinct asset identifiers")
}

fn world(case: &Case) -> World {
    case.accounts
        .iter()
        .fold(EconomyBuilder::new(), |builder, (id, entries)| {
            builder.account(*id, Account::from(basket(entries)))
        })
        .build()
        .expect("distinct account identifiers")
}

fn triples(entries: &[(Holder, Asset, Quantity)]) -> Value {
    json!(
        entries
            .iter()
            .map(|(id, asset, value)| (*id, *asset, value.to_string()))
            .collect::<Vec<_>>()
    )
}

#[test]
fn observations_and_goals_match_lean() {
    let cases = fixtures();
    assert_eq!(cases.len(), 504);
    for (index, case) in cases.iter().enumerate() {
        let model = world(case);
        let goal = case.goal.iter().fold(Goal::new(), |goal, (id, entries)| {
            goal.require(*id, basket(entries))
        });
        let view = model.view(case.requested.iter().copied());
        let key = view.observation_key();
        let queries = case.expected["queries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|query| {
                let id = Holder(query["account"].as_u64().unwrap());
                let asset = Asset(query["asset"].as_u64().unwrap());
                assert_eq!(
                    view.account(&id).is_some(),
                    key.existing_accounts().contains(&id),
                    "case {index}"
                );
                json!({
                    "account": id,
                    "asset": asset,
                    "balance": model.balance(&id, &asset).to_string(),
                    "observed": view.balance(&id, &asset).map(|value| value.to_string()),
                })
            })
            .collect::<Vec<_>>();
        let requirements = goal
            .requirements()
            .iter()
            .map(|(id, amounts)| {
                (
                    *id,
                    amounts
                        .iter()
                        .map(|(asset, value)| (*asset, value.to_string()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let actual = json!({
            "matches": model.matches(&goal),
            "requirements": requirements,
            "goalAssets": goal.asset_keys().into_iter().copied().collect::<Vec<_>>(),
            "visible": key.visible_accounts(),
            "existing": key.existing_accounts(),
            "observationKey": triples(key.balances()),
            "stateKey": triples(&model.state_key()),
            "queries": queries,
        });
        let mut expected = case.expected.clone();
        expected.as_object_mut().unwrap().remove("fingerprint");
        assert_eq!(actual, expected, "observation case {index}");
    }
}

// Rust's generic Hash stream uses native-endian usize/u64 writes here. This is
// the explicit architecture scope of the current reviewed fingerprint adapter.
#[cfg(all(target_pointer_width = "64", target_endian = "little"))]
#[test]
fn fingerprints_match_lean_on_native_64_little_endian() {
    for (index, case) in fixtures().iter().enumerate() {
        assert_eq!(
            json!(world(case).state_fingerprint().get().to_string()),
            case.expected["fingerprint"],
            "fingerprint case {index}"
        );
    }
}
