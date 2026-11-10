//! The lending market as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    lending::{self, AccountId, Asset, Config, LoanTrace, World, balance},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::path;
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProblemPath, ProposalSpec, ServiceError, StudyKey, StudyProgress, document, problem_error,
    proposal, telemetry,
};
use axionomy_view::{
    ObjectiveDirectionView, ObjectiveView, ProposalView, SearchTelemetryView, TelemetryKindView,
    ViewDocument,
};

/// Lending market: its catalog entry and how its studies are computed.
pub struct Lending {
    descriptor: ProblemDescriptor,
}

impl Default for Lending {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("lending"),
                    title: "Lending market",
                    summary: "Finance a bill that falls due before revenue arrives. Collateral caps the loan, interest follows pool utilization along a kinked curve, a reserve cut stays with the market, and a scheduled price drop can push an over-borrowed loan into liquidation.",
                    instances: [
                        "One pooled market and a two-period gap between the bill and revenue, with a price drop while the loan is open",
                        "A thin pool near its kink, a bill due early, revenue in two installments and a 40% price drop while the loan is open",
                        "The Showcase loan at four times the scale over twelve periods, with revenue in three installments",
                    ],
                },
                ProblemFamily::Market,
                path!("least-interest"),
                &[
                    StrategyCopy {
                        key: path!("least-interest"),
                        label: "Least interest",
                        description: "Dijkstra search charging each closed period's interest to the plan.",
                        algorithm: "Dijkstra search",
                    },
                    StrategyCopy {
                        key: path!("fewest-steps"),
                        label: "Fewest steps",
                        description: "Breadth-first search: the fewest exchanges, whatever the loan costs.",
                        algorithm: "breadth-first search",
                    },
                    StrategyCopy {
                        key: path!("over-borrowed"),
                        label: "Over-borrowed",
                        description: "Borrow to the limit at once and watch interest and the price drop end in liquidation.",
                        algorithm: "scripted scenario",
                    },
                    StrategyCopy {
                        key: path!("first-depositor-attack"),
                        label: "First-depositor attack",
                        description: "Try to inflate a fresh market's share price against a later depositor.",
                        algorithm: "scripted scenario",
                    },
                ],
                &[
                    Capability::DeterministicSearch,
                    Capability::WeightedSearch,
                    Capability::AtomicSettlement,
                    Capability::MultiAccountExchange,
                    Capability::FeasibilityAssessment,
                    Capability::TemporalEffects,
                ],
            ),
        }
    }
}

impl Problem for Lending {
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

const SOURCE: &str = "Lending market";

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let config = match study.instance {
        InstanceProfile::Micro => Config::micro(),
        InstanceProfile::Showcase => Config::showcase(),
        InstanceProfile::Stress => Config::stress(),
    };
    let problem = &study.problem;
    let initial = lending::world(&config);
    let goal = lending::goal(&config);
    let cheapest = lending::least_interest(&config)
        .ok_or_else(|| problem_error(problem, "no plan repays the loan"))?;
    let fewest = lending::fewest_steps(&config)
        .ok_or_else(|| problem_error(problem, "no plan repays the loan"))?;
    let mut documents = Vec::new();
    for (strategy, title, description, solution, algorithm) in [
        (
            path!("least-interest"),
            "Lending · least interest",
            "Dijkstra search charges each closed period's interest to the plan. It borrows the shortfall just before the bill is due, keeps the loan safe through the price drop, and repays from each installment as it arrives.",
            &cheapest,
            "Dijkstra search",
        ),
        (
            path!("fewest-steps"),
            "Lending · fewest steps",
            "Breadth-first search counts exchanges, not interest. It borrows in the first period and repays once at the end, so the loan runs longest.",
            &fewest,
            "breadth-first search",
        ),
    ] {
        let mut view = document(
            progress,
            DocumentSpec {
                problem,
                document: strategy,
                title,
                description,
                source_label: SOURCE,
            },
            &initial,
            &goal,
            solution.trace(),
            objectives(&initial, solution.trace(), problem)?,
            scene,
        )
        .map_err(|error| problem_error(problem, format!("{error:?}")))?;
        view.telemetry.push(telemetry(
            algorithm,
            true,
            [
                (
                    TelemetryKindView::Expanded,
                    solution.expanded() as u64,
                    "states explored".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    solution.trace().exchanges().len() as u64,
                    "exchanges in the plan".into(),
                ),
            ],
        ));
        view.proposals.extend(refusals(&initial));
        documents.push(view);
    }

