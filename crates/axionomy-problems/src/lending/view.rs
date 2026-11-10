//! How the lending market is presented: its labels, units, scene and leaderboards.

use crate::lending::{self, AccountId, Asset, WAD, World, balance};
use axionomy::path;
use axionomy_view::{
    ClassPath, Format, GraphEdgeView, GraphNodeView, ItemPath, Keyed, Label, QuantityFormat, Scene,
    SceneAnchorView, SceneEntityRoleView, SceneEntityView, SceneGlyphView, SceneToneView, ViewId,
    asset_metric, debug_labels, link_account, link_balance, visual_entity, visual_metric,
};

/// Cite the balance when it exists, otherwise the account that would hold it.
fn evidence(
    entity: SceneEntityView,
    world: &World,
    account: AccountId,
    asset: Asset,
) -> SceneEntityView {
    if balance(world, account, asset) > 0 {
        link_balance::<lending::Ontology>(entity, &account, &asset)
    } else {
        link_account::<lending::Ontology>(entity, &account)
    }
}

/// A WAD-scaled share as a percentage with two decimals.
fn percent(value: u128) -> String {
    let hundredths = value * 10_000 / u128::from(WAD);
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

fn node(key: ItemPath, label: &str, classes: Vec<ClassPath>, x: f64, y: f64) -> GraphNodeView {
    GraphNodeView {
        id: ViewId::new(key, label),
        classes,
        x: Some(x),
        y: Some(y),
    }
}

fn edge(
    id: ItemPath,
    source: ItemPath,
    target: ItemPath,
    label: &str,
    active: bool,
) -> GraphEdgeView {
    GraphEdgeView {
        id,
        source,
        target,
        label: Some(label.into()),
        classes: if active {
            vec![path!("active")]
        } else {
            Vec::new()
        },
    }
}

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
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
    let pending_installments = world.balance(&AccountId::Customer, &Asset::Cash).to_u64();
    let seized = balance(world, AccountId::Keeper, Asset::Collateral);
    let borrower_classes = if unsafe_loan {
        vec![path!("danger")]
    } else if open_loan {
        vec![path!("active")]
    } else {
        Vec::new()
    };
    let supplier_classes = vec![if bill_paid {
        path!("success")
    } else {
        path!("goal")
    }];
    let keeper_classes = if seized > 0 {
        vec![path!("warning")]
    } else {
        Vec::new()
    };

    let mut nodes = vec![
        node(
            path!("oracle"),
            "Price oracle",
            vec![path!("information")],
            380.0,
            0.0,
        ),
        node(
            path!("market"),
            "Lending market",
            vec![path!("active")],
            380.0,
            200.0,
        ),
        node(
            path!("borrower"),
            "Borrower",
            borrower_classes,
            720.0,
            200.0,
        ),
        node(
            path!("supplier"),
            "Supplier",
            supplier_classes,
            1_060.0,
            80.0,
        ),
        node(path!("customer"), "Customer", Vec::new(), 1_060.0, 320.0),
        node(path!("keeper"), "Keeper", keeper_classes, 380.0, 400.0),
    ];
    let mut edges = vec![
        edge(
            path!("loan"),
            path!("market"),
            path!("borrower"),
            "loan",
            open_loan,
        ),
        edge(
            path!("bill"),
            path!("borrower"),
            path!("supplier"),
            "bill",
            bill_paid,
        ),
        edge(
            path!("revenue"),
            path!("customer"),
            path!("borrower"),
            "installments",
            pending_installments == 0,
        ),
        edge(
            path!("price"),
            path!("oracle"),
            path!("market"),
            "collateral price",
            false,
        ),
        edge(
            path!("liquidation"),
            path!("keeper"),
            path!("market"),
            "liquidation",
            seized > 0,
        ),
    ];
    for (row, depositor) in depositors.iter().enumerate() {
        let key = path!("depositor").then(depositor);
        nodes.push(node(
            key.clone(),
            &format!("Depositor {}", depositor + 1),
            vec![path!("person")],
            0.0,
            row as f64 * 130.0 + 40.0,
        ));
        edges.push(edge(
            path!("vault").then(depositor),
            key,
            path!("market"),
            "vault shares",
            balance(world, AccountId::Depositor(*depositor), Asset::Share) > 0,
        ));
    }

    let mut entities = vec![
        evidence(
            visual_entity(
                path!("market-cash"),
                "Market cash",
                SceneGlyphView::Money,
                SceneAnchorView::GraphNode {
                    node: path!("market"),
                },
                SceneEntityRoleView::State,
                SceneToneView::Active,
                Some(format!("{} cash on hand", market(Asset::Cash))),
            ),
            world,
            AccountId::Market,
            Asset::Cash,
        ),
        evidence(
            visual_entity(
                path!("posted-collateral"),
                "Posted collateral",
                SceneGlyphView::Shield,
                SceneAnchorView::GraphNode {
                    node: path!("borrower"),
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
        ),
    ];
    if open_loan {
        entities.push(evidence(
            visual_entity(
                path!("debt"),
                "Loan",
                SceneGlyphView::Token,
                SceneAnchorView::GraphNode {
                    node: path!("borrower"),
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
        ));
    }
    if seized > 0 {
        entities.push(evidence(
            visual_entity(
                path!("seized"),
                "Seized collateral",
                SceneGlyphView::Shield,
                SceneAnchorView::GraphNode {
                    node: path!("keeper"),
                },
                SceneEntityRoleView::State,
                SceneToneView::Warning,
                Some(format!("{seized} seized")),
            ),
            world,
            AccountId::Keeper,
            Asset::Collateral,
        ));
    }

    let elapsed = balance(world, AccountId::Clock, Asset::ElapsedPeriod);
    let remaining = balance(world, AccountId::Clock, Asset::Period);
    Some(
        Scene::graph("Who lends, borrows and is paid", nodes, edges, None)
            .with_entities(entities)
            .with_metrics([
                visual_metric(
                    path!("period"),
                    "Period",
                    format!("{} of {}", elapsed + 1, elapsed + remaining),
                    None,
                ),
                visual_metric(
                    path!("utilization"),
                    "Utilization",
                    percent(lending::utilization_view(world)),
                    Some("%"),
                ),
                visual_metric(
                    path!("rate"),
                    "Interest per period",
                    percent(lending::rate_view(world)),
                    Some("%"),
                ),
                asset_metric(path!("owed"), "Borrower owes", &Asset::Cash, owed),
                asset_metric(
                    path!("borrow-limit"),
                    "Borrowing limit",
                    &Asset::Cash,
                    lending::borrow_limit_view(world),
                ),
                asset_metric(
                    path!("liquidation-limit"),
                    "Liquidation threshold",
                    &Asset::Cash,
                    liquidation_limit,
                ),
                asset_metric(
                    path!("price"),
                    "Collateral price",
                    &Asset::CollateralPrice,
                    market(Asset::CollateralPrice),
                ),
                asset_metric(
                    path!("reserves"),
                    "Reserves",
                    &Asset::Reserves,
                    market(Asset::Reserves),
                ),
                asset_metric(
                    path!("share-assets"),
                    "Depositors' assets",
                    &Asset::Cash,
                    lending::total_assets_view(world),
                ),
            ]),
    )
}

// How Studio names and writes this problem's values.

/// The market's terms are basis points scaled by `WAD` (10^18), so 10^16 atoms
/// are one percent; the borrow index is a `WAD`-scaled multiplier. Values the
/// market keeps in cash read as cash, and the clock counts periods. Cash
/// itself stays a plain count: its label already names the unit.
impl Format for lending::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::BorrowIndex => QuantityFormat::decimal(18),
            Self::BaseRate
            | Self::Kink
            | Self::SlopeBelowKink
            | Self::SlopeAboveKink
            | Self::ReserveFactor
            | Self::LoanToValue
            | Self::LiquidationThreshold
            | Self::LiquidationBonus => QuantityFormat::decimal(16).unit("%"),
            Self::Cash | Self::Reserves | Self::CollateralPrice | Self::PriceDrop => {
                QuantityFormat::COUNT.unit("cash")
            }
            Self::Period | Self::ElapsedPeriod | Self::ShockAfter => {
                QuantityFormat::COUNT.unit("periods")
            }
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for lending::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Market => "Lending market".into(),
            Self::Clock => "Clock".into(),
            Self::Oracle => "Price oracle".into(),
            Self::Depositor(id) => format!("Depositor {}", id + 1),
            Self::Borrower => "Borrower".into(),
            Self::Keeper => "Keeper".into(),
            Self::Supplier => "Supplier".into(),
            Self::Customer => "Customer".into(),
        }
    }
}

impl Label for lending::Asset {
    fn label(&self) -> String {
        match self {
            Self::Cash => "Cash".into(),
            Self::Share => "Vault share".into(),
            Self::LockedShare => "Locked genesis share".into(),
            Self::ShareSupply => "Vault share supply".into(),
            Self::DebtShare => "Debt share".into(),
            Self::DebtShareSupply => "Debt share supply".into(),
            Self::BorrowIndex => "Borrow index".into(),
            Self::Reserves => "Reserves".into(),
            Self::Collateral => "Collateral".into(),
            Self::PostedCollateral => "Posted collateral".into(),
            Self::CollateralPrice => "Collateral price".into(),
            Self::BaseRate => "Base rate".into(),
            Self::Kink => "Utilization kink".into(),
            Self::SlopeBelowKink => "Rate slope below the kink".into(),
            Self::SlopeAboveKink => "Rate slope above the kink".into(),
            Self::ReserveFactor => "Reserve factor".into(),
            Self::LoanToValue => "Loan-to-value limit".into(),
            Self::LiquidationThreshold => "Liquidation threshold".into(),
            Self::LiquidationBonus => "Liquidation bonus".into(),
            Self::CreditLine => "Credit line".into(),
            Self::Invoice => "Supplier invoice".into(),
            Self::BillPaid => "Bill paid".into(),
            Self::PurchaseOrder(id) => format!("Purchase order {}", id + 1),
            Self::MarketIdentity => "Market identity".into(),
            Self::ClockIdentity => "Clock identity".into(),
            Self::OracleIdentity => "Oracle identity".into(),
            Self::Period => "Time remaining".into(),
            Self::ElapsedPeriod => "Time elapsed".into(),
            Self::PendingShock => "Pending price drop".into(),
            Self::ShockAfter => "Price drop after".into(),
            Self::PriceDrop => "Price drop".into(),
        }
    }
}

impl Label for lending::RateId {
    fn label(&self) -> String {
        match self {
            Self::Deposit => "Deposit cash for vault shares".into(),
            Self::Redeem => "Redeem vault shares for cash".into(),
            Self::Donate => "Donate cash to the market".into(),
            Self::PostCollateral => "Post collateral".into(),
            Self::WithdrawCollateral => "Withdraw collateral".into(),
            Self::Borrow => "Borrow cash".into(),
            Self::Repay => "Repay cash".into(),
            Self::RepayShares => "Repay the whole loan".into(),
            Self::PayBill => "Pay the supplier's bill".into(),
            Self::CollectRevenue(id) => format!("Collect installment {}", id + 1),
            Self::Accrue => "Close the period and accrue interest".into(),
            Self::Reprice => "Apply the scheduled price drop".into(),
            Self::Liquidate => "Liquidate the loan".into(),
        }
    }
}

debug_labels!(lending::Role);
