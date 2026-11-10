//! The multi-party marketplace as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    marketplace::{self, AccountId, BuyerId, ObjectiveKey, SellerId, World},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::path;
use axionomy_study::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProblemPath, ProposalSpec, StudyError, StudyKey, StudyProgress, document, problem_error,
    proposal, telemetry,
};
use axionomy_view::{
    Keyed, ObjectiveAxisView, ObjectiveDirectionView, ObjectiveView, ParetoFrontView,
    TelemetryKindView, ViewDocument, evaluated_front, exact_front, exact_values,
};

/// Multi-party marketplace: its catalog entry and how its studies are computed.
pub struct Marketplace {
    descriptor: ProblemDescriptor,
}

impl Default for Marketplace {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("marketplace"),
                    title: "Multi-party marketplace",
                    summary: "Negotiate funded contracts, reserve scarce shipping capacity and fulfill deliveries over time. Route delays, price concessions, late rebates and refundable reservations couple buyer and seller outcomes.",
                    instances: [
                        "Two orders across three buyers, three sellers, and one active carrier",
                        "Four arriving orders with escrow, two occupied carriers, delivery deadlines and price concessions",
                        "Six arriving orders with scarce stock, persistent budgets, route delays and reusable carrier capacity",
                    ],
                },
                ProblemFamily::Market,
                path!("market-clearing"),
                &[
                    StrategyCopy {
                        key: path!("market-clearing"),
                        label: "Clear the market",
                        description: "Pick orders that can all settle together without overdrawing any account.",
                        algorithm: "market clearing",
                    },
                    StrategyCopy {
                        key: path!("pareto-buyers"),
                        label: "Pareto: favors buyers",
                        description: "Micro: exact clearing frontier. Larger markets: bounded fulfillment search favoring buyer benefit.",
                        algorithm: "exact Pareto clearing",
                    },
                    StrategyCopy {
                        key: path!("pareto-sellers"),
                        label: "Pareto: favors sellers",
                        description: "Micro: exact clearing frontier. Larger markets: bounded fulfillment search favoring seller benefit.",
                        algorithm: "exact Pareto clearing",
                    },
                ],
                &[
                    Capability::AtomicSettlement,
                    Capability::MultiAccountExchange,
                    Capability::FeasibilityAssessment,
                    Capability::ExactPareto,
                ],
            ),
        }
    }
}

impl Problem for Marketplace {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, StudyError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let profile = study.instance;
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
        .map_err(|error| problem_error(&study.problem, error))?;
    let buyer_trace = if let Some(front) = &pareto {
        pareto_trace(front, true, &study.problem)?
    } else {
        marketplace::fulfillment::plan(&initial, marketplace::fulfillment::Preference::Buyers)
            .ok_or_else(|| problem_error(&study.problem, "buyer fulfillment search exhausted"))?
    };
    let seller_trace = if let Some(front) = &pareto {
        pareto_trace(front, false, &study.problem)?
    } else {
        marketplace::fulfillment::plan(&initial, marketplace::fulfillment::Preference::Sellers)
            .ok_or_else(|| problem_error(&study.problem, "seller fulfillment search exhausted"))?
    };
    let traces = [
        (
            path!("market-clearing"),
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
            path!("pareto-buyers"),
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
            path!("pareto-sellers"),
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
            .map_err(|error| problem_error(&study.problem, error))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
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
        .map_err(|error| problem_error(&study.problem, error))?;
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
            view.proposals.push(proposal(ProposalSpec { id: path!("near-match").then(&index), label: &format!("Near match {}", index + 1), description: "A complete set of participants that almost settles; the rejection names the exact account and amount that came up short." }, &initial, assessed_match.exchange()));
        }
        documents.push(view);
    }
    if timed {
        for index in 0..documents.len() {
            let front = evaluated_front(
                &documents,
                &documents[index],
                "Non-dominated evaluated fulfillment plans",
            );
            documents[index].pareto_fronts.push(front);
        }
    }
    Ok(documents)
}

fn pareto_trace(
    result: &marketplace::ParetoResult,
    buyers: bool,
    problem: &ProblemPath,
) -> Result<axionomy::Trace<marketplace::Ontology>, StudyError> {
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
        .ok_or_else(|| problem_error(problem, "empty Pareto frontier"))
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    let mut values: Vec<_> = [
        (path!("buyer-a"), "Buyer A", AccountId::Buyer(BuyerId::A)),
        (path!("buyer-b"), "Buyer B", AccountId::Buyer(BuyerId::B)),
        (
            path!("seller-a"),
            "Seller A",
            AccountId::Seller(SellerId::A),
        ),
        (
            path!("seller-b"),
            "Seller B",
            AccountId::Seller(SellerId::B),
        ),
    ]
    .into_iter()
    .map(|(key, label, account)| {
        ObjectiveView::new(
            key,
            format!("{label} utility"),
            ObjectiveDirectionView::Maximize,
            marketplace::utility(world, account),
        )
    })
    .collect();
    if marketplace::fulfillment::enabled(world) {
        for (key, label, account) in [
            (
                path!("buyer-c"),
                "Buyer C utility",
                AccountId::Buyer(BuyerId::C),
            ),
            (
                path!("seller-c"),
                "Seller C utility",
                AccountId::Seller(SellerId::C),
            ),
        ] {
            values.push(ObjectiveView::new(
                key,
                label,
                ObjectiveDirectionView::Maximize,
                marketplace::utility(world, account),
            ));
        }
        values.push(ObjectiveView::new(
            path!("ontime"),
            "On-time deliveries",
            ObjectiveDirectionView::Maximize,
            marketplace::orders(world)
                .into_iter()
                .map(|order| {
                    world
                        .balance(&AccountId::Order(order), &marketplace::Asset::OnTime)
                        .to_u64()
                })
                .sum::<u64>(),
        ));
        values.push(ObjectiveView::new(
            path!("elapsed"),
            "Elapsed time",
            ObjectiveDirectionView::Minimize,
            marketplace::fulfillment::now(world),
        ));
    }
    values
}

fn front_view(result: &marketplace::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    exact_front(
        "Buyer gain vs. seller gain",
        ["Buyer A", "Buyer B", "Seller A", "Seller B"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| ObjectiveAxisView {
                key: path!("utility").then(&index),
                label: label.into(),
                direction: ObjectiveDirectionView::Maximize,
            })
            .collect(),
        selected,
        result.front().entries().iter().map(exact_values),
        |values| {
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" / ")
        },
    )
}
