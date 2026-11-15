#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Pattern, Quantity, Rate,
    basket,
};

pub mod fulfillment;
#[cfg(feature = "packages")]
pub mod package;

pub const GROSS_PAYMENT: u64 = 100;
pub const SELLER_PROCEEDS: u64 = 80;
pub const TAX: u64 = 10;
pub const PLATFORM_COMMISSION: u64 = 5;
pub const SHIPPING_FEE: u64 = 5;
pub const SECOND_GROSS_PAYMENT: u64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum OrderId {
    A,
    B,
    C,
    D,
    E,
    F,
}

const MICRO_ORDERS: [OrderId; 2] = [OrderId::A, OrderId::B];
const SHOWCASE_ORDERS: [OrderId; 4] = [OrderId::A, OrderId::B, OrderId::C, OrderId::D];
const STRESS_ORDERS: [OrderId; 6] = [
    OrderId::A,
    OrderId::B,
    OrderId::C,
    OrderId::D,
    OrderId::E,
    OrderId::F,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketSize {
    Micro,
    Showcase,
    Stress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Item {
    Widget,
    Gadget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum BuyerId {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum SellerId {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum CarrierId {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Buyer(BuyerId),
    Seller(SellerId),
    Platform,
    TaxAuthority,
    Carrier(CarrierId),
    Order(OrderId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Money,
    Item(Item),
    PurchaseIntent(OrderId),
    PurchaseReceipt(OrderId),
    SaleOffer(Item),
    CompletedSale(OrderId),
    ShippingCapacity,
    UsedShippingCapacity,
    OpenOrder(OrderId),
    SettledOrder(OrderId),
    SettledValue(u64),
    /// Participant benefit declared by settlement terms, distinct from cash flow.
    Utility,
    BuyerLicense,
    SellerLicense,
    CarrierLicense,
    Contract(OrderId),
    InShipment(OrderId),
    PurchaseLocked(OrderId),
    SaleReserved(OrderId),
    Booking(OrderId),
    DeliveredBy(OrderId),
    AgreedPrice(u64),
    Arrival,
    Deadline,
    LeaseUntil,
    ReadyAt,
    Now,
    TimeRemaining,
    TransitTime,
    DelayRisk,
    RouteDelay,
    OnTime,
    Late,
    Refunded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Buyer,
    Seller,
    Platform,
    TaxAuthority,
    Carrier,
    Order,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    SettleOrder(OrderId),
    DiscountedContract(OrderId),
    Dispatch(OrderId),
    Deliver {
        order: OrderId,
        discounted: bool,
        late: bool,
    },
    Cancel {
        order: OrderId,
        discounted: bool,
    },
    Advance {
        to: u8,
    },
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `marketplace` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "marketplace";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

/// An order and the parties its exchanges bind: its buyer, seller and
/// carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MarketMatch {
    order: OrderId,
    buyer: BuyerId,
    seller: SellerId,
    carrier: CarrierId,
}

impl MarketMatch {
    pub const fn new(order: OrderId, buyer: BuyerId, seller: SellerId, carrier: CarrierId) -> Self {
        Self {
            order,
            buyer,
            seller,
            carrier,
        }
    }

    pub const fn order(self) -> OrderId {
        self.order
    }

    pub const fn buyer(self) -> BuyerId {
        self.buyer
    }

    pub const fn seller(self) -> SellerId {
        self.seller
    }

    pub const fn carrier(self) -> CarrierId {
        self.carrier
    }

    pub fn exchange(self) -> Action {
        settlement(self.order, self.buyer, self.seller, self.carrier)
    }
}

pub fn initial() -> World {
    build(MarketSize::Micro)
}

/// A four-order market day with shared buyer budgets, seller inventory, and
/// carrier capacity coupling otherwise individually feasible settlements.
pub fn initial_showcase() -> World {
    fulfillment::build(build(MarketSize::Showcase))
}

/// A six-order market whose total inventory and shipping capacity are exactly
/// sufficient, coupling every settlement choice to the remaining market.
pub fn initial_stress() -> World {
    fulfillment::build(build(MarketSize::Stress))
}

fn build(size: MarketSize) -> World {
    let orders: &[OrderId] = match size {
        MarketSize::Micro => &MICRO_ORDERS,
        MarketSize::Showcase => &SHOWCASE_ORDERS,
        MarketSize::Stress => &STRESS_ORDERS,
    };
    let showcase = !matches!(size, MarketSize::Micro);
    let stress = matches!(size, MarketSize::Stress);
    let mut buyer_a = basket([
        (
            Asset::Money,
            if stress {
                330
            } else if showcase {
                300
            } else {
                190
            },
        ),
        (Asset::PurchaseIntent(OrderId::A), 1),
        (
            Asset::PurchaseIntent(if stress { OrderId::C } else { OrderId::B }),
            1,
        ),
    ]);
    let mut buyer_b = basket([
        (
            Asset::Money,
            if stress {
                300
            } else if showcase {
                210
            } else {
                100
            },
        ),
        (
            Asset::PurchaseIntent(if stress { OrderId::B } else { OrderId::A }),
            1,
        ),
    ]);
    let mut buyer_c = basket([
        (
            Asset::Money,
            if stress {
                350
            } else if showcase {
                190
            } else {
                75
            },
        ),
        (
            Asset::PurchaseIntent(if stress { OrderId::D } else { OrderId::A }),
            1,
        ),
    ]);
    if stress {
        buyer_a.insert(Asset::PurchaseIntent(OrderId::E), Quantity::new(1));
        buyer_b.insert(Asset::PurchaseIntent(OrderId::F), Quantity::new(1));
    } else if showcase {
        buyer_a.insert(Asset::PurchaseIntent(OrderId::C), Quantity::new(1));
        buyer_b.insert(Asset::PurchaseIntent(OrderId::C), Quantity::new(1));
        buyer_c.insert(Asset::PurchaseIntent(OrderId::B), Quantity::new(1));
        buyer_c.insert(Asset::PurchaseIntent(OrderId::D), Quantity::new(1));
    }
    let mut builder = EconomyBuilder::new()
        .account(AccountId::Buyer(BuyerId::A), Account::from(buyer_a))
        .account(AccountId::Buyer(BuyerId::B), Account::from(buyer_b))
        .account(AccountId::Buyer(BuyerId::C), Account::from(buyer_c))
        .account(
            AccountId::Seller(SellerId::A),
            Account::from(basket([
                (Asset::Item(Item::Widget), if showcase { 2 } else { 1 }),
                (Asset::SaleOffer(Item::Widget), if showcase { 2 } else { 1 }),
            ])),
        )
        .account(
            AccountId::Seller(SellerId::B),
            Account::from(basket([
                (Asset::Item(Item::Widget), 1),
                (Asset::SaleOffer(Item::Widget), 1),
                (Asset::Item(Item::Gadget), u64::from(showcase)),
                (Asset::SaleOffer(Item::Gadget), u64::from(showcase)),
            ])),
        )
        .account(
            AccountId::Seller(SellerId::C),
            Account::from(basket([
                (Asset::Item(Item::Gadget), if showcase { 2 } else { 1 }),
                (Asset::SaleOffer(Item::Gadget), if showcase { 2 } else { 1 }),
            ])),
        )
        .account(
            AccountId::Carrier(CarrierId::A),
            Account::from(basket([(
                Asset::ShippingCapacity,
                if showcase { 3 } else { 2 },
            )])),
        )
        .account(
            AccountId::Carrier(CarrierId::B),
            Account::from(basket([(
                Asset::ShippingCapacity,
                if stress { 3 } else { u64::from(showcase) },
            )])),
        )
        .account(AccountId::Platform, Account::from(basket([])))
        .account(AccountId::TaxAuthority, Account::from(basket([])));
    for &order in orders {
        builder = builder
            .account(
                AccountId::Order(order),
                Account::from(basket([(Asset::OpenOrder(order), 1)])),
            )
            .rate(RateId::SettleOrder(order), settlement_rate(order));
    }
    let buyer_lifecycle = orders.iter().copied().fold(
        LinearInvariant::new(path!("buyer-order-lifecycle")),
        |invariant, order| {
            invariant
                .weight(Asset::PurchaseIntent(order), 1)
                .weight(Asset::PurchaseReceipt(order), 1)
        },
    );
    let seller_lifecycle = orders.iter().copied().fold(
        LinearInvariant::new(path!("seller-order-lifecycle"))
            .weight(Asset::SaleOffer(Item::Widget), 1)
            .weight(Asset::SaleOffer(Item::Gadget), 1),
        |invariant, order| invariant.weight(Asset::CompletedSale(order), 1),
    );
    let order_lifecycle = orders.iter().copied().fold(
        LinearInvariant::new(path!("order-lifecycle")),
        |invariant, order| {
            invariant
                .weight(Asset::OpenOrder(order), 1)
                .weight(Asset::SettledOrder(order), 1)
        },
    );
    builder
        .invariant(LinearInvariant::new(path!("money-accounting")).weight(Asset::Money, 1))
        .invariant(
            LinearInvariant::new(path!("item-accounting"))
                .weight(Asset::Item(Item::Widget), 1)
                .weight(Asset::Item(Item::Gadget), 1),
        )
        .invariant(buyer_lifecycle)
        .invariant(seller_lifecycle)
        .invariant(
            LinearInvariant::new(path!("shipping-capacity-accounting"))
                .weight(Asset::ShippingCapacity, 1)
                .weight(Asset::UsedShippingCapacity, 1),
        )
        .invariant(order_lifecycle)
        .build()
        .expect("marketplace model is valid")
}

pub fn goal() -> Goal<Ontology> {
    goal_for(&MICRO_ORDERS)
}

pub fn goal_showcase() -> Goal<Ontology> {
    goal_for(&SHOWCASE_ORDERS)
}

pub fn goal_stress() -> Goal<Ontology> {
    goal_for(&STRESS_ORDERS)
}

fn goal_for(orders: &[OrderId]) -> Goal<Ontology> {
    orders.iter().copied().fold(Goal::new(), |goal, order| {
        goal.require(
            AccountId::Order(order),
            basket([(Asset::SettledOrder(order), 1)]),
        )
    })
}

pub fn orders(world: &World) -> Vec<OrderId> {
    let mut orders = world
        .rate_ids()
        .filter_map(|rate| rate.as_literal())
        .filter_map(|rate| match rate {
            RateId::SettleOrder(order) => Some(*order),
            _ => None,
        })
        .collect::<Vec<_>>();
    orders.sort();
    orders.dedup();
    orders
}

pub fn settlement(order: OrderId, buyer: BuyerId, seller: SellerId, carrier: CarrierId) -> Action {
    Exchange::new(RateId::SettleOrder(order), Quantity::new(1))
        .bind(Role::Buyer, AccountId::Buyer(buyer))
        .bind(Role::Seller, AccountId::Seller(seller))
        .bind(Role::Platform, AccountId::Platform)
        .bind(Role::TaxAuthority, AccountId::TaxAuthority)
        .bind(Role::Carrier, AccountId::Carrier(carrier))
        .bind(Role::Order, AccountId::Order(order))
}

fn settlement_rate(order: OrderId) -> Rate<Ontology> {
    let (item, gross, seller_proceeds, tax, commission, shipping) = order_terms(order);
    let (buyer_utility, seller_utility) = utility_terms(order);
    among(platform(), Pattern::literal(AccountId::Order(order)))
        .consume(
            Role::Buyer,
            basket([(Asset::Money, gross), (Asset::PurchaseIntent(order), 1)]),
        )
        .produce(
            Role::Buyer,
            basket([
                (Asset::Item(item), 1),
                (Asset::PurchaseReceipt(order), 1),
                (Asset::Utility, buyer_utility),
            ]),
        )
        .consume(
            Role::Seller,
            basket([(Asset::Item(item), 1), (Asset::SaleOffer(item), 1)]),
        )
        .produce(
            Role::Seller,
            basket([
                (Asset::Money, seller_proceeds),
                (Asset::CompletedSale(order), 1),
                (Asset::Utility, seller_utility),
            ]),
        )
        .produce(Role::Platform, basket([(Asset::Money, commission)]))
        .produce(Role::TaxAuthority, basket([(Asset::Money, tax)]))
        .consume(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
        .produce(
            Role::Carrier,
            basket([(Asset::Money, shipping), (Asset::UsedShippingCapacity, 1)]),
        )
        .consume(Role::Order, basket([(Asset::OpenOrder(order), 1)]))
        .produce(
            Role::Order,
            basket([
                (Asset::SettledOrder(order), 1),
                (Asset::SettledValue(gross), 1),
            ]),
        )
}

const fn utility_terms(order: OrderId) -> (u64, u64) {
    match order {
        OrderId::A => (30, 20),
        OrderId::B => (25, 18),
        OrderId::C => (34, 17),
        OrderId::D => (22, 24),
        OrderId::E => (28, 21),
        OrderId::F => (31, 19),
    }
}

const fn order_terms(order: OrderId) -> (Item, u64, u64, u64, u64, u64) {
    match order {
        OrderId::A => (
            Item::Widget,
            GROSS_PAYMENT,
            SELLER_PROCEEDS,
            TAX,
            PLATFORM_COMMISSION,
            SHIPPING_FEE,
        ),
        OrderId::B => (Item::Gadget, SECOND_GROSS_PAYMENT, 72, 9, 5, 4),
        OrderId::C => (Item::Widget, 110, 86, 11, 7, 6),
        OrderId::D => (Item::Gadget, 95, 75, 10, 5, 5),
        OrderId::E => (Item::Widget, 105, 83, 10, 6, 6),
        OrderId::F => (Item::Gadget, 115, 90, 12, 7, 6),
    }
}

/// The parties an order's exchange chooses: its buyer, seller and carrier.
const PARTIES: [Role; 3] = [Role::Buyer, Role::Seller, Role::Carrier];

/// A law of the market, its platform role fixed to the platform's account.
fn platform() -> Rate<Ontology> {
    Rate::new().account(Role::Platform, Pattern::literal(AccountId::Platform))
}

/// `rate` as an exchange of the order whose account `order` names: its
/// institutions fixed to their accounts, the tax authority and the order's
/// own, and its parties distinct from one another and from the
/// institutions.
fn among(rate: Rate<Ontology>, order: Pattern<AccountId>) -> Rate<Ontology> {
    let mut rate = rate
        .account(
            Role::TaxAuthority,
            Pattern::literal(AccountId::TaxAuthority),
        )
        .account(Role::Order, order);
    for (index, party) in PARTIES.iter().enumerate() {
        for other in
            PARTIES[index + 1..]
                .iter()
                .chain(&[Role::Platform, Role::TaxAuthority, Role::Order])
        {
            rate = rate.distinct(*party, *other);
        }
    }
    rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{ApplyError, AssessmentStatus, ExchangeAssessment, Trace};

    /// Every binding of a settlement of each of the market's orders: each
    /// buyer, seller and carrier the market has.
    fn every_settlement(world: &World) -> Vec<Action> {
        let mut settlements = Vec::new();
        for order in orders(world) {
            for buyer in [BuyerId::A, BuyerId::B, BuyerId::C] {
                for seller in [SellerId::A, SellerId::B, SellerId::C] {
                    for carrier in [CarrierId::A, CarrierId::B] {
                        settlements.push(settlement(order, buyer, seller, carrier));
                    }
                }
            }
        }
        settlements
    }

    #[test]
    fn five_of_the_thirty_six_settlements_apply_and_assessing_changes_nothing() {
        let world = initial();
        let before = world.state_key();

        let every = every_settlement(&world);
        assert_eq!(every.len(), 36);
        let exact = world.applicable(every);
        assert_eq!(exact.len(), 5);
        assert_eq!(world.state_key(), before);

        for exchange in exact {
            match exchange.rate() {
                RateId::SettleOrder(OrderId::A) => {
                    assert!(matches!(
                        exchange.bindings().get(&Role::Buyer),
                        Some(AccountId::Buyer(BuyerId::A | BuyerId::B))
                    ));
                    assert!(matches!(
                        exchange.bindings().get(&Role::Seller),
                        Some(AccountId::Seller(SellerId::A | SellerId::B))
                    ));
                }
                RateId::SettleOrder(OrderId::B) => {
                    assert_eq!(
                        exchange.bindings().get(&Role::Buyer),
                        Some(&AccountId::Buyer(BuyerId::A))
                    );
                    assert_eq!(
                        exchange.bindings().get(&Role::Seller),
                        Some(&AccountId::Seller(SellerId::C))
                    );
                }
                RateId::SettleOrder(OrderId::C | OrderId::D | OrderId::E | OrderId::F) => {
                    unreachable!("micro fixture only has two orders")
                }
                _ => unreachable!("Micro settles directly"),
            }
            assert_eq!(
                exchange.bindings().get(&Role::Carrier),
                Some(&AccountId::Carrier(CarrierId::A))
            );
        }
    }

    #[test]
    fn reports_buyer_seller_and_carrier_shortfalls_together() {
        let world = initial();
        let before = world.state_key();
        let exchange = settlement(OrderId::A, BuyerId::C, SellerId::C, CarrierId::B);
        let assessment = world.assess(&exchange);

        assert_eq!(assessment.status(), AssessmentStatus::Infeasible);
        assert_eq!(assessment.shortfalls().len(), 3);
        assert_eq!(
            assessment
                .shortfall(&AccountId::Buyer(BuyerId::C))
                .expect("buyer shortfall")
                .quantity(&Asset::Money),
            Quantity::new(25)
        );
        assert_eq!(
            assessment
                .shortfall(&AccountId::Seller(SellerId::C))
                .expect("seller shortfall")
                .quantity(&Asset::Item(Item::Widget)),
            Quantity::new(1)
        );
        assert_eq!(
            assessment
                .shortfall(&AccountId::Carrier(CarrierId::B))
                .expect("carrier shortfall")
                .quantity(&Asset::ShippingCapacity),
            Quantity::new(1)
        );
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn settles_six_accounts_atomically_and_projection_matches_receipt() {
        let mut world = initial();
        let exchange = settlement(OrderId::A, BuyerId::A, SellerId::A, CarrierId::A);
        let replay_exchange = exchange.clone();
        let assessment = world.assess(&exchange);
        let projected = assessment
            .projected_deltas()
            .expect("ready match projects deltas");
        assert_eq!(projected.len(), 6);

        let receipt = world.apply(exchange).expect("ready match settles");
        assert_eq!(receipt.deltas().len(), 6);
        for (projected, actual) in projected.iter().zip(receipt.deltas()) {
            assert_eq!(projected.account(), actual.account());
            assert_eq!(projected.consumed(), actual.consumed());
            assert_eq!(projected.produced(), actual.produced());
            assert_eq!(projected.preserved(), actual.preserved());
        }

        assert_eq!(
            world.balance(&AccountId::Buyer(BuyerId::A), &Asset::Money),
            Quantity::new(90)
        );
        assert_eq!(
            world.balance(&AccountId::Buyer(BuyerId::A), &Asset::Item(Item::Widget)),
            Quantity::new(1)
        );
        assert_eq!(
            world.balance(&AccountId::Seller(SellerId::A), &Asset::Money),
            Quantity::new(SELLER_PROCEEDS)
        );
        assert_eq!(
            world.balance(&AccountId::Platform, &Asset::Money),
            Quantity::new(PLATFORM_COMMISSION)
        );
        assert_eq!(
            world.balance(&AccountId::TaxAuthority, &Asset::Money),
            Quantity::new(TAX)
        );
        assert_eq!(
            world.balance(&AccountId::Carrier(CarrierId::A), &Asset::Money),
            Quantity::new(SHIPPING_FEE)
        );
        assert_eq!(
            world.balance(
                &AccountId::Order(OrderId::A),
                &Asset::SettledOrder(OrderId::A)
            ),
            Quantity::new(1)
        );
        assert_eq!(
            world.applicable(every_settlement(&world)),
            [settlement(
                OrderId::B,
                BuyerId::A,
                SellerId::C,
                CarrierId::A
            )]
        );

        let mut trace = Trace::new();
        trace.push(replay_exchange);
        let replayed = initial().replayed(&trace).expect("settlement must replay");
        assert_eq!(replayed.state_key(), world.state_key());
    }

    #[test]
    fn two_compatible_settlements_clear_the_market_and_replay() {
        let world = initial();
        // Buyer A's budget covers both orders exactly, with seller A's
        // widget, seller C's gadget and both of carrier A's capacity.
        let clearing: Trace<Ontology> = [
            settlement(OrderId::A, BuyerId::A, SellerId::A, CarrierId::A),
            settlement(OrderId::B, BuyerId::A, SellerId::C, CarrierId::A),
        ]
        .into_iter()
        .collect();
        let replayed = world
            .replayed(&clearing)
            .expect("clearing trace must replay");
        assert!(replayed.matches(&goal()));
        let settled = |order, value| {
            replayed
                .balance(&AccountId::Order(order), &Asset::SettledValue(value))
                .to_u64()
                * value
        };
        assert_eq!(
            settled(OrderId::A, GROSS_PAYMENT) + settled(OrderId::B, SECOND_GROSS_PAYMENT),
            GROSS_PAYMENT + SECOND_GROSS_PAYMENT
        );
        assert!(replayed.applicable(every_settlement(&replayed)).is_empty());
        assert_eq!(
            world.replayed(&clearing).unwrap().state_key(),
            replayed.state_key()
        );
    }

    #[test]
    fn failed_settlement_returns_all_shortfalls_and_changes_nothing() {
        let mut world = initial();
        let before = world.state_key();
        let error = world
            .apply(settlement(
                OrderId::A,
                BuyerId::C,
                SellerId::C,
                CarrierId::B,
            ))
            .expect_err("three participants are missing requirements");

        let ApplyError::Infeasible { shortfalls } = error else {
            panic!("expected an infeasible exchange");
        };
        assert_eq!(shortfalls.len(), 3);
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn rejects_one_account_bound_to_distinct_market_roles() {
        let world = initial();
        let exchange = Exchange::new(RateId::SettleOrder(OrderId::A), Quantity::new(1))
            .bind(Role::Buyer, AccountId::Buyer(BuyerId::A))
            .bind(Role::Seller, AccountId::Buyer(BuyerId::A))
            .bind(Role::Platform, AccountId::Platform)
            .bind(Role::TaxAuthority, AccountId::TaxAuthority)
            .bind(Role::Carrier, AccountId::Carrier(CarrierId::A))
            .bind(Role::Order, AccountId::Order(OrderId::A));

        let ExchangeAssessment::Invalid { issues } = world.assess(&exchange) else {
            panic!("buyer and seller must be different accounts");
        };
        assert!(issues.iter().any(|issue| matches!(
            issue,
            ApplyError::RolesMustDiffer {
                left: Role::Buyer,
                right: Role::Seller,
            }
        )));
    }

    #[test]
    fn fixed_accounts_reject_swapped_institutions() {
        let world = initial();
        let rebound = Exchange::new(RateId::SettleOrder(OrderId::A), Quantity::new(1))
            .bind(Role::Buyer, AccountId::Buyer(BuyerId::A))
            .bind(Role::Seller, AccountId::Seller(SellerId::A))
            .bind(Role::Platform, AccountId::TaxAuthority)
            .bind(Role::TaxAuthority, AccountId::Platform)
            .bind(Role::Carrier, AccountId::Carrier(CarrierId::A))
            .bind(Role::Order, AccountId::Order(OrderId::A));

        assert!(!world.is_applicable(&rebound));
    }
}
