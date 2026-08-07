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
    let showcase = !matches!(
        instance_profile(request, descriptor),
        InstanceProfile::Micro
    );
    let initial = if showcase {
        marketplace::initial_showcase()
    } else {
        marketplace::initial()
    };
    let goal = if showcase {
        marketplace::goal_showcase()
    } else {
        marketplace::goal()
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
            "A compatible set of buyer, seller, carrier, platform, tax, and order bindings settles atomically.",
            clearing.trace().clone(),
            "market clearing",
        ),
        (
            "pareto_buyers",
            "Marketplace Pareto · buyer utility",
            "The exact non-dominated clearing favoring aggregate buyer utility.",
            buyer_trace,
            "exact Pareto clearing",
        ),
        (
            "pareto_sellers",
            "Marketplace Pareto · seller utility",
            "The exact non-dominated clearing favoring aggregate seller utility.",
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
            view.proposals.push(proposal("marketplace", ProposalSpec { id: &format!("near-match-{index}"), label: &format!("Near match {}", index + 1), description: "A complete multi-party binding ranked by caller policy; exact account shortfalls remain core-derived." }, &initial, assessed_match.exchange()));
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
        title: "Participant utility frontier".into(),
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
            x: Some(60.0),
            y: Some(60.0 + (buyer as u8 as f64) * 95.0),
        });
    }
    for seller in [SellerId::A, SellerId::B, SellerId::C] {
        nodes.push(GraphNodeView {
            id: ViewId::new(format!("seller:{seller:?}"), format!("Seller {seller:?}")),
            classes: vec!["seller".into()],
            x: Some(560.0),
            y: Some(60.0 + (seller as u8 as f64) * 95.0),
        });
    }
    nodes.push(GraphNodeView {
        id: ViewId::new("platform", "Platform + tax"),
        classes: vec!["resource".into()],
        x: Some(310.0),
        y: Some(80.0),
    });
    for carrier in [CarrierId::A, CarrierId::B] {
        nodes.push(GraphNodeView {
            id: ViewId::new(
                format!("carrier:{carrier:?}"),
                format!("Carrier {carrier:?}"),
            ),
            classes: vec!["carrier".into()],
            x: Some(310.0),
            y: Some(200.0 + (carrier as u8 as f64) * 85.0),
        });
    }
    let edges = marketplace::orders(world)
        .into_iter()
        .flat_map(|order| {
            let settled = !world
                .balance(
                    &AccountId::Order(order),
                    &marketplace::Asset::SettledOrder(order),
                )
                .is_zero();
            [
                GraphEdgeView {
                    id: format!("order:{order:?}:in"),
                    source: "buyer:A".into(),
                    target: "platform".into(),
                    label: Some(format!("Order {order:?}")),
                    classes: if settled {
                        vec!["completed".into()]
                    } else {
                        Vec::new()
                    },
                },
                GraphEdgeView {
                    id: format!("order:{order:?}:out"),
                    source: "platform".into(),
                    target: "seller:A".into(),
                    label: Some("atomic split".into()),
                    classes: if settled {
                        vec!["completed".into()]
                    } else {
                        Vec::new()
                    },
                },
            ]
        })
        .collect();
    Some(Scene::graph(
        "Candidate participants and atomic settlement flows",
        nodes,
        edges,
        None,
    ))
}
