//! Assessment-driven matching and atomic multi-party market settlement.
//!
//! Candidate enumeration and ranking are derived, disposable policy. The
//! economy remains the authority for participant state, settlement terms,
//! feasibility, effects, and the completed order.

use axionomy::{
    Account, AssessmentStatus, Economy, EconomyBuilder, Exchange, ExchangeAssessment, Goal,
    LinearInvariant, Quantity, Rate, basket,
};

pub const GROSS_PAYMENT: u64 = 100;
pub const SELLER_PROCEEDS: u64 = 80;
pub const TAX: u64 = 10;
pub const PLATFORM_COMMISSION: u64 = 5;
pub const SHIPPING_FEE: u64 = 5;

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
    OrderBook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Money,
    Widget,
    PurchaseIntent,
    PurchaseReceipt,
    SaleOffer,
    CompletedSale,
    MarketplaceLicense,
    TaxPolicy,
    ShippingCapacity,
    UsedShippingCapacity,
    OpenOrder,
    SettledOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Buyer,
    Seller,
    Platform,
    TaxAuthority,
    Carrier,
    OrderBook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    SettleOrder,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Assessment = ExchangeAssessment<AccountId, Asset, RateId, Role>;

/// One possible buyer, seller, and carrier binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MarketMatch {
    buyer: BuyerId,
    seller: SellerId,
    carrier: CarrierId,
}

impl MarketMatch {
    pub const fn new(buyer: BuyerId, seller: SellerId, carrier: CarrierId) -> Self {
        Self {
            buyer,
            seller,
            carrier,
        }
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
        settlement(self.buyer, self.seller, self.carrier)
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
    EconomyBuilder::new()
        .account(
            AccountId::Buyer(BuyerId::A),
            Account::from(basket([(Asset::Money, 120), (Asset::PurchaseIntent, 1)])),
        )
        .account(
            AccountId::Buyer(BuyerId::B),
            Account::from(basket([(Asset::Money, 100), (Asset::PurchaseIntent, 1)])),
        )
        .account(
            AccountId::Buyer(BuyerId::C),
            Account::from(basket([(Asset::Money, 75), (Asset::PurchaseIntent, 1)])),
        )
        .account(
            AccountId::Seller(SellerId::A),
            Account::from(basket([(Asset::Widget, 1), (Asset::SaleOffer, 1)])),
        )
        .account(
            AccountId::Seller(SellerId::B),
            Account::from(basket([(Asset::Widget, 1), (Asset::SaleOffer, 1)])),
        )
        .account(
            AccountId::Seller(SellerId::C),
            Account::from(basket([(Asset::SaleOffer, 1)])),
        )
        .account(
            AccountId::Carrier(CarrierId::A),
            Account::from(basket([(Asset::ShippingCapacity, 1)])),
        )
        .account(AccountId::Carrier(CarrierId::B), Account::default())
        .account(
            AccountId::Platform,
            Account::from(basket([(Asset::MarketplaceLicense, 1)])),
        )
        .account(
            AccountId::TaxAuthority,
            Account::from(basket([(Asset::TaxPolicy, 1)])),
        )
        .account(
            AccountId::OrderBook,
            Account::from(basket([(Asset::OpenOrder, 1)])),
        )
        .rate(RateId::SettleOrder, all_distinct(settlement_rate()))
        .invariant(LinearInvariant::new("money accounting").weight(Asset::Money, 1))
        .invariant(LinearInvariant::new("widget accounting").weight(Asset::Widget, 1))
        .invariant(
            LinearInvariant::new("buyer order lifecycle")
                .weight(Asset::PurchaseIntent, 1)
                .weight(Asset::PurchaseReceipt, 1),
        )
        .invariant(
            LinearInvariant::new("seller order lifecycle")
                .weight(Asset::SaleOffer, 1)
                .weight(Asset::CompletedSale, 1),
        )
        .invariant(
            LinearInvariant::new("shipping capacity accounting")
                .weight(Asset::ShippingCapacity, 1)
                .weight(Asset::UsedShippingCapacity, 1),
        )
        .invariant(
            LinearInvariant::new("order-book lifecycle")
                .weight(Asset::OpenOrder, 1)
                .weight(Asset::SettledOrder, 1),
        )
        .build()
        .expect("marketplace model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::OrderBook, basket([(Asset::SettledOrder, 1)]))
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
            AccountId::Platform | AccountId::TaxAuthority | AccountId::OrderBook => {}
        }
    }

    buyers.sort();
    sellers.sort();
    carriers.sort();

