use axionomy::{Account, AccountError, AssetAmount, Basket, BasketError, Quantity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Credits,
    Tokens,
}

fn basket<const N: usize>(entries: [(Asset, u64); N]) -> Basket<Asset> {
    entries
        .map(|(asset, quantity)| (asset, Quantity::new(quantity)))
        .into()
}

#[test]
fn missing_assets_have_zero_balance() {
    let account = Account::new(basket([(Asset::Credits, 10)]));

    assert_eq!(account.balance(&Asset::Tokens), Quantity::ZERO);
}

#[test]
fn zero_quantities_are_not_stored() {
    let mut balances = basket([(Asset::Credits, 10)]);

    balances.insert(Asset::Credits, Quantity::ZERO);

    assert!(balances.is_empty());
}

#[test]
fn withdrawal_reports_exact_shortfall_and_is_atomic() {
    let mut account = Account::new(basket([(Asset::Credits, 5), (Asset::Tokens, 2)]));
    let original = account.clone();

    let error = account
        .withdraw(&basket([(Asset::Credits, 8), (Asset::Tokens, 2)]))
        .unwrap_err();

    assert_eq!(
        error,
        AccountError::InsufficientBalance {
            shortfall: basket([(Asset::Credits, 3)]),
        }
    );
    assert_eq!(account, original);
}

#[test]
fn deposit_overflow_is_atomic() {
    let mut account = Account::new(Basket::from([
        (Asset::Credits, Quantity::new(10)),
        (Asset::Tokens, Quantity::MAX),
    ]));
    let original = account.clone();

    let error = account
        .deposit(&basket([(Asset::Credits, 5), (Asset::Tokens, 1)]))
        .unwrap_err();

    assert_eq!(
        error,
        AccountError::Overflow {
            asset: Asset::Tokens,
        }
    );
    assert_eq!(account, original);
}

#[test]
fn baskets_scale_with_checked_arithmetic() {
    let balances = basket([(Asset::Credits, 4), (Asset::Tokens, 3)]);

    let scaled = balances.checked_scale(Quantity::new(2)).unwrap();

    assert_eq!(scaled, basket([(Asset::Credits, 8), (Asset::Tokens, 6)]));
    assert_eq!(
        Basket::from([(Asset::Credits, Quantity::MAX)])
            .checked_scale(Quantity::new(2))
            .unwrap_err(),
        Asset::Credits
    );
}

#[test]
fn baskets_offer_deterministic_sorted_iteration() {
    let balances = basket([(Asset::Tokens, 2), (Asset::Credits, 5)]);

    assert_eq!(
        balances.iter_sorted().collect::<Vec<_>>(),
        vec![
            (&Asset::Credits, &Quantity::new(5)),
            (&Asset::Tokens, &Quantity::new(2)),
        ]
    );
}

#[test]
fn baskets_accept_asset_qualified_amounts_and_reject_duplicates() {
    let balances = Basket::try_from_amounts([
        AssetAmount::new(Asset::Credits, Quantity::new(5)),
        AssetAmount::new(Asset::Tokens, Quantity::new(2)),
    ])
    .unwrap();

    assert_eq!(balances, basket([(Asset::Credits, 5), (Asset::Tokens, 2)]));
    assert_eq!(
        Basket::try_from_amounts([
            AssetAmount::new(Asset::Credits, Quantity::new(5)),
            AssetAmount::new(Asset::Credits, Quantity::new(2)),
        ]),
        Err(BasketError::DuplicateAsset {
            asset: Asset::Credits,
        })
    );
}

#[test]
fn zero_entries_cannot_hide_duplicate_assets() {
    for quantities in [[0, 0], [0, 7], [7, 0], [7, 9]] {
        let entries = [
            (Asset::Credits, Quantity::new(quantities[0])),
            (Asset::Tokens, Quantity::new(3)),
            (Asset::Credits, Quantity::new(quantities[1])),
        ];
        let expected = Err(BasketError::DuplicateAsset {
            asset: Asset::Credits,
        });
        assert_eq!(Basket::try_from_entries(entries), expected);
        assert_eq!(
            Basket::try_from_amounts(entries.map(AssetAmount::from)),
            expected
        );
    }
}

#[test]
fn validated_baskets_remove_zeros_without_reordering_other_assets() {
    let balances = Basket::try_from_entries([
        ("first", Quantity::new(2)),
        ("zero", Quantity::ZERO),
        ("last", Quantity::new(5)),
    ])
    .unwrap();
    assert_eq!(
        balances
            .iter()
            .map(|(asset, amount)| (*asset, amount.to_u64()))
            .collect::<Vec<_>>(),
        vec![("first", 2), ("last", 5)]
    );
    assert!(
        Basket::try_from_entries([("zero", Quantity::ZERO)])
            .unwrap()
            .is_empty()
    );
}
