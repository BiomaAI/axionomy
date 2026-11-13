//! The binary form the store records economies, exchanges and receipts in:
//! every field written, quantities as bytes, expressions tagged by variant,
//! and JSON, which people read, unchanged.

use axionomy::path::Path;
use axionomy::{Account, Quantity, QuantityComparison, Rounding, basket, paths};
use serde::{Serialize, de::DeserializeOwned};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

/// `value` written in binary and read back, checking that writing what was
/// read gives the same bytes.
fn binary<T: Serialize + DeserializeOwned>(value: &T) -> (Vec<u8>, T) {
    let bytes = postcard::to_allocvec(value).unwrap();
    let read: T = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(postcard::to_allocvec(&read).unwrap(), bytes);
    (bytes, read)
}

fn json(value: &impl Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

/// A bank that opens and closes vaults, paying a computed fee and checking a
/// parameter: every part of a rate the binary form must carry.
fn economy() -> paths::Economy {
    type Q = paths::QuantityExpression;
    let fee = Q::mul_div(Q::units(), Q::constant(3), Q::constant(100), Rounding::Ceil);
    paths::EconomyBuilder::new()
        .account(path("bank"), Account::from(basket([(path("cash"), 1_000)])))
        .account(path("treasury"), Account::from(basket([])))
        .rate(
            path("open"),
            paths::Rate::new()
                .consume(path("bank"), basket([(path("cash"), 1)]))
                .produce(path("vault"), basket([(path("cash"), 1)]))
                .consume_computed(path("bank"), path("cash"), fee.clone())
                .produce_computed(path("fees"), path("cash"), fee)
                .condition(
                    path("enough"),
                    Q::balance(path("bank"), path("cash")),
                    QuantityComparison::GreaterThanOrEqual,
                    Q::maximum(Q::parameter(path("floor")), Q::units()),
                )
                .opens(path("vault"))
                .distinct(path("bank"), path("vault")),
        )
        .rate(
            path("close"),
            paths::Rate::new()
                .consume_computed(
                    path("vault"),
                    path("cash"),
                    Q::balance(path("vault"), path("cash")),
                )
                .produce_computed(
                    path("bank"),
                    path("cash"),
                    Q::balance(path("vault"), path("cash")),
                )
                .closes(path("vault"))
                .distinct(path("bank"), path("vault")),
        )
        .invariant(paths::LinearInvariant::new(path("cash")).weight(path("cash"), 1))
        .build()
        .unwrap()
}

#[test]
fn a_quantity_is_its_bytes_in_binary_and_its_digits_in_json() {
    let largest = Quantity::from_be_bytes([u8::MAX; 32]);
    for (quantity, length) in [
        (Quantity::new(0), 0),
        (Quantity::new(1), 1),
        (Quantity::new(255), 1),
        (Quantity::new(256), 2),
        (Quantity::new(u64::MAX), 8),
        (largest, 32),
    ] {
        let (bytes, read) = binary(&quantity);
        assert_eq!(read, quantity);
        // A length, then the bytes without leading zeros.
        assert_eq!(bytes.len(), 1 + length);
        assert_eq!(
            json(&quantity),
            serde_json::Value::String(quantity.to_string())
        );
    }
    // Only the shortest form is read: a leading zero byte is refused.
    assert!(postcard::from_bytes::<Quantity>(&[2, 0, 1]).is_err());
}

#[test]
fn economies_exchanges_and_receipts_read_back_from_binary() {
    let mut economy = economy();
    let (_, read) = binary(&economy);
    assert_eq!(json(&read), json(&economy));

    let open = paths::Exchange::new(path("open"), Quantity::new(200))
        .bind(path("bank"), path("bank"))
        .bind(path("vault"), path("vault/1"))
        .bind(path("fees"), path("treasury"))
        .parameter(path("floor"), Quantity::new(10));
    let close = paths::Exchange::new(path("close"), Quantity::new(1))
        .bind(path("vault"), path("vault/1"))
        .bind(path("bank"), path("bank"));
    for exchange in [&open, &close] {
        assert_eq!(&binary(exchange).1, exchange);
    }

    let opened = economy.apply(open).unwrap();
    let closed = economy.apply(close).unwrap();
    for receipt in [&opened, &closed] {
        assert_eq!(json(&binary(receipt).1), json(receipt));
    }
    assert_eq!(opened.opened().len(), 1);
    assert_eq!(closed.closed().len(), 1);
    assert_eq!(json(&binary(&economy).1), json(&economy));
}

#[test]
fn json_keeps_the_shape_people_read() {
    let economy = economy();
    let rates = json(&economy)["rates"].clone();
    let rate = |id: &str| {
        rates
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry[0] == id)
            .map(|entry| entry[1].clone())
            .unwrap()
    };
    let (open, close) = (rate("open"), rate("close"));
    // A rate leaves out what it neither opens nor closes.
    assert!(open.get("closes").is_none() && open["opens"] == serde_json::json!(["vault"]));
    assert!(close.get("opens").is_none() && close["closes"] == serde_json::json!(["vault"]));
    // Expressions are tagged by kind.
    assert_eq!(open["computed_consume"][0]["quantity"]["kind"], "mul_div");
    assert_eq!(open["conditions"][0]["right"]["kind"], "maximum");

    let transfer =
        paths::Exchange::new(path("close"), Quantity::new(1)).bind(path("vault"), path("vault/1"));
    // An exchange without parameters leaves them out.
    assert_eq!(
        json(&transfer),
        serde_json::json!({"rate": "close", "bindings": {"vault": "vault/1"}, "units": "1"})
    );
}
