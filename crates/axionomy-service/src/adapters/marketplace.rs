use super::*;
use axionomy_problems::marketplace::{
    self, AccountId, BuyerId, CarrierId, ObjectiveKey, SellerId, World,
};
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let profile = instance_profile(request, descriptor);
    let initial = match profile {
        InstanceProfile::Micro => marketplace::initial(),
        InstanceProfile::Showcase => marketplace::initial_showcase(),
        InstanceProfile::Stress => marketplace::initial_stress(),
    };
    let goal = match profile {
        InstanceProfile::Micro => marketplace::goal(),
        InstanceProfile::Showcase => marketplace::goal_showcase(),
        InstanceProfile::Stress => marketplace::goal_stress(),
    };
    let clearing = marketplace::clear_market(&initial);
    let pareto =
        marketplace::pareto_front(&initial).map_err(|error| problem_error("marketplace", error))?;
    let buyer_trace = pareto_trace(&pareto, true)?;
    let seller_trace = pareto_trace(&pareto, false)?;
    let traces = [
        (
            "market_clearing",
            "Marketplace · compatible clearing",
            "A set of orders that can all settle together; buyer, seller, carrier, platform, and tax move in one step.",
            clearing.trace().clone(),
            "market clearing",
        ),
        (
            "pareto_buyers",
            "Marketplace Pareto · buyer utility",
            "The clearing on the frontier with the greatest total gain for buyers.",
            buyer_trace,
            "exact Pareto clearing",
        ),
        (
            "pareto_sellers",
            "Marketplace Pareto · seller utility",
            "The clearing on the frontier with the greatest total gain for sellers.",
            seller_trace,
            "exact Pareto clearing",
        ),
    ];
    let assessed = marketplace::assessed_matches(&initial);
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("marketplace", error))?;
        let mut view = document(
            DocumentSpec {
                problem: "marketplace",
                strategy,
                title,
                description,
                source_label: "Multi-party marketplace",
            },
            &initial,
            &goal,
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("marketplace", error))?;
        view.pareto_fronts.push(front_view(&pareto, &view));
        view.telemetry.push(telemetry(
            algorithm,
            true,
            [
                (
                    TelemetryKindView::Generated,
                    marketplace::candidate_matches(&initial).len() as u64,
                    "candidate bindings assessed".into(),
                ),
                (
                    TelemetryKindView::Expanded,
                    trace.exchanges().len() as u64,
                    "settled orders".into(),
                ),
            ],
        ));
        for (index, assessed_match) in assessed
            .iter()
            .filter(|entry| !entry.assessment().is_applicable())
            .take(3)
            .enumerate()
        {
            view.proposals.push(proposal("marketplace", ProposalSpec { id: &format!("near-match-{index}"), label: &format!("Near match {}", index + 1), description: "A complete set of participants that almost settles; the rejection names the exact account and amount that came up short." }, &initial, assessed_match.exchange()));
        }
        documents.push(view);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn pareto_trace(
    result: &marketplace::ParetoResult,
    buyers: bool,
) -> Result<axionomy::Trace<marketplace::RateId, marketplace::Role, AccountId>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .max_by_key(|entry| {
            entry
                .objectives()
                .objectives()
                .iter()
                .filter(|objective| {
                    matches!(
                        (buyers, objective.key()),
                        (true, ObjectiveKey::Buyer(_)) | (false, ObjectiveKey::Seller(_))
                    )
                })
                .map(|objective| *objective.value())
                .sum::<u64>()
        })
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error("marketplace", "empty Pareto frontier"))
}
fn objectives(world: &World) -> Vec<ObjectiveView> {
    [
        ("buyer_a", "Buyer A", AccountId::Buyer(BuyerId::A)),
        ("buyer_b", "Buyer B", AccountId::Buyer(BuyerId::B)),
        ("seller_a", "Seller A", AccountId::Seller(SellerId::A)),
        ("seller_b", "Seller B", AccountId::Seller(SellerId::B)),
    ]
    .into_iter()
    .map(|(key, label, account)| ObjectiveView {
        key: key.into(),
        label: format!("{label} utility"),
        direction: ObjectiveDirectionView::Maximize,
        value: marketplace::utility(world, account).to_string(),
    })
    .collect()
}
fn front_view(result: &marketplace::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|o| o.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Buyer gain vs. seller gain".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: ["Buyer A", "Buyer B", "Seller A", "Seller B"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| ObjectiveAxisView {
                key: format!("utility_{index}"),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
            })
            .collect(),
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|o| o.value().to_string())
                    .collect::<Vec<_>>();
                ParetoPointView {
                    label: values.join(" / "),
                    selected: values
                        .iter()
                        .map(String::as_str)
                        .eq(selected.iter().copied()),
                    values,
                }
            })
            .collect(),
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let mut nodes = Vec::new();
    for buyer in [BuyerId::A, BuyerId::B, BuyerId::C] {
        nodes.push(GraphNodeView {
            id: ViewId::new(format!("buyer:{buyer:?}"), format!("Buyer {buyer:?}")),
            classes: vec!["buyer".into()],
            x: Some(30.0),
            y: Some(60.0 + (buyer as u8 as f64) * 95.0),
        });
    }
    for seller in [SellerId::A, SellerId::B, SellerId::C] {
        nodes.push(GraphNodeView {
            id: ViewId::new(format!("seller:{seller:?}"), format!("Seller {seller:?}")),
            classes: vec!["seller".into()],
            x: Some(760.0),
            y: Some(60.0 + (seller as u8 as f64) * 95.0),
        });
    }
    nodes.push(GraphNodeView {
        id: ViewId::new("platform", "Platform"),
        classes: vec!["resource".into()],
        x: Some(510.0),
        y: Some(30.0),
    });
    nodes.push(GraphNodeView {
        id: ViewId::new("tax", "Tax authority"),
        classes: vec!["resource".into()],
        x: Some(650.0),
        y: Some(30.0),
    });
    for carrier in [CarrierId::A, CarrierId::B] {
        nodes.push(GraphNodeView {
            id: ViewId::new(
                format!("carrier:{carrier:?}"),
                format!("Carrier {carrier:?}"),
            ),
            classes: vec!["carrier".into()],
            x: Some(510.0),
            y: Some(200.0 + (carrier as u8 as f64) * 85.0),
        });
    }
    let orders = marketplace::orders(world);
    for (index, order) in orders.iter().enumerate() {
        nodes.push(GraphNodeView {
            id: ViewId::new(format!("order:{order:?}"), format!("Order {order:?}")),
            classes: if !world
                .balance(
                    &AccountId::Order(*order),
                    &marketplace::Asset::SettledOrder(*order),
                )
                .is_zero()
            {
                vec!["completed".into()]
            } else {
                vec!["resource".into()]
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
                CarrierId::A,
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
            vec!["completed".into()]
        } else {
            vec!["uncertain".into()]
        };
        edges.push(GraphEdgeView {
            id: format!(
                "candidate:{:?}:{:?}:{:?}:{:?}:in",
                candidate.order(),
                candidate.buyer(),
                candidate.seller(),
                candidate.carrier()
            ),
            source: format!("buyer:{:?}", candidate.buyer()),
            target: format!("order:{:?}", candidate.order()),
            label: Some(format!("via {:?}", candidate.carrier())),
            classes: classes.clone(),
        });
        edges.push(GraphEdgeView {
            id: format!(
                "candidate:{:?}:{:?}:{:?}:{:?}:out",
                candidate.order(),
                candidate.buyer(),
                candidate.seller(),
                candidate.carrier()
            ),
            source: format!("order:{:?}", candidate.order()),
            target: format!("seller:{:?}", candidate.seller()),
            label: Some(if settled {
                "atomic split".into()
            } else {
                "feasible match".into()
            }),
            classes,
        });
        if settled {
            for (suffix, target, label) in [
                ("platform", "platform".to_owned(), "commission"),
                ("tax", "tax".to_owned(), "tax"),
                (
                    "carrier",
                    format!("carrier:{:?}", candidate.carrier()),
                    "shipping fee",
                ),
            ] {
                edges.push(GraphEdgeView {
                    id: format!("settlement:{:?}:{suffix}", candidate.order()),
                    source: format!("order:{:?}", candidate.order()),
                    target,
                    label: Some(label.into()),
                    classes: vec!["completed".into()],
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
        link_account(
            visual_entity(
                format!("order-token:{order:?}"),
                format!("Order {order:?}"),
                SceneGlyphView::Package,
                SceneAnchorView::GraphNode {
                    node: if settled {
                        "platform".into()
                    } else {
                        format!("order:{order:?}")
                    },
                },
                if settled {
                    SceneToneView::Success
                } else {
                    SceneToneView::Active
                },
                Some(
                    if settled {
                        "cleared atomically"
                    } else {
                        "awaiting match"
                    }
                    .into(),
                ),
            ),
            format!("marketplace:account:order-{order:?}").to_ascii_lowercase(),
        )
    });
    let participant_entities = [BuyerId::A, BuyerId::B, BuyerId::C]
        .into_iter()
        .map(|buyer| {
            link_account(
                visual_entity(
                    format!("actor:buyer:{buyer:?}"),
                    format!("Buyer {buyer:?}"),
                    SceneGlyphView::Person,
                    SceneAnchorView::GraphNode {
                        node: format!("buyer:{buyer:?}"),
                    },
                    SceneToneView::Neutral,
                    Some("demand".into()),
                ),
                format!("marketplace:account:buyer-{buyer:?}").to_ascii_lowercase(),
            )
        })
        .chain(
            [SellerId::A, SellerId::B, SellerId::C]
                .into_iter()
                .map(|seller| {
                    link_account(
                        visual_entity(
                            format!("actor:seller:{seller:?}"),
                            format!("Seller {seller:?}"),
                            SceneGlyphView::Organization,
                            SceneAnchorView::GraphNode {
                                node: format!("seller:{seller:?}"),
                            },
                            SceneToneView::Neutral,
                            Some("supply".into()),
                        ),
                        format!("marketplace:account:seller-{seller:?}").to_ascii_lowercase(),
                    )
                }),
        )
        .chain([CarrierId::A, CarrierId::B].into_iter().map(|carrier| {
            link_account(
                visual_entity(
                    format!("actor:carrier:{carrier:?}"),
                    format!("Carrier {carrier:?}"),
                    SceneGlyphView::Vehicle,
                    SceneAnchorView::GraphNode {
                        node: format!("carrier:{carrier:?}"),
                    },
                    SceneToneView::Neutral,
                    Some("capacity".into()),
                ),
                format!("marketplace:account:carrier-{carrier:?}").to_ascii_lowercase(),
            )
        }))
        .chain(
            [
                (
                    "actor:platform",
                    "Platform",
                    "platform",
                    "marketplace:account:platform",
                ),
                (
                    "actor:tax",
                    "Tax authority",
                    "tax",
                    "marketplace:account:taxauthority",
                ),
            ]
            .into_iter()
            .map(|(id, label, node, account)| {
                link_account(
                    visual_entity(
                        id,
                        label,
                        SceneGlyphView::Organization,
                        SceneAnchorView::GraphNode { node: node.into() },
                        SceneToneView::Neutral,
                        Some("settlement role".into()),
                    ),
                    account,
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
    Some(
        Scene::graph("Who pays whom, if this clears", nodes, edges, None)
            .with_entities(order_entities.chain(participant_entities))
            .with_metrics([
                visual_metric(
                    "open",
                    "Open orders",
                    orders.len() - settled,
                    Some("orders"),
                ),
                visual_metric("settled", "Settled orders", settled, Some("orders")),
                visual_metric(
                    "feasible",
                    "Feasible bindings",
                    assessed
                        .iter()
                        .filter(|entry| entry.assessment().is_applicable())
                        .count(),
                    Some("matches"),
                ),
            ]),
    )
}
