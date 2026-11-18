#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, OneSegment, Param, Pattern,
    Quantity, QuantityExpression as Q, Rate, Relation, basket,
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
    LeaseUntil,
    ReadyAt,
    Now,
    TimeRemaining,
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

/// The market's laws, each naming its order, whose terms give the item it
/// trades. A contract, its cancellation and its delivery name whether its
/// price was conceded; a dispatch the carrier that booked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    /// Micro's settlement of an order in one exchange.
    SettleOrder {
        order: OrderId,
    },
    /// A funded contract: money, goods and capacity locked until delivery.
    Contract {
        order: OrderId,
        discounted: bool,
    },
    Dispatch {
        order: OrderId,
        carrier: CarrierId,
    },
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
    /// A seller accepts the standing concession on this order.
    DiscountOrder {
        order: OrderId,
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
        builder = builder.account(
            AccountId::Order(order),
            Account::from(basket([(Asset::OpenOrder(order), 1)])),
        );
    }
    builder = builder.rate_at(
        RateIdPattern::SettleOrder {
            order: order().var(),
        }
        .into(),
        settlement_rate(orders, 0),
    );
    if matches!(size, MarketSize::Micro) {
        builder = builder.rate_at(
            RateIdPattern::DiscountOrder {
                order: order().var(),
            }
            .into(),
            settlement_rate(&[OrderId::A], 7),
        );
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

/// The market's orders, in identifier order: each has an account of its own.
pub fn orders(world: &World) -> Vec<OrderId> {
    world
        .accounts()
        .filter_map(|(account, _)| match account {
            AccountId::Order(order) => Some(*order),
            _ => None,
        })
        .collect()
}

pub fn settlement(order: OrderId, buyer: BuyerId, seller: SellerId, carrier: CarrierId) -> Action {
    Exchange::new(RateId::SettleOrder { order }, Quantity::new(1))
        .bind(Role::Buyer, AccountId::Buyer(buyer))
        .bind(Role::Seller, AccountId::Seller(seller))
        .bind(Role::Platform, AccountId::Platform)
        .bind(Role::TaxAuthority, AccountId::TaxAuthority)
        .bind(Role::Carrier, AccountId::Carrier(carrier))
        .bind(Role::Order, AccountId::Order(order))
}

/// Micro's settlement of any of `orders`, `settle-order/{order}`: the buyer
/// pays the gross price for the item and a receipt, the seller is paid its
/// proceeds for the item and its offer, the platform its commission, the
/// tax authority the tax and the carrier its shipping fee for a unit of its
/// capacity, and the order settles at its gross price, the item and each
/// amount per unit as the order's terms give them.
fn settlement_rate(orders: &[OrderId], concession: u64) -> Rate<Ontology> {
    let (order, item) = (order(), item());
    let per_unit = |amount: Param<u64>| amount.quantity() * Q::units();
    among(platform(), AccountIdPattern::Order(order.var()).into())
        .variable(order.values(orders.iter().copied()))
        .relation(priced(orders, concession))
        .consume_at(
            Role::Buyer,
            one([AssetPattern::PurchaseIntent(order.var())]),
        )
        .consume_computed(Role::Buyer, Asset::Money, per_unit(gross()))
        .produce_at(
            Role::Buyer,
            one([
                AssetPattern::Item(item.var()),
                AssetPattern::PurchaseReceipt(order.var()),
            ]),
        )
        .produce_computed(Role::Buyer, Asset::Utility, per_unit(buyer_utility()))
        .consume_at(
            Role::Seller,
            one([
                AssetPattern::Item(item.var()),
                AssetPattern::SaleOffer(item.var()),
            ]),
        )
        .produce_at(
            Role::Seller,
            one([AssetPattern::CompletedSale(order.var())]),
        )
        .produce_computed(Role::Seller, Asset::Money, per_unit(proceeds()))
        .produce_computed(Role::Seller, Asset::Utility, per_unit(seller_utility()))
        .produce_computed(Role::Platform, Asset::Money, per_unit(commission()))
        .produce_computed(Role::TaxAuthority, Asset::Money, per_unit(tax()))
        .consume(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
        .produce(Role::Carrier, basket([(Asset::UsedShippingCapacity, 1)]))
        .produce_computed(Role::Carrier, Asset::Money, per_unit(shipping()))
        .consume_at(Role::Order, one([AssetPattern::OpenOrder(order.var())]))
        .produce_at(
            Role::Order,
            one([
                AssetPattern::SettledOrder(order.var()),
                AssetPattern::SettledValue(gross().var()),
            ]),
        )
}

/// One unit of each asset `assets` names.
fn one<const N: usize>(assets: [AssetPattern; N]) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset.into(), Quantity::new(1)))
        .collect()
}

/// What an order trades and on what terms: the item, the gross price its
/// buyer pays, its seller's proceeds, the tax, the platform's commission,
/// the carrier's shipping fee, and the utility its buyer and its seller gain
/// when it settles at that price. Its laws read them from a relation with a
/// row for each order, the item a word and the rest whole numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terms {
    pub item: Item,
    pub gross: u64,
    pub proceeds: u64,
    pub tax: u64,
    pub commission: u64,
    pub shipping: u64,
    pub buyer_utility: u64,
    pub seller_utility: u64,
}

