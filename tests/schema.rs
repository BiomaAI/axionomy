use axionomy::{
    Account, Quantity, basket,
    paths::{Economy as WireEconomy, EconomyBuilder, Exchange as WireExchange, Rate},
};

#[test]
fn economy_schema_matches_the_direct_serde_shape() {
    let schema = schemars::schema_for!(WireEconomy);
    let schema = serde_json::to_value(schema).expect("schema serializes");
    let root = schema
        .get("$ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .and_then(|name| schema["$defs"].get(name))
        .unwrap_or(&schema);
    let properties = root
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("economy schema is an object");

    assert!(properties.contains_key("accounts"));
    assert!(properties.contains_key("rates"));
    assert!(properties.contains_key("invariants"));
}

#[test]
fn public_schema_types_round_trip_through_their_existing_serde_contract() {
    let economy: WireEconomy = EconomyBuilder::new()
        .account(
            "workshop".parse().unwrap(),
            Account::from(basket([("raw".parse().unwrap(), 1)])),
        )
        .rate(
            "build".parse().unwrap(),
            Rate::new()
                .consume(
                    "shop".parse().unwrap(),
                    basket([("raw".parse().unwrap(), 1)]),
                )
                .produce(
                    "shop".parse().unwrap(),
                    basket([("finished".parse().unwrap(), 1)]),
                ),
        )
        .build()
        .expect("model is valid");
    let exchange = WireExchange::new("build".parse().unwrap(), Quantity::new(1))
        .bind("shop".parse().unwrap(), "workshop".parse().unwrap());

    let economy_json = serde_json::to_value(&economy).expect("economy serializes");
    let exchange_json = serde_json::to_value(&exchange).expect("exchange serializes");
    let decoded_economy: WireEconomy =
        serde_json::from_value(economy_json).expect("economy deserializes");
    let decoded_exchange: WireExchange =
        serde_json::from_value(exchange_json).expect("exchange deserializes");

    assert!(decoded_economy.is_applicable(&decoded_exchange));
}
