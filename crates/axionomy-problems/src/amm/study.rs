//! The Living Market as a study: its catalog entry, strategies and documents.

use super::view::{
    attach_contribution_leaderboards, causal_contribution_board, defaults, scene, unmet_needs,
};
use crate::{
    amm::{self, ACTORS, AccountId, Actor, Asset, Scenario, World},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::path;
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, ServiceError, StudyKey, StudyProgress, document, problem_error, proposal,
    telemetry,
};
use axionomy_view::{ObjectiveDirectionView, ObjectiveView, TelemetryKindView, ViewDocument};

/// The Living Market: its catalog entry and how its studies are computed.
pub struct Amm {
    descriptor: ProblemDescriptor,
}

impl Default for Amm {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("amm"),
                    title: "The Living Market",
                    summary: "Watch repeated production and trade reshape a closed economy. Daily needs and obligations, cloudy periods, limited storage, delayed capacity investment, and responsive liquidity interact through an exact constant-product AMM. Missed needs and defaults remain visible.",
                    instances: [
                        "A compact closed market that exposes every reserve calculation and refusal",
                        "Eight market days with recurring needs, finite credit, production investment, and responsive trading",
                        "Twelve market days at four times the production scale with the same seeded sunlight across counterfactuals",
                    ],
                },
                ProblemFamily::Market,
                path!("market-day"),
                &[
                    StrategyCopy {
                        key: path!("market-day"),
                        label: "Responsive market",
                        description: "Actors revisit production, trade, investment, and liquidity decisions throughout the market season.",
                        algorithm: "verified autonomous market replay",
                    },
                    StrategyCopy {
                        key: path!("no-whale"),
                        label: "Counterfactual: no whale",
                        description: "Replay the same economy without its largest buyer and inspect the causal price difference.",
                        algorithm: "counterfactual replay",
                    },
                    StrategyCopy {
                        key: path!("thin-liquidity"),
                        label: "Counterfactual: thin liquidity",
                        description: "Withdraw half the founding liquidity and inspect how the same actors adapt to different depth and prices.",
                        algorithm: "counterfactual replay",
                    },
                ],
                &[
                    Capability::AtomicSettlement,
                    Capability::MultiAccountExchange,
                    Capability::FeasibilityAssessment,
                    Capability::ReplayDerivedLeaderboards,
                    Capability::NonFungibleFacts,
                ],
            ),
        }
    }
}

impl Problem for Amm {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, ServiceError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let initial = match study.instance {
        InstanceProfile::Micro => amm::initial(),
        InstanceProfile::Showcase => amm::initial_showcase_with_seed(study.seed),
        InstanceProfile::Stress => amm::initial_stress_with_seed(study.seed),
    };
    let scenarios = [
        (
            path!("market-day"),
            Scenario::MarketDay,
            "Living Market · endogenous price discovery",
            "Actors repeatedly reconsider trades as production, inventory, daily needs and obligations change. Capacity investment takes two days; scarce storage and cloudy days change the cost of waiting. No external price exists.",
        ),
        (
            path!("no-whale"),
            Scenario::NoWhale,
            "Living Market · without the demand whale",
            "The same closed economy replays without its largest buyer, exposing that actor's causal effect on the discovered exchange value.",
        ),
        (
            path!("thin-liquidity"),
            Scenario::ThinLiquidity,
            "Living Market · thin-liquidity reality",
            "Half the founding liquidity exits at the opening. Actors respond to the changed prices and depth across the same production conditions and daily obligations.",
        ),
    ];

    let mut documents = Vec::new();
    for (strategy, scenario, title, description) in scenarios {
        let trace = amm::trace(&initial, scenario);
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error(&study.problem, format!("{error:?}")))?;
        let final_pool = amm::pool_state(&final_world);
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
                title,
                description,
                source_label: "Closed endogenous market",
            },
            &initial,
            &amm::market_goal(&initial),
            &trace,
            vec![
                ObjectiveView::new(
                    path!("needs"),
                    "Needs satisfied",
                    ObjectiveDirectionView::Maximize,
                    satisfied_needs(&final_world),
                ),
                ObjectiveView::new(
                    path!("utility"),
                    "Realized utility",
                    ObjectiveDirectionView::Maximize,
                    total_utility(&final_world),
                ),
                ObjectiveView::new(
                    path!("depth"),
                    "Energy liquidity",
                    ObjectiveDirectionView::Maximize,
                    final_pool.energy,
                ),
                ObjectiveView::new(
                    path!("unmet"),
                    "Unmet daily needs",
                    ObjectiveDirectionView::Minimize,
                    unmet_needs(&final_world),
                ),
                ObjectiveView::new(
                    path!("defaults"),
                    "Missed obligations",
                    ObjectiveDirectionView::Minimize,
                    defaults(&final_world),
                ),
            ],
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        attach_contribution_leaderboards(&initial, &trace, &mut view, &study.problem)?;
        if matches!(scenario, Scenario::MarketDay)
            && let Some(final_frame) = view.frames.last_mut()
        {
            final_frame
                .after
                .leaderboards
                .push(causal_contribution_board(&initial));
        }
        view.telemetry.push(telemetry(
            "verified market replay",
            true,
            [
                (
                    TelemetryKindView::Transitions,
                    trace.exchanges().len() as u64,
                    "authoritative exchanges".into(),
                ),
                (
                    TelemetryKindView::Accounts,
                    initial.accounts().count() as u64,
                    "closed-economy accounts".into(),
                ),
                (
                    TelemetryKindView::Alternatives,
                    ACTORS.len() as u64,
                    "heterogeneous actors".into(),
                ),
            ],
        ));

        let quote = amm::quote_output(&initial, Asset::Credit, 10_000).unwrap_or_default();
        let guarded = amm::buy_energy(Actor::Factory, 10_000, quote.saturating_add(1));
        view.proposals.push(proposal(ProposalSpec {
                id: path!("impossible-minimum-output"),
                label: "Demand more than the curve returns",
                description: "The trader supplies enough credit, but its minimum-output protection correctly refuses the AMM quote.",
            },
            &initial,
            &guarded,
        ));
        let wrong_ratio = amm::add_liquidity(Actor::AdaptiveLp, 1_000, 9_000, 1);
        view.proposals.push(proposal(ProposalSpec {
                id: path!("price-moving-liquidity"),
                label: "Deposit liquidity at the wrong ratio",
                description: "Liquidity may deepen the discovered market price, but this rule refuses to rewrite that price by depositing an inconsistent reserve ratio.",
            },
            &initial,
            &wrong_ratio,
        ));
        documents.push(view);
    }

    Ok(documents)
}

fn satisfied_needs(world: &World) -> u64 {
    [Actor::Factory, Actor::Household]
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::SatisfiedNeed(actor))
                .to_u64()
        })
        .sum()
}

fn total_utility(world: &World) -> u64 {
    ACTORS
        .into_iter()
        .map(|actor| {
            world
                .balance(&AccountId::Actor(actor), &Asset::Utility)
                .to_u64()
        })
        .sum()
}
