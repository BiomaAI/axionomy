use axionomy::{Economy, LinearInvariant};
use serde::Deserialize;
use serde_json::json;

// The loader must not acquire Clone or Ord requirements for asset identities.
#[derive(Debug, PartialEq, Eq, Hash, Deserialize)]
struct Asset(u8);

#[derive(Deserialize)]
struct Case {
    entries: Vec<(u8, String)>,
    expected: Option<Vec<(u8, String)>>,
}

#[test]
fn serialized_invariant_weights_match_lean_validation_and_normalization() {
    let fixtures: serde_json::Value = serde_json::from_str(crate::FIXTURES).unwrap();
    let cases: Vec<Case> = serde_json::from_value(fixtures["invariantInputs"].clone()).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let entries: Vec<_> = case
            .entries
            .iter()
            .map(|(asset, weight)| (*asset, weight.parse::<i64>().unwrap()))
            .collect();
        let input = json!({"name": "material éλ", "weights": entries});
        let loaded = serde_json::from_value::<LinearInvariant<Asset>>(input);
        match case.expected {
            None => assert!(loaded.is_err(), "duplicate accepted: {:?}", case.entries),
            Some(expected) => {
                let invariant = loaded.unwrap();
                assert_eq!(invariant.name(), "material éλ");
                let actual: Vec<_> = invariant
                    .weights()
                    .map(|(asset, weight)| (asset.0, weight.to_string()))
                    .collect();
                assert_eq!(actual, expected, "input: {:?}", case.entries);
            }
        }
    }
}

#[test]
fn economy_loading_rejects_duplicate_invariant_assets_even_after_zero() {
    for weights in [
        json!([[0, 0], [0, 1]]),
        json!([[0, 0], [0, 0]]),
        json!([[0, -1], [0, 0]]),
    ] {
        let model = json!({
            "accounts": [], "rates": [],
            "invariants": [{"name": "material", "weights": weights}],
        });
        assert!(serde_json::from_value::<Economy<Bytes>>(model).is_err());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Bytes {}

impl axionomy::Ontology for Bytes {
    const NAME: &'static str = "bytes";
    type Account = u8;
    type Asset = u8;
    type Rate = u8;
    type Role = u8;
}