    let mut matches = Vec::new();
    for buyer in buyers {
        for seller in &sellers {
            for carrier in &carriers {
                matches.push(MarketMatch::new(buyer, *seller, *carrier));
            }
        }
    }
    matches
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

pub fn settlement(buyer: BuyerId, seller: SellerId, carrier: CarrierId) -> Action {
    Exchange::new(RateId::SettleOrder, Quantity::new(1))
        .bind(Role::Buyer, AccountId::Buyer(buyer))
        .bind(Role::Seller, AccountId::Seller(seller))
        .bind(Role::Platform, AccountId::Platform)
        .bind(Role::TaxAuthority, AccountId::TaxAuthority)
        .bind(Role::Carrier, AccountId::Carrier(carrier))
        .bind(Role::OrderBook, AccountId::OrderBook)
}

fn settlement_rate() -> Rate<Role, Asset> {
    Rate::new()
        .consume(
            Role::Buyer,
            basket([(Asset::Money, GROSS_PAYMENT), (Asset::PurchaseIntent, 1)]),
        )
        .produce(
            Role::Buyer,
            basket([(Asset::Widget, 1), (Asset::PurchaseReceipt, 1)]),
        )
        .consume(
            Role::Seller,
            basket([(Asset::Widget, 1), (Asset::SaleOffer, 1)]),
        )
        .produce(
            Role::Seller,
            basket([(Asset::Money, SELLER_PROCEEDS), (Asset::CompletedSale, 1)]),
        )
        .preserve(Role::Platform, basket([(Asset::MarketplaceLicense, 1)]))
        .produce(
            Role::Platform,
            basket([(Asset::Money, PLATFORM_COMMISSION)]),
        )
        .preserve(Role::TaxAuthority, basket([(Asset::TaxPolicy, 1)]))
        .produce(Role::TaxAuthority, basket([(Asset::Money, TAX)]))
        .consume(Role::Carrier, basket([(Asset::ShippingCapacity, 1)]))
        .produce(
            Role::Carrier,
            basket([
                (Asset::Money, SHIPPING_FEE),
                (Asset::UsedShippingCapacity, 1),
            ]),
        )
        .consume(Role::OrderBook, basket([(Asset::OpenOrder, 1)]))
        .produce(Role::OrderBook, basket([(Asset::SettledOrder, 1)]))
}

fn all_distinct(mut rate: Rate<Role, Asset>) -> Rate<Role, Asset> {
    let roles = [
        Role::Buyer,
        Role::Seller,
        Role::Platform,
        Role::TaxAuthority,
        Role::Carrier,
        Role::OrderBook,
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

        assert_eq!(candidate_matches(&world).len(), 18);
        let exact = exact_matches(&world);
        assert_eq!(exact.len(), 4);
        assert!(exact.iter().all(|exchange| world.is_applicable(exchange)));
        assert_eq!(world.state_key(), before);

        for exchange in exact {
            assert!(matches!(
                exchange.bindings().get(&Role::Buyer),
                Some(AccountId::Buyer(BuyerId::A | BuyerId::B))
            ));
            assert!(matches!(
                exchange.bindings().get(&Role::Seller),
                Some(AccountId::Seller(SellerId::A | SellerId::B))
            ));
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
        let exchange = settlement(BuyerId::C, SellerId::C, CarrierId::B);
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
                .quantity(&Asset::Widget),
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
        let exchange = settlement(BuyerId::A, SellerId::A, CarrierId::A);
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
            Quantity::new(20)
        );
        assert_eq!(
            world.balance(&AccountId::Buyer(BuyerId::A), &Asset::Widget),
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
        assert!(world.matches(&goal()));
        assert!(exact_matches(&world).is_empty());

        let mut trace = Trace::new();
        trace.push(replay_exchange);
        let replayed = initial().replayed(&trace).expect("settlement must replay");
        assert_eq!(replayed.state_key(), world.state_key());
    }

    #[test]
    fn failed_settlement_returns_all_shortfalls_and_changes_nothing() {
        let mut world = initial();
        let before = world.state_key();
        let error = world
            .apply(settlement(BuyerId::C, SellerId::C, CarrierId::B))
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
        let exchange = Exchange::new(RateId::SettleOrder, Quantity::new(1))
            .bind(Role::Buyer, AccountId::Buyer(BuyerId::A))
            .bind(Role::Seller, AccountId::Buyer(BuyerId::A))
            .bind(Role::Platform, AccountId::Platform)
            .bind(Role::TaxAuthority, AccountId::TaxAuthority)
            .bind(Role::Carrier, AccountId::Carrier(CarrierId::A))
            .bind(Role::OrderBook, AccountId::OrderBook);

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
    fn caller_owned_cost_changes_near_match_ranking() {
        let world = initial();

        let cash_friendly = rank_near_matches(&world, |assessment| {
            weighted_shortfall(assessment, 1, 100, 50)
        });
        assert_eq!(
            cash_friendly[0].candidate(),
            MarketMatch::new(BuyerId::C, SellerId::A, CarrierId::A)
        );

        let capacity_friendly = rank_near_matches(&world, |assessment| {
            weighted_shortfall(assessment, 100, 50, 1)
        });
        assert_eq!(
            capacity_friendly[0].candidate(),
            MarketMatch::new(BuyerId::A, SellerId::A, CarrierId::B)
        );
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
                    Asset::Widget => widget_weight,
                    Asset::ShippingCapacity => capacity_weight,
                    _ => 1_000,
                };
                weight * quantity.get()
            })
            .sum()
    }
}