pub const fn terms(order: OrderId) -> Terms {
    let (item, gross, proceeds, tax, commission, shipping, buyer_utility, seller_utility) =
        match order {
            OrderId::A => (
                Item::Widget,
                GROSS_PAYMENT,
                SELLER_PROCEEDS,
                TAX,
                PLATFORM_COMMISSION,
                SHIPPING_FEE,
                30,
                20,
            ),
            OrderId::B => (Item::Gadget, SECOND_GROSS_PAYMENT, 72, 9, 5, 4, 25, 18),
            OrderId::C => (Item::Widget, 110, 86, 11, 7, 6, 34, 17),
            OrderId::D => (Item::Gadget, 95, 75, 10, 5, 5, 22, 24),
            OrderId::E => (Item::Widget, 105, 83, 10, 6, 6, 28, 21),
            OrderId::F => (Item::Gadget, 115, 90, 12, 7, 6, 31, 19),
        };
    Terms {
        item,
        gross,
        proceeds,
        tax,
        commission,
        shipping,
        buyer_utility,
        seller_utility,
    }
}

/// The order a law's identifier names, `{order}`.
fn order() -> Param<OrderId> {
    Param::new(path!("order"))
}

/// The item an order's law trades, `{item}`, which its terms give.
fn item() -> Param<Item> {
    Param::new(path!("item"))
}

/// The numbers an order's terms give its laws, by the names they read.
fn gross() -> Param<u64> {
    Param::new(path!("gross"))
}
fn proceeds() -> Param<u64> {
    Param::new(path!("proceeds"))
}
fn tax() -> Param<u64> {
    Param::new(path!("tax"))
}
fn commission() -> Param<u64> {
    Param::new(path!("commission"))
}
fn shipping() -> Param<u64> {
    Param::new(path!("shipping"))
}
fn buyer_utility() -> Param<u64> {
    Param::new(path!("buyer-utility"))
}
fn seller_utility() -> Param<u64> {
    Param::new(path!("seller-utility"))
}

/// The item each of `orders` trades and the numbers of its terms: law
/// structure that never changes, a row for each order.
fn priced(orders: &[OrderId], concession: u64) -> Relation {
    Relation::giving(
        [order().name().clone()],
        [
            item().name().clone(),
            gross().name().clone(),
            proceeds().name().clone(),
            tax().name().clone(),
            commission().name().clone(),
            shipping().name().clone(),
            buyer_utility().name().clone(),
            seller_utility().name().clone(),
        ],
        orders.iter().map(|&order| {
            let terms = terms(order);
            [
                order.segment(),
                terms.item.segment(),
                (terms.gross - concession).into(),
                (terms.proceeds - concession).into(),
                terms.tax.into(),
                terms.commission.into(),
                terms.shipping.into(),
                (terms.buyer_utility + concession).into(),
                (terms.seller_utility - concession).into(),
            ]
        }),
    )
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

    /// Order A's settlement: of a widget, as its terms give.
    const SETTLE_A: RateId = RateId::SettleOrder { order: OrderId::A };

    #[test]
    fn an_order_settles_the_item_its_terms_give() {
        let mut world = initial();
        world
            .apply(settlement(
                OrderId::B,
                BuyerId::A,
                SellerId::C,
                CarrierId::A,
            ))
            .unwrap();
        let held = |account, item| world.balance(&account, &Asset::Item(item)).to_u64();
        assert_eq!(held(AccountId::Buyer(BuyerId::A), Item::Gadget), 1);
        assert_eq!(held(AccountId::Buyer(BuyerId::A), Item::Widget), 0);
        assert_eq!(held(AccountId::Seller(SellerId::C), Item::Gadget), 0);
    }

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
                RateId::SettleOrder {
                    order: OrderId::A, ..
                } => {
                    assert!(matches!(
                        exchange.bindings().get(&Role::Buyer),
                        Some(AccountId::Buyer(BuyerId::A | BuyerId::B))
                    ));
                    assert!(matches!(
                        exchange.bindings().get(&Role::Seller),
                        Some(AccountId::Seller(SellerId::A | SellerId::B))
                    ));
                }
                RateId::SettleOrder {
                    order: OrderId::B, ..
                } => {
                    assert_eq!(
                        exchange.bindings().get(&Role::Buyer),
                        Some(&AccountId::Buyer(BuyerId::A))
                    );
                    assert_eq!(
                        exchange.bindings().get(&Role::Seller),
                        Some(&AccountId::Seller(SellerId::C))
                    );
                }
                RateId::SettleOrder {
                    order: OrderId::C | OrderId::D | OrderId::E | OrderId::F,
                    ..
                } => {
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
        let exchange = Exchange::new(SETTLE_A, Quantity::new(1))
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
        let rebound = Exchange::new(SETTLE_A, Quantity::new(1))
            .bind(Role::Buyer, AccountId::Buyer(BuyerId::A))
            .bind(Role::Seller, AccountId::Seller(SellerId::A))
            .bind(Role::Platform, AccountId::TaxAuthority)
            .bind(Role::TaxAuthority, AccountId::Platform)
            .bind(Role::Carrier, AccountId::Carrier(CarrierId::A))
            .bind(Role::Order, AccountId::Order(OrderId::A));

        assert!(!world.is_applicable(&rebound));
    }
}
