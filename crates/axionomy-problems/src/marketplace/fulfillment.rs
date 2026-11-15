//! Funded offers become reservations, shipments, and only then settlements.
use super::*;
use axionomy::path;
use axionomy::{QuantityComparison as C, QuantityExpression as Q};

/// Whether `world` fulfills its contracts over time, as Showcase and Stress
/// do, rather than settling each order in one exchange.
pub fn enabled(world: &World) -> bool {
    world.rate(&RateId::Dispatch(OrderId::A)).is_some()
}
fn clock() -> Q<Ontology> {
    Q::balance(Role::Platform, Asset::Now)
}
fn law() -> Rate<Ontology> {
    Rate::new()
        .preserve(Role::Platform, basket([(Asset::MarketplaceLicense, 1)]))
        .condition(
            path!("one-market-event"),
            Q::units(),
            C::Equal,
            Q::constant(1),
        )
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, Q::balance(role, asset))
}
pub(super) fn build(source: World) -> World {
    let orders = orders(&source);
    let mut builder = EconomyBuilder::new();
    for (id, account) in source.accounts() {
        let mut account = account.clone();
        let extra = match id {
            AccountId::Buyer(_) => basket([(Asset::BuyerLicense, 1)]),
            AccountId::Seller(_) => basket([(Asset::SellerLicense, 1)]),
            AccountId::Carrier(carrier) => {
                let capacity = account.balance(&Asset::ShippingCapacity).to_u64();
                account
                    .withdraw(&basket([(Asset::ShippingCapacity, capacity - 1)]))
                    .unwrap();
                basket([
                    (Asset::CarrierLicense, 1),
                    (
                        Asset::TransitTime,
                        if *carrier == CarrierId::A { 2 } else { 3 },
                    ),
                    (Asset::DelayRisk, u64::from(*carrier == CarrierId::A)),
                ])
            }
            AccountId::Order(order) => basket([
                (Asset::Arrival, (*order as u64 / 2) * 2),
                (Asset::Deadline, 4 + (*order as u64 / 2) * 3),
                (
                    Asset::RouteDelay,
                    if matches!(order, OrderId::C | OrderId::E) {
                        2
                    } else {
                        0
                    },
                ),
            ]),
            AccountId::Platform => basket([(Asset::TimeRemaining, 32)]),
            AccountId::TaxAuthority => basket([]),
        };
        account.deposit(&extra).unwrap();
        builder = builder.account(*id, account);
    }
    for order in orders.iter().copied() {
        let (item, gross, proceeds, tax, commission, shipping) = order_terms(order);
        let (buyer_utility, seller_utility) = utility_terms(order);
        for discounted in [false, true] {
            let discount = if discounted { 10 } else { 0 };
            let price = gross - discount;
            let contract = all_distinct(
                law()
                    .preserve(Role::Buyer, basket([(Asset::BuyerLicense, 1)]))
                    .preserve(Role::Seller, basket([(Asset::SellerLicense, 1)]))
                    .preserve(Role::Carrier, basket([(Asset::CarrierLicense, 1)]))
                    .preserve(Role::TaxAuthority, basket([(Asset::TaxPolicy, 1)]))
                    .condition(
                        path!("order-has-arrived"),
                        clock(),
                        C::GreaterThanOrEqual,
                        Q::balance(Role::Order, Asset::Arrival),
                    )
                    .consume(
                        Role::Buyer,
                        basket([(Asset::Money, price), (Asset::PurchaseIntent(order), 1)]),
                    )
                    .produce(Role::Buyer, basket([(Asset::PurchaseLocked(order), 1)]))
                    .consume(
                        Role::Seller,
                        basket([(Asset::Item(item), 1), (Asset::SaleOffer(item), 1)]),
                    )
                    .produce(Role::Seller, basket([(Asset::SaleReserved(order), 1)]))
                    .consume(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
                    .produce(
                        Role::Carrier,
                        basket([(Asset::UsedShippingCapacity, 1), (Asset::Booking(order), 1)]),
                    )
                    .consume(Role::Order, basket([(Asset::OpenOrder(order), 1)]))
                    .produce(
                        Role::Order,
                        basket([
                            (Asset::Contract(order), 1),
                            (Asset::Money, price),
                            (Asset::Item(item), 1),
                            (Asset::AgreedPrice(price), 1),
                        ]),
                    )
                    .produce_computed(Role::Order, Asset::LeaseUntil, clock() + Q::constant(2)),
            );
            builder = builder.rate(
                if discounted {
                    RateId::DiscountedContract(order)
                } else {
                    RateId::SettleOrder(order)
                },
                contract,
            );
            let cancel = all_distinct(
                clear(law(), Role::Order, Asset::LeaseUntil)
                    .condition(
                        path!("unshipped-reservation-expired"),
                        clock(),
                        C::GreaterThanOrEqual,
                        Q::balance(Role::Order, Asset::LeaseUntil) + Q::constant(1),
                    )
                    .consume(Role::Buyer, basket([(Asset::PurchaseLocked(order), 1)]))
                    .produce(
                        Role::Buyer,
                        basket([(Asset::PurchaseIntent(order), 1), (Asset::Money, price)]),
                    )
                    .consume(Role::Seller, basket([(Asset::SaleReserved(order), 1)]))
                    .produce(
                        Role::Seller,
                        basket([(Asset::SaleOffer(item), 1), (Asset::Item(item), 1)]),
                    )
                    .consume(
                        Role::Carrier,
                        basket([(Asset::Booking(order), 1), (Asset::UsedShippingCapacity, 1)]),
                    )
                    .produce(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
                    .consume(
                        Role::Order,
                        basket([
                            (Asset::Contract(order), 1),
                            (Asset::Money, price),
                            (Asset::Item(item), 1),
                            (Asset::AgreedPrice(price), 1),
                        ]),
                    )
                    .produce(
                        Role::Order,
                        basket([(Asset::OpenOrder(order), 1), (Asset::Refunded, 1)]),
                    )
                    .preserve(Role::TaxAuthority, basket([(Asset::TaxPolicy, 1)])),
            );
            builder = builder.rate(RateId::Cancel { order, discounted }, cancel);
            for late in [false, true] {
                let rebate = if late { 10 } else { 0 };
                let deliver = all_distinct(
                    clear(law(), Role::Order, Asset::ReadyAt)
                        .condition(
                            path!("delivery-time-elapsed"),
                            clock(),
                            C::GreaterThanOrEqual,
                            Q::balance(Role::Order, Asset::ReadyAt),
                        )
                        .condition(
                            path!("deadline-outcome"),
                            if late {
                                clock()
                            } else {
                                Q::balance(Role::Order, Asset::Deadline)
                            },
                            C::GreaterThanOrEqual,
                            if late {
                                Q::balance(Role::Order, Asset::Deadline) + Q::constant(1)
                            } else {
                                clock()
                            },
                        )
                        .preserve(Role::Order, basket([(Asset::AgreedPrice(price), 1)]))
                        .consume(
                            Role::Order,
                            basket([
                                (Asset::InShipment(order), 1),
                                (Asset::Money, price),
                                (Asset::Item(item), 1),
                            ]),
                        )
                        .produce(
                            Role::Order,
                            basket([
                                (Asset::SettledOrder(order), 1),
                                (Asset::SettledValue(price - rebate), 1),
                                (if late { Asset::Late } else { Asset::OnTime }, 1),
                            ]),
                        )
                        .consume(Role::Buyer, basket([(Asset::PurchaseLocked(order), 1)]))
                        .produce(
                            Role::Buyer,
                            basket([
                                (Asset::PurchaseReceipt(order), 1),
                                (Asset::Item(item), 1),
                                (Asset::Money, rebate),
                                (Asset::Utility, buyer_utility + discount - rebate),
                            ]),
                        )
                        .consume(Role::Seller, basket([(Asset::SaleReserved(order), 1)]))
                        .produce(
                            Role::Seller,
                            basket([
                                (Asset::CompletedSale(order), 1),
                                (Asset::Money, proceeds - discount - rebate),
                                (
                                    Asset::Utility,
                                    seller_utility.saturating_sub(discount + rebate),
                                ),
                            ]),
                        )
                        .consume(
                            Role::Carrier,
                            basket([(Asset::Booking(order), 1), (Asset::UsedShippingCapacity, 1)]),
                        )
                        .produce(
                            Role::Carrier,
                            basket([
                                (Asset::ShippingCapacity, 1),
                                (Asset::Money, shipping),
                                (Asset::DeliveredBy(order), 1),
                                (if late { Asset::Late } else { Asset::OnTime }, 1),
                            ]),
                        )
                        .produce(Role::Platform, basket([(Asset::Money, commission)]))
                        .preserve(Role::TaxAuthority, basket([(Asset::TaxPolicy, 1)]))
                        .produce(Role::TaxAuthority, basket([(Asset::Money, tax)])),
                );
                builder = builder.rate(
                    RateId::Deliver {
                        order,
                        discounted,
                        late,
                    },
                    deliver,
                );
            }
        }
        builder = builder.rate(
            RateId::Dispatch(order),
            all_distinct(
                clear(law(), Role::Order, Asset::LeaseUntil)
                    .preserve(Role::Buyer, basket([(Asset::PurchaseLocked(order), 1)]))
                    .preserve(Role::Seller, basket([(Asset::SaleReserved(order), 1)]))
                    .preserve(Role::Carrier, basket([(Asset::Booking(order), 1)]))
                    .preserve(Role::TaxAuthority, basket([(Asset::TaxPolicy, 1)]))
                    .condition(
                        path!("reservation-remains-valid"),
                        clock(),
                        C::LessThanOrEqual,
                        Q::balance(Role::Order, Asset::LeaseUntil),
                    )
                    .consume(Role::Order, basket([(Asset::Contract(order), 1)]))
                    .produce(Role::Order, basket([(Asset::InShipment(order), 1)]))
                    .produce_computed(
                        Role::Order,
                        Asset::ReadyAt,
                        clock()
                            + Q::balance(Role::Carrier, Asset::TransitTime)
                            + Q::balance(Role::Carrier, Asset::DelayRisk)
                                * Q::balance(Role::Order, Asset::RouteDelay),
                    ),
            ),
        );
    }
    for to in 1..=32 {
        builder = builder.rate(
            RateId::Advance { to },
            law()
                .condition(
                    path!("clock-moves-forward"),
                    clock() + Q::constant(1),
                    C::LessThanOrEqual,
                    Q::constant(u64::from(to)),
                )
                .consume_computed(
                    Role::Platform,
                    Asset::TimeRemaining,
                    Q::constant(u64::from(to)) - clock(),
                )
                .produce_computed(
                    Role::Platform,
                    Asset::Now,
                    Q::constant(u64::from(to)) - clock(),
                ),
        );
    }
    for invariant in source.invariants() {
        let mut invariant = invariant.clone();
        for order in orders.iter().copied() {
            invariant = match invariant.name().as_str() {
                "buyer-order-lifecycle" => invariant.weight(Asset::PurchaseLocked(order), 1),
                "seller-order-lifecycle" => invariant.weight(Asset::SaleReserved(order), 1),
                "order-lifecycle" => invariant
                    .weight(Asset::Contract(order), 1)
                    .weight(Asset::InShipment(order), 1),
                _ => invariant,
            };
        }
        builder = builder.invariant(invariant);
    }
    builder
        .invariant(
            LinearInvariant::new(path!("market-time-budget"))
                .weight(Asset::Now, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .build()
        .expect("funded fulfillment conserves money, goods and capacity")
}

/// `rate`'s exchange of one unit, binding `parties` and the institutions.
pub fn bind(rate: RateId, parties: MarketMatch) -> Action {
    Exchange::new(rate, Quantity::new(1))
        .bind(Role::Buyer, AccountId::Buyer(parties.buyer))
        .bind(Role::Seller, AccountId::Seller(parties.seller))
        .bind(Role::Carrier, AccountId::Carrier(parties.carrier))
        .bind(Role::Order, AccountId::Order(parties.order))
        .bind(Role::Platform, AccountId::Platform)
        .bind(Role::TaxAuthority, AccountId::TaxAuthority)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_contract_locks_money_goods_and_capacity_until_real_delivery() {
        let mut world = initial_showcase();
        let candidate = MarketMatch::new(OrderId::A, BuyerId::A, SellerId::A, CarrierId::A);
        world.apply(candidate.exchange()).unwrap();
        assert_eq!(
            world
                .balance(&AccountId::Order(OrderId::A), &Asset::Money)
                .to_u64(),
            100
        );
        assert_eq!(
            world
                .balance(&AccountId::Buyer(BuyerId::A), &Asset::Item(Item::Widget))
                .to_u64(),
            0
        );
        assert!(!world.is_applicable(
            &MarketMatch::new(OrderId::B, BuyerId::A, SellerId::C, CarrierId::A).exchange()
        ));
        world
            .apply(bind(RateId::Dispatch(OrderId::A), candidate))
            .unwrap();
        let delivery = bind(
            RateId::Deliver {
                order: OrderId::A,
                discounted: false,
                late: false,
            },
            candidate,
        );
        assert!(!world.is_applicable(&delivery));
        world
            .apply(
                Exchange::new(RateId::Advance { to: 2 }, Quantity::new(1))
                    .bind(Role::Platform, AccountId::Platform),
            )
            .unwrap();
        assert!(
            !world.is_applicable(
                &delivery
                    .clone()
                    .bind(Role::Buyer, AccountId::Buyer(BuyerId::B))
            )
        );
        world.apply(delivery).unwrap();
        assert_eq!(
            world
                .balance(&AccountId::Carrier(CarrierId::A), &Asset::ShippingCapacity)
                .to_u64(),
            1
        );
        assert_eq!(
            world
                .balance(&AccountId::Seller(SellerId::A), &Asset::Money)
                .to_u64(),
            80
        );
    }
    #[test]
    fn expired_reservation_refunds_every_party_and_can_be_rebooked() {
        let mut world = initial_showcase();
        let candidate = MarketMatch::new(OrderId::A, BuyerId::A, SellerId::A, CarrierId::A);
        world
            .apply(bind(RateId::DiscountedContract(OrderId::A), candidate))
            .unwrap();
        world
            .apply(
                Exchange::new(RateId::Advance { to: 3 }, Quantity::new(1))
                    .bind(Role::Platform, AccountId::Platform),
            )
            .unwrap();
        assert!(!world.is_applicable(&bind(RateId::Dispatch(OrderId::A), candidate)));
        world
            .apply(bind(
                RateId::Cancel {
                    order: OrderId::A,
                    discounted: true,
                },
                candidate,
            ))
            .unwrap();
        assert_eq!(
            world
                .balance(&AccountId::Buyer(BuyerId::A), &Asset::Money)
                .to_u64(),
            300
        );
        assert_eq!(
            world
                .balance(&AccountId::Seller(SellerId::A), &Asset::Item(Item::Widget))
                .to_u64(),
            2
        );
        world.apply(candidate.exchange()).unwrap();
    }
}
