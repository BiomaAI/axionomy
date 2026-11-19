//! Funded offers become reservations, shipments, and only then settlements.
use super::*;

/// Whether `world` fulfills its contracts over time, as Showcase and Stress
/// do, rather than settling each order in one exchange.
pub fn enabled(world: &World) -> bool {
    world.named(&RateId::Advance { to: 1 }).is_some()
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
