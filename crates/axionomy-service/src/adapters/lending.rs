use super::*;
use axionomy_problems::lending::{self, AccountId, Asset, Config, LoanTrace, WAD, World, balance};
use axionomy_view::{
    GraphEdgeView, GraphNodeView, ObjectiveDirectionView, ObjectiveView, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, TelemetryKindView, ViewId,
};

const PROBLEM: &str = "lending";
const SOURCE: &str = "Lending market";

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let config = match instance_profile(request, descriptor) {
        InstanceProfile::Micro => Config::micro(),
        InstanceProfile::Showcase => Config::showcase(),
        InstanceProfile::Stress => Config::stress(),
    };
    let initial = lending::world(&config);
    let goal = lending::goal(&config);
    let cheapest = lending::least_interest(&config)
        .ok_or_else(|| problem_error(PROBLEM, "no plan repays the loan"))?;
    let fewest = lending::fewest_steps(&config)
        .ok_or_else(|| problem_error(PROBLEM, "no plan repays the loan"))?;
    let mut documents = Vec::new();
    for (strategy, title, description, solution, algorithm) in [
        (
            "least_interest",
            "Bridge loan · least interest",
            "Dijkstra search charges each closed period's interest to the plan. It borrows the shortfall just before the bill is due, keeps the loan safe through the price drop, and repays from each installment as it arrives.",
            &cheapest,
            "Dijkstra search",
        ),
        (
            "fewest_steps",
            "Bridge loan · fewest steps",
            "Breadth-first search counts exchanges, not interest. It borrows in the first period and repays once at the end, so the loan runs longest.",
            &fewest,
            "breadth-first search",
        ),
    ] {
        let mut view = document(
            DocumentSpec {
                problem: PROBLEM,
                strategy,
                title,
                description,
                source_label: SOURCE,
            },
            &initial,
            &goal,
            solution.trace(),
            objectives(&initial, solution.trace())?,
            scene,
        )
        .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
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
        .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
    let mut view = document(
        DocumentSpec {
            problem: PROBLEM,
            strategy: "over_borrowed",
            title: "Bridge loan · over-borrowed",
            description: "The borrower posts all its collateral and borrows to the limit in the first period. Interest at high utilization or the price drop pushes the loan past its liquidation threshold, the clock refuses to close the period, and a keeper repays the loan and seizes collateral plus the bonus.",
            source_label: SOURCE,
        },
        &initial,
        &goal,
        &liquidation,
        objectives(&initial, &liquidation)?,
        scene,
    )
    .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
    view.telemetry.push(scripted(&liquidation));
    documents.push(view);

    let genesis = lending::genesis(&config);
    let (_, attack) = lending::first_depositor_attack(&config)
        .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
    let mut view = document(
        DocumentSpec {
            problem: PROBLEM,
            strategy: "first_depositor_attack",
            title: "Fresh market · first-depositor attack",
            description: "An attacker deposits one unit into a fresh market and donates heavily to inflate the share price before a victim deposits. Shares locked at genesis absorb the donation: the victim loses less than one share's worth to rounding, and the attacker cannot recover what it gave.",
            source_label: SOURCE,
        },
        &genesis,
        &goal,
        &attack,
        Vec::new(),
        scene,
    )
    .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
    view.telemetry.push(scripted(&attack));
    documents.push(view);

    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
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

fn objectives(initial: &World, trace: &LoanTrace) -> Result<Vec<ObjectiveView>, ServiceError> {
    let interest = lending::interest_charged(initial, trace)
        .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
    let after = initial
        .replayed(trace)
        .map_err(|error| problem_error(PROBLEM, format!("{error:?}")))?;
    let collateral = balance(&after, AccountId::Borrower, Asset::Collateral)
        + balance(&after, AccountId::Borrower, Asset::PostedCollateral);
    Ok(vec![
        ObjectiveView {
            key: "interest".into(),
            label: "Interest charged".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: interest.to_string(),
        },
        ObjectiveView {
            key: "collateral".into(),
            label: "Collateral kept".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: collateral.to_string(),
        },
        ObjectiveView {
            key: "cash".into(),
            label: "Borrower cash at the end".into(),
            direction: ObjectiveDirectionView::Maximize,
            value: balance(&after, AccountId::Borrower, Asset::Cash).to_string(),
        },
    ])
}

fn refusals(initial: &World) -> Vec<ProposalView> {
    [
        (
            "unsecured-borrow",
            "Borrow before posting collateral",
            "The loan-to-value rule values posted collateral, and nothing is posted yet.",
            lending::borrow(1_000),
        ),
        (
            "early-revenue",
            "Collect the first installment now",
            "The customer's installment is not due until its period has come.",
            lending::collect_revenue(0),
        ),
        (
            "safe-liquidation",
            "Liquidate a loan that is not past its threshold",
            "A keeper may only liquidate debt above the liquidation threshold of the posted collateral.",
            lending::liquidate(),
        ),
    ]
    .into_iter()
    .map(|(id, label, description, exchange)| {
        proposal(
            PROBLEM,
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

fn account_key(account: AccountId) -> String {
    let key = match account {
        AccountId::Market => "market".to_owned(),
        AccountId::Clock => "clock".to_owned(),
        AccountId::Oracle => "oracle".to_owned(),
        AccountId::Depositor(id) => format!("depositor-{id}"),
        AccountId::Borrower => "borrower".to_owned(),
        AccountId::Keeper => "keeper".to_owned(),
        AccountId::Supplier => "supplier".to_owned(),
        AccountId::Customer => "customer".to_owned(),
    };
    format!("{PROBLEM}:account:{key}")
}

/// Cite the balance when it exists, otherwise the account that would hold it.
fn evidence(
    entity: SceneEntityView,
    world: &World,
    account: AccountId,
    asset: Asset,
    key: &str,
) -> SceneEntityView {
    if balance(world, account, asset) > 0 {
        link_balance(
            entity,
            account_key(account),
            format!("{PROBLEM}:asset:{key}"),
        )
    } else {
        link_account(entity, account_key(account))
    }
}

/// A WAD-scaled share as a percentage with two decimals.
fn percent(value: u128) -> String {
    let hundredths = value * 10_000 / u128::from(WAD);
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

fn node(key: &str, label: &str, classes: &[&str], x: f64, y: f64) -> GraphNodeView {
    GraphNodeView {
        id: ViewId::new(key, label),
        classes: classes.iter().map(|class| (*class).into()).collect(),
        x: Some(x),
        y: Some(y),
    }
}

fn edge(id: &str, source: &str, target: &str, label: &str, active: bool) -> GraphEdgeView {
    GraphEdgeView {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        label: Some(label.into()),
        classes: if active {
            vec!["active".into()]
        } else {
            Vec::new()
        },
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let market = |asset| balance(world, AccountId::Market, asset);
    let borrower = |asset| balance(world, AccountId::Borrower, asset);
    let owed = lending::debt_view(world);
    let liquidation_limit = lending::liquidation_limit_view(world);
    let unsafe_loan = owed > liquidation_limit;
    let open_loan = owed > 0;
    let depositors: Vec<u8> = world
        .accounts()
        .filter_map(|(id, _)| match id {
            AccountId::Depositor(depositor) => Some(*depositor),
            _ => None,
        })
        .collect();
    let bill_paid = borrower(Asset::BillPaid) > 0;
    let pending_installments = world.balance(&AccountId::Customer, &Asset::Cash).get();
    let seized = balance(world, AccountId::Keeper, Asset::Collateral);
    let borrower_classes: &[&str] = if unsafe_loan {
        &["danger"]
    } else if open_loan {
        &["active"]
    } else {
        &[]
    };
    let supplier_classes: &[&str] = if bill_paid { &["success"] } else { &["goal"] };
    let keeper_classes: &[&str] = if seized > 0 { &["warning"] } else { &[] };

    let mut nodes = vec![
        node("oracle", "Price oracle", &["information"], 380.0, 0.0),
        node("market", "Lending market", &["active"], 380.0, 200.0),
        node("borrower", "Borrower", borrower_classes, 720.0, 200.0),
        node("supplier", "Supplier", supplier_classes, 1_060.0, 80.0),
        node("customer", "Customer", &[], 1_060.0, 320.0),
        node("keeper", "Keeper", keeper_classes, 380.0, 400.0),
    ];
    let mut edges = vec![
        edge("loan", "market", "borrower", "loan", open_loan),
        edge("bill", "borrower", "supplier", "bill", bill_paid),
        edge(
            "revenue",
            "customer",
            "borrower",
            "installments",
            pending_installments == 0,
        ),
        edge("price", "oracle", "market", "collateral price", false),
        edge("liquidation", "keeper", "market", "liquidation", seized > 0),
    ];
    for (row, depositor) in depositors.iter().enumerate() {
        let key = format!("depositor-{depositor}");
        nodes.push(node(
            &key,
            &format!("Depositor {}", depositor + 1),
            &["person"],
            0.0,
            row as f64 * 130.0 + 40.0,
        ));
        edges.push(edge(
            &format!("vault-{depositor}"),
            &key,
            "market",
            "vault shares",
            balance(world, AccountId::Depositor(*depositor), Asset::Share) > 0,
        ));
    }

    let mut entities = vec![
        evidence(
            visual_entity(
                "market-cash",
                "Market cash",
                SceneGlyphView::Money,
                SceneAnchorView::GraphNode {
                    node: "market".into(),
                },
                SceneEntityRoleView::State,
                SceneToneView::Active,
                Some(format!("{} cash on hand", market(Asset::Cash))),
            ),
            world,
            AccountId::Market,
            Asset::Cash,
            "cash",
        ),
        evidence(
            visual_entity(
                "posted-collateral",
                "Posted collateral",
                SceneGlyphView::Shield,
                SceneAnchorView::GraphNode {
                    node: "borrower".into(),
                },
                SceneEntityRoleView::State,
                if unsafe_loan {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Neutral
                },
                Some(format!(
                    "{} posted, {} free",
                    borrower(Asset::PostedCollateral),
                    borrower(Asset::Collateral)
                )),
            ),
            world,
            AccountId::Borrower,
            Asset::PostedCollateral,
            "postedcollateral",
        ),
    ];
    if open_loan {
        entities.push(evidence(
            visual_entity(
                "debt",
                "Loan",
                SceneGlyphView::Token,
                SceneAnchorView::GraphNode {
                    node: "borrower".into(),
                },
                SceneEntityRoleView::State,
                if unsafe_loan {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Warning
                },
                Some(format!("owes {owed}, liquidated above {liquidation_limit}")),
            ),
            world,
            AccountId::Borrower,
            Asset::DebtShare,
            "debtshare",
        ));
    }
    if seized > 0 {
        entities.push(evidence(
            visual_entity(
                "seized",
                "Seized collateral",
                SceneGlyphView::Shield,
                SceneAnchorView::GraphNode {
                    node: "keeper".into(),
                },
                SceneEntityRoleView::State,
                SceneToneView::Warning,
                Some(format!("{seized} seized")),
            ),
            world,
            AccountId::Keeper,
            Asset::Collateral,
            "collateral",
        ));
    }

    let elapsed = balance(world, AccountId::Clock, Asset::ElapsedPeriod);
    let remaining = balance(world, AccountId::Clock, Asset::Period);
    Some(
        Scene::graph("Who lends, borrows and is paid", nodes, edges, None)
            .with_entities(entities)
            .with_metrics([
                visual_metric(
                    "period",
                    "Period",
                    format!("{} of {}", elapsed + 1, elapsed + remaining),
                    None,
                ),
                visual_metric(
                    "utilization",
                    "Utilization",
                    percent(lending::utilization_view(world)),
                    Some("%"),
                ),
                visual_metric(
                    "rate",
                    "Interest per period",
                    percent(lending::rate_view(world)),
                    Some("%"),
                ),
                visual_metric("owed", "Borrower owes", owed, Some("cash")),
                visual_metric(
                    "borrow_limit",
                    "Borrowing limit",
                    lending::borrow_limit_view(world),
                    Some("cash"),
                ),
                visual_metric(
                    "liquidation_limit",
                    "Liquidation threshold",
                    liquidation_limit,
                    Some("cash"),
                ),
                visual_metric(
                    "price",
                    "Collateral price",
                    market(Asset::CollateralPrice),
                    Some("cash"),
                ),
                visual_metric(
                    "reserves",
                    "Reserves",
                    market(Asset::Reserves),
                    Some("cash"),
                ),
                visual_metric(
                    "share_assets",
                    "Depositors' assets",
                    lending::total_assets_view(world),
                    Some("cash"),
                ),
            ]),
    )
}