    let (_, liquidation) = lending::over_borrowed(&config)
        .map_err(|error| problem_error(problem, format!("{error:?}")))?;
    let mut view = document(
        progress,
        DocumentSpec {
            problem,
            document: path!("over-borrowed"),
            title: "Lending · over-borrowed",
            description: "The borrower posts all its collateral and borrows to the limit in the first period. Interest at high utilization or the price drop pushes the loan past its liquidation threshold, the clock refuses to close the period, and a keeper repays the loan and seizes collateral plus the bonus.",
            source_label: SOURCE,
        },
        &initial,
        &goal,
        &liquidation,
        objectives(&initial, &liquidation, problem)?,
        scene,
    )
    .map_err(|error| problem_error(problem, format!("{error:?}")))?;
    view.telemetry.push(scripted(&liquidation));
    documents.push(view);

    let genesis = lending::genesis(&config);
    let (_, attack) = lending::first_depositor_attack(&config)
        .map_err(|error| problem_error(problem, format!("{error:?}")))?;
    let mut view = document(
        progress,
        DocumentSpec {
            problem,
            document: path!("first-depositor-attack"),
            title: "Lending · first-depositor attack on a fresh market",
            description: "An attacker deposits one unit into a fresh market and donates heavily to inflate the share price before a victim deposits. Shares locked at genesis absorb the donation: the victim loses less than one share's worth to rounding, and the attacker cannot recover what it gave.",
            source_label: SOURCE,
        },
        &genesis,
        &goal,
        &attack,
        Vec::new(),
        scene,
    )
    .map_err(|error| problem_error(problem, format!("{error:?}")))?;
    view.telemetry.push(scripted(&attack));
    documents.push(view);

    Ok(documents)
}

fn scripted(trace: &LoanTrace) -> SearchTelemetryView {
    telemetry(
        "scripted scenario",
        true,
        [(
            TelemetryKindView::Generated,
            trace.exchanges().len() as u64,
            "exchanges in the scenario".into(),
        )],
    )
}

fn objectives(
    initial: &World,
    trace: &LoanTrace,
    problem: &ProblemPath,
) -> Result<Vec<ObjectiveView>, ServiceError> {
    let interest = lending::interest_charged(initial, trace)
        .map_err(|error| problem_error(problem, format!("{error:?}")))?;
    let after = initial
        .replayed(trace)
        .map_err(|error| problem_error(problem, format!("{error:?}")))?;
    let collateral = balance(&after, AccountId::Borrower, Asset::Collateral)
        + balance(&after, AccountId::Borrower, Asset::PostedCollateral);
    Ok(vec![
        ObjectiveView::new(
            path!("interest"),
            "Interest charged",
            ObjectiveDirectionView::Minimize,
            interest,
        ),
        ObjectiveView::new(
            path!("collateral"),
            "Collateral kept",
            ObjectiveDirectionView::Maximize,
            collateral,
        ),
        ObjectiveView::new(
            path!("cash"),
            "Borrower cash at the end",
            ObjectiveDirectionView::Maximize,
            balance(&after, AccountId::Borrower, Asset::Cash),
        ),
    ])
}

fn refusals(initial: &World) -> Vec<ProposalView> {
    [
        (
            path!("unsecured-borrow"),
            "Borrow before posting collateral",
            "The loan-to-value rule values posted collateral, and nothing is posted yet.",
            lending::borrow(1_000),
        ),
        (
            path!("early-revenue"),
            "Collect the first installment now",
            "The customer's installment is not due until its period has come.",
            lending::collect_revenue(0),
        ),
        (
            path!("safe-liquidation"),
            "Liquidate a loan that is not past its threshold",
            "A keeper may only liquidate debt above the liquidation threshold of the posted collateral.",
            lending::liquidate(),
        ),
    ]
    .into_iter()
    .map(|(id, label, description, exchange)| {
        proposal(
            ProposalSpec {
                id,
                label,
                description,
            },
            initial,
            &exchange,
        )
    })
    .collect()
}
