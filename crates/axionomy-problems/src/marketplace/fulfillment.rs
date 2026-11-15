//! Funded offers become reservations, shipments, and only then settlements.
use super::*;
use axionomy::path;
use axionomy::{QuantityComparison as C, Slot};

/// Whether `world` fulfills its contracts over time, as Showcase and Stress
/// do, rather than settling each order in one exchange.
pub fn enabled(world: &World) -> bool {
    world.named(&RateId::Advance { to: 1 }).is_some()
}
fn clock() -> Q<Ontology> {
    Q::balance(Role::Platform, Asset::Now)
}
fn law() -> Rate<Ontology> {
    platform().condition(
        path!("one-market-event"),
        Q::units(),
        C::Equal,
        Q::constant(1),
    )
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, Q::balance(role, asset))
}

/// When an order arrives, the tick its delivery is due by, and how long a
/// route delay holds up a carrier exposed to it: the order's schedule,
/// which its contract, delivery and dispatch read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub arrival: u64,
    pub deadline: u64,
    pub route_delay: u64,
}

/// Orders arrive in waves of two, two ticks apart, each due four ticks
/// after its wave and three more for each wave before it; C and E meet a
/// delay of two ticks on fast routes.
pub const fn schedule(order: OrderId) -> Schedule {
    let wave = order as u64 / 2;
    Schedule {
        arrival: wave * 2,
        deadline: 4 + wave * 3,
        route_delay: if matches!(order, OrderId::C | OrderId::E) {
            2
        } else {
            0
        },
    }
}

/// How long a carrier's shipment takes and whether route delays hold it up:
/// carrier A ships fast, exposed to delays; carrier B slower, never delayed.
pub const fn fleet(carrier: CarrierId) -> (u64, u64) {
    match carrier {
        CarrierId::A => (2, 1),
        CarrierId::B => (3, 0),
    }
}

/// The names a market's laws read its schedule, concession and fleet by.
fn arrival() -> Param<u64> {
    Param::new(path!("arrival"))
}
fn deadline() -> Param<u64> {
    Param::new(path!("deadline"))
}
fn route_delay() -> Param<u64> {
    Param::new(path!("route-delay"))
}
fn discount() -> Param<u64> {
    Param::new(path!("discount"))
}
fn price() -> Param<u64> {
    Param::new(path!("price"))
}
fn transit() -> Param<u64> {
    Param::new(path!("transit"))
}
fn exposure() -> Param<u64> {
    Param::new(path!("exposure"))
}
/// Whether a contract conceded ten credits of its price, `{discounted}`.
fn discounted() -> Param<bool> {
    Param::new(path!("discounted"))
}
/// The carrier a dispatch ships with, `{carrier}`.
fn carrier() -> Param<CarrierId> {
    Param::new(path!("carrier"))
}

/// Each of `orders`' schedule: law structure that never changes.
fn scheduled(orders: &[OrderId]) -> Relation {
    Relation::each_giving(
        &order(),
        [arrival(), deadline(), route_delay()].map(|number| number.name().clone()),
        orders.iter().map(|&order| {
            let schedule = schedule(order);
            (
                order,
                [schedule.arrival, schedule.deadline, schedule.route_delay],
            )
        }),
    )
}

/// What a conceded price gives up: ten credits, or nothing at full price.
fn concession() -> Relation {
    Relation::each_giving(
        &discounted(),
        [discount().name().clone()],
        [(false, [0u64]), (true, [10])],
    )
}

/// `rate` for any of `orders` and the item each trades, `{order}/{item}`, at
/// its price or ten credits less, `{discounted}`: its order role fixed to the
/// order's account, its terms and schedule given by their relations, and
/// `{price}` what the buyer pays.
fn agreed(rate: Rate<Ontology>, orders: &[OrderId]) -> Rate<Ontology> {
    among(rate, AccountIdPattern::Order(order().var()).into())
        .variable(order().values(orders.iter().copied()))
        .variable(item().every())
        .variable(discounted().every())
        .relation(priced(orders))
        .relation(scheduled(orders))
        .relation(concession())
        .derive(
            price().name().clone(),
            gross().quantity() - discount().quantity(),
        )
}

/// What an agreement locks in the order's account until it is delivered or
/// refunded: its contract, the item and the record of its price.
fn locked() -> Vec<(Pattern<Asset>, Quantity)> {
    one([
        AssetPattern::Contract(order().var()),
        AssetPattern::Item(item().var()),
        AssetPattern::AgreedPrice(price().var()),
    ])
}

