//! How the multi-party marketplace is presented: its labels, units, scene and leaderboards.

use crate::marketplace::{self, AccountId, BuyerId, CarrierId, SellerId, World};
use axionomy::path;
use axionomy_view::{
    Format, GraphEdgeView, GraphNodeView, Keyed, Label, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, ViewId, asset_metric, link_account,
    visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let timed = marketplace::fulfillment::enabled(world);
    let mut nodes = Vec::new();
    for buyer in [BuyerId::A, BuyerId::B, BuyerId::C] {
        nodes.push(GraphNodeView {
            id: ViewId::new(path!("buyer").then(&buyer), format!("Buyer {buyer:?}")),
            classes: vec![path!("buyer")],
            x: Some(30.0),
            y: Some(60.0 + (buyer as u8 as f64) * 95.0),
        });
    }
    for seller in [SellerId::A, SellerId::B, SellerId::C] {
        nodes.push(GraphNodeView {
            id: ViewId::new(path!("seller").then(&seller), format!("Seller {seller:?}")),
            classes: vec![path!("seller")],
            x: Some(760.0),
            y: Some(60.0 + (seller as u8 as f64) * 95.0),
        });
    }
    nodes.push(GraphNodeView {
        id: ViewId::new(path!("platform"), "Platform"),
        classes: vec![path!("resource")],
        x: Some(510.0),
        y: Some(30.0),
    });
    nodes.push(GraphNodeView {
        id: ViewId::new(path!("tax"), "Tax authority"),
        classes: vec![path!("resource")],
        x: Some(650.0),
        y: Some(30.0),
    });
    for carrier in [CarrierId::A, CarrierId::B] {
        nodes.push(GraphNodeView {
            id: ViewId::new(
                path!("carrier").then(&carrier),
                format!("Carrier {carrier:?}"),
            ),
            classes: vec![path!("carrier")],
            x: Some(510.0),
            y: Some(200.0 + (carrier as u8 as f64) * 85.0),
        });
    }
    let orders = marketplace::orders(world);
    for (index, order) in orders.iter().enumerate() {
        nodes.push(GraphNodeView {
            id: ViewId::new(path!("order").then(order), format!("Order {order:?}")),
            classes: if !world
                .balance(
                    &AccountId::Order(*order),
                    &marketplace::Asset::SettledOrder(*order),
                )
                .is_zero()
            {
                vec![path!("completed")]
            } else {
                vec![path!("resource")]
            },
            x: Some(265.0),
            y: Some(55.0 + index as f64 * 78.0),
        });
    }
    let assessed = marketplace::assessed_matches(world);
    let mut matches = assessed
        .iter()
        .filter(|entry| entry.assessment().is_applicable())
        .map(|entry| entry.candidate())
        .collect::<Vec<_>>();
    for order in &orders {
        if let Some(booking) = marketplace::fulfillment::booking(world, *order) {
            matches.push(booking);
        }
        let buyer = [BuyerId::A, BuyerId::B, BuyerId::C]
            .into_iter()
            .find(|buyer| {
                !world
                    .balance(
                        &AccountId::Buyer(*buyer),
                        &marketplace::Asset::PurchaseReceipt(*order),
                    )
                    .is_zero()
            });
        let seller = [SellerId::A, SellerId::B, SellerId::C]
            .into_iter()
            .find(|seller| {
                !world
                    .balance(
                        &AccountId::Seller(*seller),
                        &marketplace::Asset::CompletedSale(*order),
                    )
                    .is_zero()
            });
        if let (Some(buyer), Some(seller)) = (buyer, seller) {
            matches.push(marketplace::MarketMatch::new(
                *order,
                buyer,
                seller,
                [CarrierId::A, CarrierId::B]
                    .into_iter()
                    .find(|carrier| {
                        world
                            .balance(
                                &AccountId::Carrier(*carrier),
                                &marketplace::Asset::DeliveredBy(*order),
                            )
                            .to_u64()
                            > 0
                    })
                    .unwrap_or(CarrierId::A),
            ));
        }
    }
    matches.sort();
    matches.dedup();
    let mut edges = Vec::new();
    for candidate in &matches {
        let settled = !world
            .balance(
                &AccountId::Order(candidate.order()),
                &marketplace::Asset::SettledOrder(candidate.order()),
            )
            .is_zero();
        let classes = if settled {
            vec![path!("completed")]
        } else {
            vec![path!("uncertain")]
        };
        edges.push(GraphEdgeView {
            id: path!("candidate")
                .then(&candidate.order())
                .then(&candidate.buyer())
                .then(&candidate.seller())
                .then(&candidate.carrier())
                .join(&path!("in")),
            source: path!("buyer").then(&candidate.buyer()),
            target: path!("order").then(&candidate.order()),
            label: Some(format!("via {:?}", candidate.carrier())),
            classes: classes.clone(),
        });
        edges.push(GraphEdgeView {
            id: path!("candidate")
                .then(&candidate.order())
                .then(&candidate.buyer())
                .then(&candidate.seller())
                .then(&candidate.carrier())
                .join(&path!("out")),
            source: path!("order").then(&candidate.order()),
            target: path!("seller").then(&candidate.seller()),
            label: Some(if settled {
                "atomic split".into()
            } else {
                "feasible match".into()
            }),
            classes,
        });
        if settled {
            for (suffix, target, label) in [
                (path!("platform"), path!("platform"), "commission"),
                (path!("tax"), path!("tax"), "tax"),
                (
                    path!("carrier"),
                    path!("carrier").then(&candidate.carrier()),
                    "shipping fee",
                ),
            ] {
                edges.push(GraphEdgeView {
                    id: path!("settlement").then(&candidate.order()).join(&suffix),
                    source: path!("order").then(&candidate.order()),
                    target,
                    label: Some(label.into()),
                    classes: vec![path!("completed")],
                });
            }
        }
    }
    let order_entities = orders.iter().map(|order| {
        let settled = !world
            .balance(
                &AccountId::Order(*order),
                &marketplace::Asset::SettledOrder(*order),
            )
            .is_zero();
        let mut entity = link_account::<marketplace::Ontology>(
            visual_entity(
                path!("order-token").then(order),
                format!("Order {order:?}"),
                SceneGlyphView::Package,
                SceneAnchorView::GraphNode {
                    node: if settled && !timed {
                        path!("platform")
                    } else {
                        path!("order").then(order)
                    },
                },
                SceneEntityRoleView::State,
                if settled {
                    SceneToneView::Success
                } else {
                    SceneToneView::Active
                },
                Some(
                    if settled {
                        "delivered and settled"
                    } else if world
                        .balance(
                            &AccountId::Order(*order),
                            &marketplace::Asset::InShipment(*order),
                        )
                        .to_u64()
                        > 0
                    {
                        "in transit; funds held"
                    } else if world
                        .balance(
                            &AccountId::Order(*order),
                            &marketplace::Asset::Contract(*order),
                        )
                        .to_u64()
                        > 0
                    {
                        "reserved; awaiting dispatch"
                    } else {
                        "awaiting match"
                    }
                    .into(),
                ),
            ),
            &AccountId::Order(*order),
        );
        if timed {
            entity.metrics = vec![
                asset_metric(
                    path!("escrow"),
                    "Funds held",
                    &marketplace::Asset::Money,
                    world.balance(&AccountId::Order(*order), &marketplace::Asset::Money),
                ),
                asset_metric(
                    path!("deadline"),
                    "Deadline",
                    &marketplace::Asset::Deadline,
                    world.balance(&AccountId::Order(*order), &marketplace::Asset::Deadline),
                ),
            ];
        }
        entity
    });
    let participant_entities = [BuyerId::A, BuyerId::B, BuyerId::C]
        .into_iter()
        .map(|buyer| {
            link_account::<marketplace::Ontology>(
                visual_entity(
                    path!("actor/buyer").then(&buyer),
                    format!("Buyer {buyer:?}"),
                    SceneGlyphView::Person,
                    SceneAnchorView::GraphNode {
                        node: path!("buyer").then(&buyer),
                    },
                    SceneEntityRoleView::State,
                    SceneToneView::Neutral,
                    Some("demand".into()),
                ),
                &AccountId::Buyer(buyer),
            )
        })
        .chain(
            [SellerId::A, SellerId::B, SellerId::C]
                .into_iter()
                .map(|seller| {
                    link_account::<marketplace::Ontology>(
                        visual_entity(
                            path!("actor/seller").then(&seller),
                            format!("Seller {seller:?}"),
                            SceneGlyphView::Organization,
                            SceneAnchorView::GraphNode {
                                node: path!("seller").then(&seller),
                            },
                            SceneEntityRoleView::State,
                            SceneToneView::Neutral,
                            Some("supply".into()),
                        ),
                        &AccountId::Seller(seller),
                    )
                }),
        )
        .chain([CarrierId::A, CarrierId::B].into_iter().map(|carrier| {
            link_account::<marketplace::Ontology>(
                visual_entity(
                    path!("actor/carrier").then(&carrier),
                    format!("Carrier {carrier:?}"),
                    SceneGlyphView::Vehicle,
                    SceneAnchorView::GraphNode {
                        node: path!("carrier").then(&carrier),
                    },
                    SceneEntityRoleView::State,
                    SceneToneView::Neutral,
                    Some("capacity".into()),
                ),
                &AccountId::Carrier(carrier),
            )
        }))
        .chain(
            [
                (
                    path!("actor/platform"),
                    "Platform",
                    path!("platform"),
                    AccountId::Platform,
                ),
                (
                    path!("actor/tax"),
                    "Tax authority",
                    path!("tax"),
                    AccountId::TaxAuthority,
                ),
            ]
            .into_iter()
            .map(|(id, label, node, account)| {
                link_account::<marketplace::Ontology>(
                    visual_entity(
                        id,
                        label,
                        SceneGlyphView::Organization,
                        SceneAnchorView::GraphNode { node },
                        SceneEntityRoleView::State,
                        SceneToneView::Neutral,
                        Some("settlement role".into()),
                    ),
                    &account,
                )
            }),
        );
    let settled = orders
        .iter()
        .filter(|order| {
            !world
                .balance(
                    &AccountId::Order(**order),
                    &marketplace::Asset::SettledOrder(**order),
                )
                .is_zero()
        })
        .count();
    let mut scene = Scene::graph("Contracts, shipping and settlement", nodes, edges, None)
        .with_entities(order_entities.chain(participant_entities))
        .with_metrics([
            visual_metric(
                path!("open"),
                "Open orders",
                orders.len() - settled,
                Some("orders"),
            ),
            visual_metric(path!("settled"), "Settled orders", settled, Some("orders")),
            visual_metric(
                path!("feasible"),
                "Feasible bindings",
                assessed
                    .iter()
                    .filter(|entry| entry.assessment().is_applicable())
                    .count(),
                Some("matches"),
            ),
        ]);
    if timed {
        scene.metrics.extend([
            asset_metric(
                path!("world-time"),
                "World time",
                &marketplace::Asset::Now,
                marketplace::fulfillment::now(world),
            ),
            asset_metric(
                path!("escrow"),
                "Funds held in contracts",
                &marketplace::Asset::Money,
                orders
                    .iter()
                    .map(|order| {
                        world
                            .balance(&AccountId::Order(*order), &marketplace::Asset::Money)
                            .to_u64()
                    })
                    .sum::<u64>(),
            ),
            visual_metric(
                path!("in-transit"),
                "Shipments underway",
                orders
                    .iter()
                    .map(|order| {
                        world
                            .balance(
                                &AccountId::Order(*order),
                                &marketplace::Asset::InShipment(*order),
                            )
                            .to_u64()
                    })
                    .sum::<u64>(),
                None,
            ),
            visual_metric(
                path!("ontime"),
                "On-time deliveries",
                orders
                    .iter()
                    .map(|order| {
                        world
                            .balance(&AccountId::Order(*order), &marketplace::Asset::OnTime)
                            .to_u64()
                    })
                    .sum::<u64>(),
                None,
            ),
            visual_metric(
                path!("late"),
                "Late deliveries",
                orders
                    .iter()
                    .map(|order| {
                        world
                            .balance(&AccountId::Order(*order), &marketplace::Asset::Late)
                            .to_u64()
                    })
                    .sum::<u64>(),
                None,
            ),
        ]);
    }
    Some(scene)
}

