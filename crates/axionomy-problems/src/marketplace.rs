//! Assessment-driven matching and atomic multi-party market settlement.
//!
//! Candidate enumeration and ranking are derived, disposable policy. The
//! economy remains the authority for participant state, settlement terms,
//! feasibility, effects, and completed orders.

use axionomy::{
    Account, AssessmentStatus, Economy, EconomyBuilder, Exchange, ExchangeAssessment, Goal,
    LinearInvariant, Quantity, Rate, Trace, basket,
};
use axionomy_search::pareto::{self, Objective, ObjectiveVector, ParetoError, ParetoSearchResult};
use std::collections::HashSet;

pub const GROSS_PAYMENT: u64 = 100;
pub const SELLER_PROCEEDS: u64 = 80;
pub const TAX: u64 = 10;
pub const PLATFORM_COMMISSION: u64 = 5;
pub const SHIPPING_FEE: u64 = 5;
pub const SECOND_GROSS_PAYMENT: u64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Item {
    Widget,
    Gadget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuyerId {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SellerId {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CarrierId {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Buyer(BuyerId),
    Seller(SellerId),
    Platform,
    TaxAuthority,
    Carrier(CarrierId),
    Order(OrderId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Money,
    Item(Item),
    PurchaseIntent(OrderId),
    PurchaseReceipt(OrderId),
    SaleOffer(Item),
    CompletedSale(OrderId),
    MarketplaceLicense,
    TaxPolicy,
    ShippingCapacity,
    UsedShippingCapacity,
    OpenOrder(OrderId),
    SettledOrder(OrderId),
    SettledValue(u64),
    /// Participant benefit declared by settlement terms, distinct from cash flow.
    Utility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Buyer,
    Seller,
    Platform,
    TaxAuthority,
    Carrier,
    Order,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    SettleOrder(OrderId),
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Assessment = ExchangeAssessment<AccountId, Asset, RateId, Role>;
pub type ParetoResult = ParetoSearchResult<RateId, Role, AccountId, u64, ObjectiveKey, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveKey {
    Buyer(BuyerId),
    Seller(SellerId),
}

/// One possible order, buyer, seller, and carrier binding.
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

#[derive(Debug, Clone)]
pub struct ClearingProposal {
    trace: Trace<RateId, Role, AccountId>,
    settled_orders: usize,
    gross_value: u64,
}

impl ClearingProposal {
    pub const fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub const fn settled_orders(&self) -> usize {
        self.settled_orders
    }

    pub const fn gross_value(&self) -> u64 {
        self.gross_value
    }
}

/// A proposed match paired with the core's non-mutating explanation.
#[derive(Debug, Clone)]
pub struct AssessedMatch {
    candidate: MarketMatch,
    exchange: Action,
    assessment: Assessment,
}

impl AssessedMatch {
    pub const fn candidate(&self) -> MarketMatch {
        self.candidate
    }

    pub const fn exchange(&self) -> &Action {
        &self.exchange
    }

    pub const fn assessment(&self) -> &Assessment {
        &self.assessment
    }
}

pub fn initial() -> World {
    build(MarketSize::Micro)
}

/// A four-order market day with shared buyer budgets, seller inventory, and
/// carrier capacity coupling otherwise individually feasible settlements.
pub fn initial_showcase() -> World {
    build(MarketSize::Showcase)
}

/// A six-order market whose total inventory and shipping capacity are exactly
/// sufficient, coupling every settlement choice to the remaining market.
pub fn initial_stress() -> World {
    build(MarketSize::Stress)
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
        .account(
            AccountId::Platform,
            Account::from(basket([(Asset::MarketplaceLicense, 1)])),
        )
        .account(
            AccountId::TaxAuthority,
            Account::from(basket([(Asset::TaxPolicy, 1)])),
        )
        .invariant(LinearInvariant::new("money accounting").weight(Asset::Money, 1));
    for &order in orders {
        builder = builder
            .account(
                AccountId::Order(order),
                Account::from(basket([(Asset::OpenOrder(order), 1)])),
            )
            .rate(
                RateId::SettleOrder(order),
                all_distinct(settlement_rate(order)),
            );
    }
    let buyer_lifecycle = orders.iter().copied().fold(
        LinearInvariant::new("buyer order lifecycle"),
        |invariant, order| {
            invariant
                .weight(Asset::PurchaseIntent(order), 1)
                .weight(Asset::PurchaseReceipt(order), 1)
        },
    );
    let seller_lifecycle = orders.iter().copied().fold(
        LinearInvariant::new("seller order lifecycle")
            .weight(Asset::SaleOffer(Item::Widget), 1)
            .weight(Asset::SaleOffer(Item::Gadget), 1),
        |invariant, order| invariant.weight(Asset::CompletedSale(order), 1),
    );
    let order_lifecycle = orders.iter().copied().fold(
        LinearInvariant::new("order lifecycle"),
        |invariant, order| {
            invariant
                .weight(Asset::OpenOrder(order), 1)
                .weight(Asset::SettledOrder(order), 1)
        },
    );
    builder
        .invariant(LinearInvariant::new("money accounting").weight(Asset::Money, 1))
        .invariant(
            LinearInvariant::new("item accounting")
                .weight(Asset::Item(Item::Widget), 1)
                .weight(Asset::Item(Item::Gadget), 1),
        )
        .invariant(buyer_lifecycle)
        .invariant(seller_lifecycle)
        .invariant(
            LinearInvariant::new("shipping capacity accounting")
                .weight(Asset::ShippingCapacity, 1)
                .weight(Asset::UsedShippingCapacity, 1),
        )
        .invariant(order_lifecycle)
        .build()
        .expect("marketplace model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    goal_for(&MICRO_ORDERS)
}

pub fn goal_showcase() -> Goal<AccountId, Asset> {
    goal_for(&SHOWCASE_ORDERS)
}

pub fn goal_stress() -> Goal<AccountId, Asset> {
    goal_for(&STRESS_ORDERS)
}

fn goal_for(orders: &[OrderId]) -> Goal<AccountId, Asset> {
    orders.iter().copied().fold(Goal::new(), |goal, order| {
        goal.require(
            AccountId::Order(order),
            basket([(Asset::SettledOrder(order), 1)]),
        )
    })
}

/// Derives the finite Cartesian candidate set from participant accounts.
pub fn candidate_matches(world: &World) -> Vec<MarketMatch> {
    let mut buyers = Vec::new();
    let mut sellers = Vec::new();
    let mut carriers = Vec::new();

    for (account, _) in world.accounts() {
        match account {
            AccountId::Buyer(buyer) => buyers.push(*buyer),
            AccountId::Seller(seller) => sellers.push(*seller),
            AccountId::Carrier(carrier) => carriers.push(*carrier),
            AccountId::Platform | AccountId::TaxAuthority | AccountId::Order(_) => {}
        }
    }

    buyers.sort();
    sellers.sort();
    carriers.sort();

    let mut matches = Vec::new();
    for order in orders(world) {
        for buyer in &buyers {
            for seller in &sellers {
                for carrier in &carriers {
                    matches.push(MarketMatch::new(order, *buyer, *seller, *carrier));
                }
            }
        }
    }
    matches
}

pub fn orders(world: &World) -> Vec<OrderId> {
    let mut orders = world
        .rate_ids()
        .map(|rate| match rate {
            RateId::SettleOrder(order) => *order,
        })
        .collect::<Vec<_>>();
    orders.sort();
    orders.dedup();
    orders
}

pub fn candidates(world: &World) -> Vec<Action> {
    candidate_matches(world)
        .into_iter()
        .map(MarketMatch::exchange)
        .collect()
}

/// Filters candidate bindings through the core's applicability decision.
pub fn exact_matches(world: &World) -> Vec<Action> {
    world.applicable(candidates(world))
}

/// Assesses every candidate without mutating the market.
pub fn assessed_matches(world: &World) -> Vec<AssessedMatch> {
    candidate_matches(world)
        .into_iter()
        .map(|candidate| {
            let exchange = candidate.exchange();
            let assessment = world.assess(&exchange);
            AssessedMatch {
                candidate,
                exchange,
                assessment,
            }
        })
        .collect()
}

/// Ranks structurally valid but infeasible matches with caller-owned policy.
///
/// The cost function may turn the complete sparse shortfall vector into a
/// scalar heuristic, but it cannot change exchange validity or effects.
pub fn rank_near_matches(
    world: &World,
    mut cost: impl FnMut(&Assessment) -> u64,
) -> Vec<AssessedMatch> {
    let mut scored = assessed_matches(world)
        .into_iter()
        .filter(|candidate| candidate.assessment.status() == AssessmentStatus::Infeasible)
        .map(|candidate| (cost(candidate.assessment()), candidate))
        .collect::<Vec<_>>();

    scored.sort_by_key(|(score, candidate)| (*score, candidate.candidate));
    scored.into_iter().map(|(_, candidate)| candidate).collect()
}

/// Searches the finite concrete fixture for the best compatible settlement set.
///
/// The search state is disposable. Every edge is still a core-assessed
/// settlement exchange, and the returned trace is replay-validated.
pub fn clear_market(world: &World) -> ClearingProposal {
    let mut visited = HashSet::new();
    let mut best = ClearingProposal {
        trace: Trace::new(),
        settled_orders: settled_orders(world),
        gross_value: gross_value(world),
    };
    clear_branch(world.clone(), Trace::new(), &mut visited, &mut best);
    let replayed = world
        .replayed(best.trace())
        .expect("clearing search only records applicable exchanges");
    assert_eq!(settled_orders(&replayed), best.settled_orders);
    assert_eq!(gross_value(&replayed), best.gross_value);
    best
}

/// Exhaustively exposes non-dominated participant-utility allocations among
/// complete, atomic market clearings.
pub fn pareto_front(world: &World) -> Result<ParetoResult, ParetoError> {
    let goal = goal_for(&orders(world));
    pareto::search(world, &goal, exact_matches, objectives)
}

pub fn objectives(world: &World) -> ObjectiveVector<ObjectiveKey, u64> {
    ObjectiveVector::try_new([
        Objective::maximize(
            ObjectiveKey::Buyer(BuyerId::A),
            utility(world, AccountId::Buyer(BuyerId::A)),
        ),
        Objective::maximize(
            ObjectiveKey::Buyer(BuyerId::B),
            utility(world, AccountId::Buyer(BuyerId::B)),
        ),
        Objective::maximize(
            ObjectiveKey::Seller(SellerId::A),
            utility(world, AccountId::Seller(SellerId::A)),
        ),
        Objective::maximize(
            ObjectiveKey::Seller(SellerId::B),
            utility(world, AccountId::Seller(SellerId::B)),
        ),
    ])
    .expect("market objective schema is static and unique")
}

pub fn utility(world: &World, account: AccountId) -> u64 {
    world.balance(&account, &Asset::Utility).get()
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

fn settlement_rate(order: OrderId) -> Rate<Role, Asset> {
    let (item, gross, seller_proceeds, tax, commission, shipping) = order_terms(order);
    let (buyer_utility, seller_utility) = utility_terms(order);
    Rate::new()
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
        .preserve(Role::Platform, basket([(Asset::MarketplaceLicense, 1)]))
        .produce(Role::Platform, basket([(Asset::Money, commission)]))
        .preserve(Role::TaxAuthority, basket([(Asset::TaxPolicy, 1)]))
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

fn clear_branch(
    world: World,
    trace: Trace<RateId, Role, AccountId>,
    visited: &mut HashSet<Vec<(AccountId, Asset, Quantity)>>,
    best: &mut ClearingProposal,
) {
    if !visited.insert(world.state_key()) {
        return;
    }
    let candidate = (settled_orders(&world), gross_value(&world));
    if candidate > (best.settled_orders, best.gross_value) {
        best.trace = trace.clone();
        best.settled_orders = candidate.0;
        best.gross_value = candidate.1;
    }
    for exchange in exact_matches(&world) {
        let mut next = world.fork();
        if next.apply(exchange.clone()).is_err() {
            continue;
        }
        let mut next_trace = trace.clone();
        next_trace.push(exchange);
        clear_branch(next, next_trace, visited, best);
    }
}

fn settled_orders(world: &World) -> usize {
    orders(world)
        .into_iter()
        .filter(|order| {
            !world
                .balance(&AccountId::Order(*order), &Asset::SettledOrder(*order))
                .is_zero()
        })
        .count()
}

fn gross_value(world: &World) -> u64 {
    orders(world)
        .into_iter()
        .map(|order| {
            let gross = order_terms(order).1;
            world
                .balance(&AccountId::Order(order), &Asset::SettledValue(gross))
                .get()
                * gross
        })
        .sum()
}

fn all_distinct(mut rate: Rate<Role, Asset>) -> Rate<Role, Asset> {
    let roles = [
        Role::Buyer,
        Role::Seller,
        Role::Platform,
        Role::TaxAuthority,
        Role::Carrier,
        Role::Order,
    ];
    for (index, left) in roles.iter().enumerate() {
        for right in &roles[index + 1..] {
            rate = rate.distinct(*left, *right);
        }
    }
    rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{ApplyError, ExchangeAssessment, Trace};

    #[test]
    fn derives_and_filters_all_candidate_bindings_without_mutation() {
        let world = initial();
        let before = world.state_key();

        assert_eq!(candidate_matches(&world).len(), 36);
        let exact = exact_matches(&world);
        assert_eq!(exact.len(), 5);
        assert!(exact.iter().all(|exchange| world.is_applicable(exchange)));
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
        assert_eq!(exact_matches(&world).len(), 1);

        let mut trace = Trace::new();
        trace.push(replay_exchange);
        let replayed = initial().replayed(&trace).expect("settlement must replay");
        assert_eq!(replayed.state_key(), world.state_key());
    }

    #[test]
    fn clearing_search_selects_two_compatible_settlements_and_replays() {
        let world = initial();
        let clearing = clear_market(&world);

        assert_eq!(clearing.settled_orders(), 2);
        assert_eq!(clearing.gross_value(), GROSS_PAYMENT + SECOND_GROSS_PAYMENT);
        assert_eq!(clearing.trace().exchanges().len(), 2);
        let replayed = world
            .replayed(clearing.trace())
            .expect("clearing trace must replay");
        assert!(replayed.matches(&goal()));
        assert!(exact_matches(&replayed).is_empty());
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
    fn role_capabilities_reject_swapped_institutions() {
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

    #[test]
    fn caller_owned_cost_changes_near_match_ranking() {
        let world = initial();

        let cash_friendly = rank_near_matches(&world, |assessment| {
            weighted_shortfall(assessment, 1, 100, 50)
        });
        assert_eq!(
            cash_friendly[0].candidate(),
            MarketMatch::new(OrderId::A, BuyerId::C, SellerId::A, CarrierId::A)
        );

        let capacity_friendly = rank_near_matches(&world, |assessment| {
            weighted_shortfall(assessment, 100, 50, 1)
        });
        assert_eq!(
            capacity_friendly[0].candidate(),
            MarketMatch::new(OrderId::A, BuyerId::A, SellerId::A, CarrierId::B)
        );
    }

    #[test]
    fn pareto_front_exposes_buyer_and_seller_allocation_choices() {
        let initial = initial();
        let result = pareto_front(&initial).unwrap();
        assert_eq!(result.front().len(), 4);

        let mut outcomes = Vec::new();
        for entry in result.front().entries() {
            let replayed = initial.replayed(entry.payload()).unwrap();
            assert!(replayed.matches(&goal()));
            assert_eq!(&objectives(&replayed), entry.objectives());
            outcomes.push((
                utility(&replayed, AccountId::Buyer(BuyerId::A)),
                utility(&replayed, AccountId::Buyer(BuyerId::B)),
                utility(&replayed, AccountId::Seller(SellerId::A)),
                utility(&replayed, AccountId::Seller(SellerId::B)),
            ));
        }
        outcomes.sort_unstable();
        assert_eq!(
            outcomes,
            [
                (25, 30, 0, 20),
                (25, 30, 20, 0),
                (55, 0, 0, 20),
                (55, 0, 20, 0),
            ]
        );
    }

    #[test]
    fn stress_market_couples_six_orders_and_replays_the_clearing() {
        let showcase = initial_showcase();
        let stress = initial_stress();
        assert!(orders(&stress).len() > orders(&showcase).len());
        assert!(candidate_matches(&stress).len() > candidate_matches(&showcase).len());

        let clearing = clear_market(&stress);
        assert_eq!(clearing.settled_orders(), STRESS_ORDERS.len());
        let replayed = stress
            .replayed(clearing.trace())
            .expect("stress clearing must replay");
        assert!(replayed.matches(&goal_stress()));
    }

    fn weighted_shortfall(
        assessment: &Assessment,
        money_weight: u64,
        widget_weight: u64,
        capacity_weight: u64,
    ) -> u64 {
        assessment
            .shortfalls()
            .iter()
            .flat_map(|shortfall| shortfall.missing().iter())
            .map(|(asset, quantity)| {
                let weight = match asset {
                    Asset::Money => money_weight,
                    Asset::Item(Item::Widget | Item::Gadget) => widget_weight,
                    Asset::ShippingCapacity => capacity_weight,
                    _ => 1_000,
                };
                weight * quantity.get()
            })
            .sum()
    }
}
