//! Inherited helpers against Lean-generated order and hash traversal contracts.
use axionomy::{AssetAmount, ObservationKey, Quantity, QuantityScalar, StateFingerprint};
use num_traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::panic::catch_unwind;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../verification/conformance.json")).unwrap()
}
fn items(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
fn number(value: &Value) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}
fn fingerprint(value: u64) -> StateFingerprint {
    serde_json::from_value(json!(value)).unwrap()
}

#[test]
fn ordering_and_clamp_match_lean_including_invalid_ranges() {
    let data = fixtures();
    assert_eq!(items(&data["inherited"]["order"]).len(), 36);
    for case in items(&data["inherited"]["order"]) {
        let left = Quantity::new(number(&case["left"]));
        let right = Quantity::new(number(&case["right"]));
        assert_eq!(left.min(right).get(), number(&case["min"]));
        assert_eq!(left.max(right).get(), number(&case["max"]));
        assert_eq!(
            fingerprint(left.get()).min(fingerprint(right.get())).get(),
            number(&case["min"])
        );
        assert_eq!(
            fingerprint(left.get()).max(fingerprint(right.get())).get(),
            number(&case["max"])
        );
    }
    assert_eq!(items(&data["inherited"]["clamp"]).len(), 216);
    for case in items(&data["inherited"]["clamp"]) {
        let value = number(&case["value"]);
        let lower = number(&case["lower"]);
        let upper = number(&case["upper"]);
        let quantity = catch_unwind(|| {
            Quantity::new(value)
                .clamp(Quantity::new(lower), Quantity::new(upper))
                .get()
        });
        let key = catch_unwind(|| {
            fingerprint(value)
                .clamp(fingerprint(lower), fingerprint(upper))
                .get()
        });
        if case["result"].is_null() {
            assert!(quantity.is_err());
            assert!(key.is_err());
        } else {
            let expected = number(&case["result"]);
            assert_eq!(quantity.unwrap(), expected);
            assert_eq!(key.unwrap(), expected);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Asset(u64);
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Holder(u64);
#[derive(Default)]
struct Writes(Vec<Vec<u8>>);
impl Hasher for Writes {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.push(bytes.to_vec());
    }
}
fn chunks<T: Hash>(values: &[T]) -> Value {
    let mut state = Writes::default();
    T::hash_slice(values, &mut state);
    json!(state.0)
}

#[test]
fn slice_hashing_preserves_exact_write_chunks_without_outer_prefix() {
    let data = fixtures();
    let keys = items(&data["derived"]["keys"])
        .iter()
        .map(|case| {
            // Deserialization supplies inputs; this test does not certify that boundary.
            serde_json::from_value::<ObservationKey<Holder, Asset>>(json!({
            "visible_accounts": case["visible"], "existing_accounts": case["existing"],
            "balances": items(&case["balances"]).iter().map(|entry| {
                (entry[0].as_u64().unwrap(), entry[1][0].as_u64().unwrap(), number(&entry[1][1]))
            }).collect::<Vec<_>>()
        })).unwrap()
        })
        .collect::<Vec<_>>();
    for case in items(&data["inherited"]["hashSlices"]) {
        let quantities = items(&case["numbers"])
            .iter()
            .map(|v| Quantity::new(number(v)))
            .collect::<Vec<_>>();
        let amounts = quantities
            .iter()
            .map(|q| AssetAmount::new(Asset(7), *q))
            .collect::<Vec<_>>();
        let fingerprints = quantities
            .iter()
            .map(|q| fingerprint(q.get()))
            .collect::<Vec<_>>();
        let selected_keys = items(&case["indices"])
            .iter()
            .map(|i| keys[usize::try_from(i.as_u64().unwrap()).unwrap()].clone())
            .collect::<Vec<_>>();
        assert_eq!(chunks(&quantities), case["quantityChunks"]);
        assert_eq!(chunks(&fingerprints), case["quantityChunks"]);
        assert_eq!(chunks(&amounts), case["amountChunks"]);
        assert_eq!(chunks(&selected_keys), case["keyChunks"]);
    }
    // Native u64's own override writes one bulk chunk; Quantity<u64> inherits
    // the per-element default. Comparing chunks detects accidental dispatch.
    assert_eq!(items(&chunks(&[3_u64, 5])).len(), 1);
    assert_eq!(
        items(&chunks(&[Quantity::new(3), Quantity::new(5)])).len(),
        2
    );
}

#[derive(Debug)]
struct Payload(u64);
impl Clone for Payload {
    fn clone(&self) -> Self {
        Self(self.0)
    }
    fn clone_from(&mut self, _: &Self) {
        panic!("payload clone_from must not be called")
    }
}
#[allow(clippy::partialeq_ne_impl)]
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    fn ne(&self, _: &Self) -> bool {
        panic!("payload ne must not be called")
    }
}
impl Hash for Payload {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
    fn hash_slice<H: Hasher>(_: &[Self], _: &mut H) {
        panic!("payload hash_slice must not be called")
    }
}

#[test]
fn core_defaults_do_not_dispatch_to_payload_convenience_overrides() {
    let source = AssetAmount::new(Payload(8), Quantity::new(5));
    let mut target = AssetAmount::new(Payload(7), Quantity::new(3));
    assert!(target.ne(&source));
    target.clone_from(&source);
    assert_eq!(target, source);
    assert!(!target.ne(&source));
    assert_eq!(items(&chunks(&[target, source])).len(), 4);
}

// Tags distinguish values with the same numeric key. Rust still requires the
// full QuantityScalar interface; no private field or unchecked cast is used.
#[derive(Debug, Clone)]
struct Tagged {
    value: u64,
    tag: u8,
}
impl PartialEq for Tagged {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Tagged {}
impl Hash for Tagged {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.value.hash(h);
    }
}
impl std::fmt::Display for Tagged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}
impl Ord for Tagged {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
    fn min(self, _: Self) -> Self {
        panic!("payload min must not be called")
    }
    fn max(self, _: Self) -> Self {
        panic!("payload max must not be called")
    }
    fn clamp(self, _: Self, _: Self) -> Self {
        panic!("payload clamp must not be called")
    }
}
impl PartialOrd for Tagged {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
macro_rules! arithmetic {
    ($trait:ident, $op:ident, $checked_trait:ident, $checked:ident) => {
        impl std::ops::$trait for Tagged {
            type Output = Self;
            fn $op(self, other: Self) -> Self {
                self.$checked(&other).unwrap()
            }
        }
        impl $checked_trait for Tagged {
            fn $checked(&self, other: &Self) -> Option<Self> {
                self.value
                    .$checked(other.value)
                    .map(|value| Self { value, tag: 0 })
            }
        }
    };
}
arithmetic!(Add, add, CheckedAdd, checked_add);
arithmetic!(Sub, sub, CheckedSub, checked_sub);
arithmetic!(Mul, mul, CheckedMul, checked_mul);
arithmetic!(Div, div, CheckedDiv, checked_div);
impl Zero for Tagged {
    fn zero() -> Self {
        Self { value: 0, tag: 0 }
    }
    fn is_zero(&self) -> bool {
        self.value == 0
    }
}
impl QuantityScalar for Tagged {
    type SignedMeasure = i128;
    fn is_nonnegative(&self) -> bool {
        true
    }
    fn from_u64(value: u64) -> Option<Self> {
        Some(Self { value, tag: 0 })
    }
    fn checked_weighted(&self, coefficient: i64) -> Option<i128> {
        i128::from(self.value).checked_mul(i128::from(coefficient))
    }
}

#[test]
fn ties_retain_first_min_second_max_and_original_clamped_value() {
    let make = |value, tag| Quantity::try_from_scalar(Tagged { value, tag }).unwrap();
    let data = fixtures();
    for case in items(&data["inherited"]["taggedOrder"]) {
        let left = case["left"].as_u64().unwrap();
        let right = case["right"].as_u64().unwrap();
        assert_eq!(
            u64::from(make(left, 1).min(make(right, 2)).as_scalar().tag),
            case["minTag"].as_u64().unwrap()
        );
        assert_eq!(
            u64::from(make(left, 1).max(make(right, 2)).as_scalar().tag),
            case["maxTag"].as_u64().unwrap()
        );
    }
    for case in items(&data["inherited"]["taggedClamp"]) {
        let value = case["value"].as_u64().unwrap();
        let lower = case["lower"].as_u64().unwrap();
        let upper = case["upper"].as_u64().unwrap();
        let result = catch_unwind(|| {
            make(value, 1)
                .clamp(make(lower, 2), make(upper, 3))
                .as_scalar()
                .tag
        });
        if case["tag"].is_null() {
            assert!(result.is_err());
        } else {
            assert_eq!(u64::from(result.unwrap()), case["tag"].as_u64().unwrap());
        }
    }
}