// How Studio names and writes this problem's values.

/// Money reads in credits, the unit of every price and concession; the
/// platform's clock and every time it keeps read in ticks.
impl Format for marketplace::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Money => QuantityFormat::COUNT.unit("credits"),
            Self::Arrival
            | Self::Deadline
            | Self::LeaseUntil
            | Self::ReadyAt
            | Self::Now
            | Self::TimeRemaining
            | Self::TransitTime
            | Self::RouteDelay => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

/// How a contract's price was agreed: in full, or with the 10-credit
/// discount.
fn priced(discounted: bool) -> &'static str {
    if discounted {
        "discounted"
    } else {
        "full-price"
    }
}

impl Label for marketplace::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Buyer(id) => format!("Buyer {id:?}"),
            Self::Seller(id) => format!("Seller {id:?}"),
            Self::Platform => "Marketplace platform".into(),
            Self::TaxAuthority => "Tax authority".into(),
            Self::Carrier(id) => format!("Carrier {id:?}"),
            Self::Order(id) => format!("Order {id:?}"),
        }
    }
}

impl Label for marketplace::Asset {
    fn label(&self) -> String {
        match self {
            Self::Money => "Money".into(),
            Self::Item(item) => format!("{item:?} inventory"),
            Self::PurchaseIntent(order) => format!("Wants Order {order:?}"),
            Self::PurchaseReceipt(order) => format!("Bought Order {order:?}"),
            Self::SaleOffer(marketplace::Item::Widget) => "Offers one widget".into(),
            Self::SaleOffer(marketplace::Item::Gadget) => "Offers one gadget".into(),
            Self::CompletedSale(order) => format!("Sold Order {order:?}"),
            Self::MarketplaceLicense => "Marketplace licence".into(),
            Self::TaxPolicy => "Tax policy".into(),
            Self::ShippingCapacity => "Shipping capacity left".into(),
            Self::UsedShippingCapacity => "Shipping capacity used".into(),
            Self::OpenOrder(order) => format!("Order {order:?} is open"),
            Self::SettledOrder(order) => format!("Order {order:?} settled"),
            Self::SettledValue(value) => format!("Settled for {value} credits"),
            Self::Utility => "Participant benefit".into(),
            Self::BuyerLicense => "Buyer identity".into(),
            Self::SellerLicense => "Seller identity".into(),
            Self::CarrierLicense => "Carrier identity".into(),
            Self::Contract(order) => format!("Order {order:?} reserved"),
            Self::InShipment(order) => format!("Order {order:?} in transit"),
            Self::PurchaseLocked(order) => format!("Buyer committed to Order {order:?}"),
            Self::SaleReserved(order) => format!("Seller committed to Order {order:?}"),
            Self::Booking(order) => format!("Carrier booked for Order {order:?}"),
            Self::DeliveredBy(order) => format!("Delivered Order {order:?}"),
            Self::AgreedPrice(price) => format!("Agreed price of {price} credits"),
            Self::Arrival => "Order release time".into(),
            Self::Deadline => "Delivery deadline".into(),
            Self::LeaseUntil => "Dispatch reservation expires at".into(),
            Self::ReadyAt => "Delivery available at".into(),
            Self::Now => "World time".into(),
            Self::TimeRemaining => "Time remaining".into(),
            Self::TransitTime => "Base shipping duration".into(),
            Self::DelayRisk => "Exposure to route delays".into(),
            Self::RouteDelay => "Route delay".into(),
            Self::OnTime => "On-time deliveries".into(),
            Self::Late => "Late deliveries".into(),
            Self::Refunded => "Expired reservations refunded".into(),
        }
    }
}

impl Label for marketplace::RateId {
    fn label(&self) -> String {
        match self {
            Self::SettleOrder(order) => format!("Agree Order {order:?} at full price"),
            Self::DiscountedContract(order) => {
                format!("Agree Order {order:?} at a 10-credit discount")
            }
            Self::Dispatch(order) => format!("Dispatch Order {order:?}"),
            Self::Deliver {
                order,
                discounted,
                late,
            } => format!(
                "Deliver {} Order {order:?} {}",
                priced(*discounted),
                if *late {
                    "late with a rebate"
                } else {
                    "on time"
                }
            ),
            Self::Cancel { order, discounted } => format!(
                "Refund the expired {} reservation for Order {order:?}",
                priced(*discounted)
            ),
            Self::Advance { to } => format!("Advance market time to {to}"),
        }
    }
}

impl Label for marketplace::Role {
    fn label(&self) -> String {
        match self {
            Self::Buyer => "Buyer",
            Self::Seller => "Seller",
            Self::Platform => "Platform",
            Self::TaxAuthority => "Tax authority",
            Self::Carrier => "Carrier",
            Self::Order => "Order",
        }
        .into()
    }
}