/// A delivery of an agreement on time, or late with a ten-credit rebate to
/// the buyer from the seller's proceeds: the order settles at its price less
/// the rebate, the buyer gains its utility and any concession less the
/// rebate, and the seller its utility less both, never below nothing.
fn delivery(orders: &[OrderId], late: bool) -> Rate<Ontology> {
    let rebate = if late { 10 } else { 0 };
    let settled = Param::<u64>::new(path!("settled"));
    let outcome = if late { Asset::Late } else { Asset::OnTime };
    let (deadline, discount) = (deadline().quantity(), discount().quantity());
    let rate = clear(agreed(law(), orders), Role::Order, Asset::ReadyAt)
        .derive(
            settled.name().clone(),
            price().quantity() - Q::constant(rebate),
        )
        .condition(
            path!("delivery-time-elapsed"),
            clock(),
            C::GreaterThanOrEqual,
            Q::balance(Role::Order, Asset::ReadyAt),
        );
    let rate = if late {
        rate.condition(
            path!("deadline-outcome"),
            clock(),
            C::GreaterThanOrEqual,
            deadline + Q::constant(1),
        )
    } else {
        rate.condition(
            path!("deadline-outcome"),
            deadline,
            C::GreaterThanOrEqual,
            clock(),
        )
    };
    let lost = discount.clone() + Q::constant(rebate);
    let rate = rate
        .preserve_at(Role::Order, one([AssetPattern::AgreedPrice(price().var())]))
        .consume_at(
            Role::Order,
            one([
                AssetPattern::InShipment(order().var()),
                AssetPattern::Item(item().var()),
            ]),
        )
        .consume_computed(Role::Order, Asset::Money, price().quantity())
        .produce_at(
            Role::Order,
            one([
                AssetPattern::SettledOrder(order().var()),
                AssetPattern::SettledValue(settled.var()),
            ]),
        )
        .produce(Role::Order, basket([(outcome, 1)]))
        .consume_at(
            Role::Buyer,
            one([AssetPattern::PurchaseLocked(order().var())]),
        )
        .produce_at(
            Role::Buyer,
            one([
                AssetPattern::PurchaseReceipt(order().var()),
                AssetPattern::Item(item().var()),
            ]),
        )
        .produce_computed(
            Role::Buyer,
            Asset::Utility,
            buyer_utility().quantity() + discount - Q::constant(rebate),
        )
        .consume_at(
            Role::Seller,
            one([AssetPattern::SaleReserved(order().var())]),
        )
        .produce_at(
            Role::Seller,
            one([AssetPattern::CompletedSale(order().var())]),
        )
        .produce_computed(
            Role::Seller,
            Asset::Money,
            proceeds().quantity() - lost.clone(),
        )
        .produce_computed(
            Role::Seller,
            Asset::Utility,
            seller_utility().quantity() - Q::minimum(seller_utility().quantity(), lost),
        )
        .consume_at(Role::Carrier, one([AssetPattern::Booking(order().var())]))
        .consume(Role::Carrier, basket([(Asset::UsedShippingCapacity, 1)]))
        .produce_at(
            Role::Carrier,
            one([AssetPattern::DeliveredBy(order().var())]),
        )
        .produce(
            Role::Carrier,
            basket([(Asset::ShippingCapacity, 1), (outcome, 1)]),
        )
        .produce_computed(Role::Carrier, Asset::Money, shipping().quantity())
        .produce_computed(Role::Platform, Asset::Money, commission().quantity())
        .produce_computed(Role::TaxAuthority, Asset::Money, tax().quantity());
    if late {
        rate.produce(Role::Buyer, basket([(Asset::Money, rebate)]))
    } else {
        rate
    }
}

