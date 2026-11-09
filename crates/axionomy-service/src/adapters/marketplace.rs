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
    let timed = marketplace::fulfillment::enabled(&initial);
    let pareto = (!timed)
        .then(|| marketplace::pareto_front(&initial))
        .transpose()
        .map_err(|error| problem_error("marketplace", error))?;
    let buyer_trace = if let Some(front) = &pareto {
        pareto_trace(front, true)?
    } else {
        marketplace::fulfillment::plan(&initial, marketplace::fulfillment::Preference::Buyers)
            .ok_or_else(|| problem_error("marketplace", "buyer fulfillment search exhausted"))?
    };
    let seller_trace = if let Some(front) = &pareto {
        pareto_trace(front, false)?
    } else {
        marketplace::fulfillment::plan(&initial, marketplace::fulfillment::Preference::Sellers)
            .ok_or_else(|| problem_error("marketplace", "seller fulfillment search exhausted"))?
    };
    let traces = [
        (
            "market_clearing",
            "Marketplace · compatible clearing",
            "Micro settles directly. Larger markets reserve funds and goods, dispatch through occupied carriers, then settle on delivery. Delays can trigger buyer rebates.",
            clearing.trace().clone(),
            if timed {
                "bounded fulfillment search"
            } else {
                "market clearing"
            },
        ),
        (
            "pareto_buyers",
            "Marketplace Pareto · buyer utility",
            "Choose price concessions and delivery allocations for buyer benefit. Larger markets use a bounded beam of 24 states and compare evaluated plans.",
            buyer_trace,
            if timed {
                "bounded fulfillment search"
            } else {
                "exact Pareto clearing"
            },
        ),
        (
            "pareto_sellers",
            "Marketplace Pareto · seller utility",
            "Choose contract prices and delivery allocations for seller benefit under the same arrivals, routes and available funds.",
            seller_trace,
            if timed {
                "bounded fulfillment search"
            } else {
                "exact Pareto clearing"
            },
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
        if let Some(front) = &pareto {
            view.pareto_fronts.push(front_view(front, &view));
        }
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
                    "verified exchanges".into(),
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
    if timed {
        for index in 0..documents.len() {
            let front = evaluated_front(&documents, &documents[index]);
            documents[index].pareto_fronts.push(front);
        }
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
    let mut values: Vec<_> = [
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
    .collect();
    if marketplace::fulfillment::enabled(world) {
        for (key, label, account) in [
            ("buyer_c", "Buyer C utility", AccountId::Buyer(BuyerId::C)),
            (
                "seller_c",
                "Seller C utility",
                AccountId::Seller(SellerId::C),
            ),
        ] {
            values.push(ObjectiveView {
                key: key.into(),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
                value: marketplace::utility(world, account).to_string(),
            });
        }
        values.push(ObjectiveView {
            key: "ontime".into(),
            label: "On-time deliveries".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: marketplace::orders(world)
                .into_iter()
                .map(|order| {
                    world
                        .balance(&AccountId::Order(order), &marketplace::Asset::OnTime)
                        .to_u64()
                })
                .sum::<u64>()
                .to_string(),
        });
        values.push(ObjectiveView {
            key: "elapsed".into(),
            label: "Elapsed time".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: marketplace::fulfillment::now(world).to_string(),
        });
    }
    values
}
fn evaluated_front(documents: &[ViewDocument], selected: &ViewDocument) -> ParetoFrontView {
    let rows: Vec<_> = documents
        .iter()
        .map(|document| {
            (
                document,
                document
                    .objectives
                    .iter()
                    .map(|o| o.value.parse::<u64>().unwrap())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let dominates = |left: &[u64], right: &[u64]| {
        left.iter()
            .zip(right)
            .zip(&selected.objectives)
            .all(|((l, r), axis)| match axis.direction {
                ObjectiveDirectionView::Maximize => l >= r,
                ObjectiveDirectionView::Minimize => l <= r,
            })
            && left
                .iter()
                .zip(right)
                .zip(&selected.objectives)
                .any(|((l, r), axis)| match axis.direction {
                    ObjectiveDirectionView::Maximize => l > r,
                    ObjectiveDirectionView::Minimize => l < r,
                })
    };
    ParetoFrontView {
        title: "Non-dominated evaluated fulfillment plans".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: selected
            .objectives
            .iter()
            .map(|o| ObjectiveAxisView {
                key: o.key.clone(),
                label: o.label.clone(),
                direction: o.direction,
            })
            .collect(),
        points: rows
            .iter()
            .filter(|(_, candidate)| !rows.iter().any(|(_, other)| dominates(other, candidate)))
            .map(|(document, values)| ParetoPointView {
                label: document.title.clone(),
                selected: document.id == selected.id,
                values: values.iter().map(ToString::to_string).collect(),
            })
            .collect(),
    }
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
    let timed = marketplace::fulfillment::enabled(world);
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
        let mut entity = link_account(
            visual_entity(
                format!("order-token:{order:?}"),
                format!("Order {order:?}"),
                SceneGlyphView::Package,
                SceneAnchorView::GraphNode {
                    node: if settled && !timed {
                        "platform".into()
                    } else {
                        format!("order:{order:?}")
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
            format!("marketplace:account:order-{order:?}").to_ascii_lowercase(),
        );
        if timed {
            entity.metrics = vec![
                visual_metric(
                    "escrow",
                    "Funds held",
                    world.balance(&AccountId::Order(*order), &marketplace::Asset::Money),
                    None,
                ),
                visual_metric(
                    "deadline",
                    "Deadline",
                    world.balance(&AccountId::Order(*order), &marketplace::Asset::Deadline),
                    Some("ticks"),
                ),
            ];
        }
        entity
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
                    SceneEntityRoleView::State,
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
                            SceneEntityRoleView::State,
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
                    SceneEntityRoleView::State,
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
                        SceneEntityRoleView::State,
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
    let mut scene = Scene::graph("Contracts, shipping and settlement", nodes, edges, None)
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
        ]);
    if timed {
        scene.metrics.extend([
            visual_metric(
                "world_time",
                "World time",
                marketplace::fulfillment::now(world),
                Some("ticks"),
            ),
            visual_metric(
                "escrow",
                "Funds held in contracts",
                orders
                    .iter()
                    .map(|order| {
                        world
                            .balance(&AccountId::Order(*order), &marketplace::Asset::Money)
                            .to_u64()
                    })
                    .sum::<u64>(),
                None,
            ),
            visual_metric(
                "in_transit",
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
                "ontime",
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
                "late",
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