pub(super) fn build(source: World) -> World {
    let orders = orders(&source);
    let mut builder = EconomyBuilder::new();
    for (id, account) in source.accounts() {
        let mut account = account.clone();
        let extra = match id {
            AccountId::Buyer(_) => basket([(Asset::BuyerLicense, 1)]),
            AccountId::Seller(_) => basket([(Asset::SellerLicense, 1)]),
            AccountId::Carrier(_) => {
                let capacity = account.balance(&Asset::ShippingCapacity).to_u64();
                account
                    .withdraw(&basket([(Asset::ShippingCapacity, capacity - 1)]))
                    .unwrap();
                basket([(Asset::CarrierLicense, 1)])
            }
            AccountId::Platform => basket([(Asset::TimeRemaining, 32)]),
            AccountId::Order(_) | AccountId::TaxAuthority => basket([]),
        };
        account.deposit(&extra).unwrap();
        builder = builder.account(*id, account);
    }
    let agreement = |discounted: Slot<bool>| RateIdPattern::Contract {
        order: order().var(),
        item: item().var(),
        discounted,
    };
    // An open order's contract, once it has arrived, at its price or ten
    // credits less, `contract/{order}/{item}/{discounted}`: the buyer's
    // money and the seller's item are held in the order's account, and the
    // carrier's capacity booked, until the reservation's lease runs out.
    builder = builder.rate_at(
        agreement(discounted().var()).into(),
        agreed(law(), &orders)
            .preserve(Role::Buyer, basket([(Asset::BuyerLicense, 1)]))
            .preserve(Role::Seller, basket([(Asset::SellerLicense, 1)]))
            .preserve(Role::Carrier, basket([(Asset::CarrierLicense, 1)]))
            .condition(
                path!("order-has-arrived"),
                clock(),
                C::GreaterThanOrEqual,
                arrival().quantity(),
            )
            .consume_at(
                Role::Buyer,
                one([AssetPattern::PurchaseIntent(order().var())]),
            )
            .consume_computed(Role::Buyer, Asset::Money, price().quantity())
            .produce_at(
                Role::Buyer,
                one([AssetPattern::PurchaseLocked(order().var())]),
            )
            .consume_at(
                Role::Seller,
                one([
                    AssetPattern::Item(item().var()),
                    AssetPattern::SaleOffer(item().var()),
                ]),
            )
            .produce_at(
                Role::Seller,
                one([AssetPattern::SaleReserved(order().var())]),
            )
            .consume(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
            .produce(Role::Carrier, basket([(Asset::UsedShippingCapacity, 1)]))
            .produce_at(Role::Carrier, one([AssetPattern::Booking(order().var())]))
            .consume_at(Role::Order, one([AssetPattern::OpenOrder(order().var())]))
            .produce_at(Role::Order, locked())
            .produce_computed(Role::Order, Asset::Money, price().quantity())
            .produce_computed(Role::Order, Asset::LeaseUntil, clock() + Q::constant(2)),
    );
    // An unshipped reservation refunded once its lease has run out,
    // `cancel/{order}/{item}/{discounted}`: every party gets back what it
    // locked, and the order opens again.
    builder = builder.rate_at(
        RateIdPattern::Cancel {
            order: order().var(),
            item: item().var(),
            discounted: discounted().var(),
        }
        .into(),
        clear(agreed(law(), &orders), Role::Order, Asset::LeaseUntil)
            .condition(
                path!("unshipped-reservation-expired"),
                clock(),
                C::GreaterThanOrEqual,
                Q::balance(Role::Order, Asset::LeaseUntil) + Q::constant(1),
            )
            .consume_at(
                Role::Buyer,
                one([AssetPattern::PurchaseLocked(order().var())]),
            )
            .produce_at(
                Role::Buyer,
                one([AssetPattern::PurchaseIntent(order().var())]),
            )
            .produce_computed(Role::Buyer, Asset::Money, price().quantity())
            .consume_at(
                Role::Seller,
                one([AssetPattern::SaleReserved(order().var())]),
            )
            .produce_at(
                Role::Seller,
                one([
                    AssetPattern::SaleOffer(item().var()),
                    AssetPattern::Item(item().var()),
                ]),
            )
            .consume_at(Role::Carrier, one([AssetPattern::Booking(order().var())]))
            .consume(Role::Carrier, basket([(Asset::UsedShippingCapacity, 1)]))
            .produce(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
            .consume_at(Role::Order, locked())
            .consume_computed(Role::Order, Asset::Money, price().quantity())
            .produce_at(Role::Order, one([AssetPattern::OpenOrder(order().var())]))
            .produce(Role::Order, basket([(Asset::Refunded, 1)])),
    );
    // A shipment's delivery, on time or late, `deliver/{order}/{item}/
    // {discounted}/{late}`: the two differ in the deadline they meet.
    for late in [false, true] {
        builder = builder.rate_at(
            RateIdPattern::Deliver {
                order: order().var(),
                item: item().var(),
                discounted: discounted().var(),
                late: Slot::Is(late),
            }
            .into(),
            delivery(&orders, late),
        );
    }
    // A reservation's dispatch with the carrier that booked it,
    // `dispatch/{order}/{carrier}`: its shipment is ready after the
    // carrier's transit time, and the order's route delay where the carrier
    // is exposed to it.
    builder = builder.rate_at(
        RateIdPattern::Dispatch {
            order: order().var(),
            carrier: carrier().var(),
        }
        .into(),
        clear(
            among(law(), AccountIdPattern::Order(order().var()).into()),
            Role::Order,
            Asset::LeaseUntil,
        )
        .variable(order().values(orders.iter().copied()))
        .variable(carrier().every())
        .account(
            Role::Carrier,
            AccountIdPattern::Carrier(carrier().var()).into(),
        )
        .relation(scheduled(&orders))
        .relation(Relation::each_giving(
            &carrier(),
            [transit(), exposure()].map(|number| number.name().clone()),
            [CarrierId::A, CarrierId::B].map(|carrier| {
                let (transit, exposure) = fleet(carrier);
                (carrier, [transit, exposure])
            }),
        ))
        .preserve_at(
            Role::Buyer,
            one([AssetPattern::PurchaseLocked(order().var())]),
        )
        .preserve_at(
            Role::Seller,
            one([AssetPattern::SaleReserved(order().var())]),
        )
        .preserve_at(Role::Carrier, one([AssetPattern::Booking(order().var())]))
        .condition(
            path!("reservation-remains-valid"),
            clock(),
            C::LessThanOrEqual,
            Q::balance(Role::Order, Asset::LeaseUntil),
        )
        .consume_at(Role::Order, one([AssetPattern::Contract(order().var())]))
        .produce_at(Role::Order, one([AssetPattern::InShipment(order().var())]))
        .produce_computed(
            Role::Order,
            Asset::ReadyAt,
            clock() + transit().quantity() + exposure().quantity() * route_delay().quantity(),
        ),
    );
    // The clock advancing to any tick of the market's day, `advance/{to}`.
    let to = Param::<u8>::new(path!("to"));
    builder = builder.rate_at(
        RateIdPattern::Advance { to: to.var() }.into(),
        law()
            .variable(to.range(1, 32))
            .condition(
                path!("clock-moves-forward"),
                clock() + Q::constant(1),
                C::LessThanOrEqual,
                to.quantity(),
            )
            .consume_computed(
                Role::Platform,
                Asset::TimeRemaining,
                to.quantity() - clock(),
            )
            .produce_computed(Role::Platform, Asset::Now, to.quantity() - clock()),
    );
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

    /// Order A's contract, of a widget, at its price or ten credits less.
    fn contract(discounted: bool) -> RateId {
        RateId::Contract {
            order: OrderId::A,
            item: Item::Widget,
            discounted,
        }
    }

    /// Order A's dispatch with `carrier`.
    fn dispatch(carrier: CarrierId) -> RateId {
        RateId::Dispatch {
            order: OrderId::A,
            carrier,
        }
    }

    #[test]
    fn signed_contract_locks_money_goods_and_capacity_until_real_delivery() {
        let mut world = initial_showcase();
        let candidate = MarketMatch::new(OrderId::A, BuyerId::A, SellerId::A, CarrierId::A);
        world.apply(bind(contract(false), candidate)).unwrap();
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
        let gadget = RateId::Contract {
            order: OrderId::B,
            item: Item::Gadget,
            discounted: false,
        };
        assert!(!world.is_applicable(&bind(
            gadget,
            MarketMatch::new(OrderId::B, BuyerId::A, SellerId::C, CarrierId::A)
        )));
        // Only the carrier that booked the shipment dispatches it.
        assert!(!world.is_applicable(&bind(
            dispatch(CarrierId::B),
            MarketMatch::new(OrderId::A, BuyerId::A, SellerId::A, CarrierId::B)
        )));
        world
            .apply(bind(dispatch(CarrierId::A), candidate))
            .unwrap();
        // Carrier A ships in two ticks, and order A meets no route delay.
        assert_eq!(
            world
                .balance(&AccountId::Order(OrderId::A), &Asset::ReadyAt)
                .to_u64(),
            2
        );
        let delivery = bind(
            RateId::Deliver {
                order: OrderId::A,
                item: Item::Widget,
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
        world.apply(bind(contract(true), candidate)).unwrap();
        world
            .apply(
                Exchange::new(RateId::Advance { to: 3 }, Quantity::new(1))
                    .bind(Role::Platform, AccountId::Platform),
            )
            .unwrap();
        assert!(!world.is_applicable(&bind(dispatch(CarrierId::A), candidate)));
        world
            .apply(bind(
                RateId::Cancel {
                    order: OrderId::A,
                    item: Item::Widget,
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
        world.apply(bind(contract(false), candidate)).unwrap();
    }
}
