//! How the workshop is presented: its labels, units, scene and leaderboards.

use crate::workshop::{self, AccountId, Asset, World};
use axionomy::path;
use axionomy_view::{
    Format, GraphEdgeView, GraphNodeView, Keyed, Label, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, ViewId, asset_metric, debug_labels,
    link_balance, visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let mut assets = vec![
        ("Wood", Asset::Wood),
        ("Labor", Asset::Labor),
        ("Tool", Asset::Tool),
        ("Chair", Asset::Chair),
        ("Waste", Asset::Waste),
    ];
    if world.account(&AccountId::Customer(0)).is_some() {
        assets.extend([
            ("Frames", Asset::Frame),
            ("Panels", Asset::Panel),
            ("Tables", Asset::Table),
            ("Offcuts", Asset::Offcut),
            ("Durability", Asset::Durability),
            ("Wear", Asset::Wear),
            ("Revenue", Asset::Credit),
        ]);
    }
    let nodes = assets
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (label, asset))| GraphNodeView {
            id: ViewId::new(path::write(&asset), label),
            classes: if matches!(asset, Asset::Chair) {
                vec![path!("goal")]
            } else {
                vec![path!("resource")]
            },
            x: Some((index as f64 % 3.0) * 220.0 + 80.0),
            y: Some((index / 3) as f64 * 150.0 + 70.0),
        })
        .collect();
    let routes = if world.account(&AccountId::Customer(0)).is_some() {
        vec![
            (Asset::Wood, Asset::Frame, "cut"),
            (Asset::Wood, Asset::Panel, "cut"),
            (Asset::Wood, Asset::Offcut, "fast-cut residue"),
            (Asset::Frame, Asset::Chair, "assemble"),
            (Asset::Panel, Asset::Chair, "assemble"),
            (Asset::Frame, Asset::Table, "assemble"),
            (Asset::Panel, Asset::Table, "assemble"),
            (Asset::Offcut, Asset::Wood, "recycle with loss"),
            (Asset::Offcut, Asset::Waste, "recycling loss"),
            (Asset::Durability, Asset::Wear, "production"),
            (Asset::Wear, Asset::Durability, "repair"),
            (Asset::Chair, Asset::Credit, "deliver"),
            (Asset::Table, Asset::Credit, "deliver"),
        ]
    } else {
        vec![
            (Asset::Wood, Asset::Chair, "basic recipe"),
            (Asset::Wood, Asset::Chair, "batch recipe"),
            (Asset::Wood, Asset::Waste, "by-product"),
            (Asset::Tool, Asset::Chair, "preserved catalyst"),
        ]
    };
    let edges = routes
        .into_iter()
        .enumerate()
        .map(|(index, (from, to, label))| GraphEdgeView {
            id: path!("recipe").then(&index),
            source: path::write(&from),
            target: path::write(&to),
            label: Some(label.into()),
            classes: vec![],
        })
        .collect();
    let stocks = assets.into_iter().filter_map(|(label, asset)| {
        let quantity = world.balance(&AccountId::Workshop, &asset);
        if quantity.is_zero() {
            return None;
        }
        let mut entity = link_balance::<workshop::Ontology>(
            visual_entity(
                path!("stock").then(&asset),
                label,
                match asset {
                    Asset::Tool => SceneGlyphView::Tool,
                    Asset::Chair => SceneGlyphView::Product,
                    Asset::Waste => SceneGlyphView::Material,
                    _ => SceneGlyphView::Material,
                },
                SceneAnchorView::GraphNode {
                    node: path::write(&asset),
                },
                SceneEntityRoleView::State,
                if matches!(asset, Asset::Chair) {
                    SceneToneView::Success
                } else if matches!(asset, Asset::Waste) {
                    SceneToneView::Warning
                } else {
                    SceneToneView::Active
                },
                Some(format!("{} available", asset.format().write(quantity))),
            ),
            &AccountId::Workshop,
            &asset,
        );
        entity.metrics = vec![asset_metric(path::write(&asset), label, &asset, quantity)];
        Some(entity)
    });
    let contracts = world.accounts().filter_map(|(id, _)| {
        let AccountId::Customer(order) = id else {
            return None;
        };
        let state = if !world
            .balance(&AccountId::Workshop, &Asset::Fulfilled(*order))
            .is_zero()
        {
            "delivered and paid"
        } else if !world
            .balance(&AccountId::Workshop, &Asset::Assembled(*order))
            .is_zero()
        {
            "ready for delivery"
        } else if !world
            .balance(&AccountId::Workshop, &Asset::Prepared(*order))
            .is_zero()
        {
            "components ready"
        } else if !world
            .balance(&AccountId::Workshop, &Asset::Raw(*order))
            .is_zero()
        {
            "accepted · awaiting components"
        } else {
            "available"
        };
        let mut entity = link_balance::<workshop::Ontology>(
            visual_entity(
                path!("contract").then(order),
                format!("Contract {}", order + 1),
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: path::write(&Asset::Credit),
                },
                SceneEntityRoleView::Attachment,
                if state == "delivered and paid" {
                    SceneToneView::Success
                } else {
                    SceneToneView::Neutral
                },
                Some(state.into()),
            ),
            id,
            &Asset::CustomerIdentity(*order),
        );
        entity.metrics = vec![asset_metric(
            path!("escrow"),
            "Customer funds",
            &Asset::Credit,
            world.balance(id, &Asset::Credit),
        )];
        Some(entity)
    });
    let metrics = if world.account(&AccountId::Customer(0)).is_some() {
        vec![
            asset_metric(
                path!("completed"),
                "Contracts delivered",
                &Asset::Completed,
                world.balance(&AccountId::Workshop, &Asset::Completed),
            ),
            asset_metric(
                path!("revenue"),
                "Revenue",
                &Asset::Credit,
                world.balance(&AccountId::Workshop, &Asset::Credit),
            ),
            asset_metric(
                path!("durability"),
                "Tool durability",
                &Asset::Durability,
                world.balance(&AccountId::Workshop, &Asset::Durability),
            ),
            asset_metric(
                path!("repairs"),
                "Repair kits remaining",
                &Asset::RepairKit,
                world.balance(&AccountId::Workshop, &Asset::RepairKit),
            ),
            visual_metric(
                path!("upgrade"),
                "Precision upgrade",
                if world
                    .balance(&AccountId::Workshop, &Asset::Upgraded)
                    .is_zero()
                {
                    "available"
                } else {
                    "installed"
                },
                None,
            ),
        ]
    } else {
        vec![
            asset_metric(
                path!("waste"),
                "Waste",
                &Asset::Waste,
                workshop::waste(world),
            ),
            asset_metric(
                path!("time"),
                "Process time",
                &Asset::SpentTime,
                workshop::spent_time(world),
            ),
        ]
    };
    Some(
        Scene::graph(
            "Materials, tooling and contract production",
            nodes,
            edges,
            None,
        )
        .with_entities(stocks.chain(contracts))
        .with_metrics(metrics),
    )
}

// How Studio names and writes this problem's values.

impl Format for workshop::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Wood | Self::Panel | Self::Waste | Self::Labor | Self::SpentLabor => {
                QuantityFormat::COUNT.unit("units")
            }
            Self::Time | Self::SpentTime => QuantityFormat::COUNT.unit("ticks"),
            Self::Credit => QuantityFormat::COUNT.unit("credits"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for workshop::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Workshop => "Workshop".into(),
            Self::Customer(id) => format!("Customer {}", id + 1),
        }
    }
}

impl Label for workshop::Asset {
    fn label(&self) -> String {
        match self {
            Self::CustomerIdentity(id) => format!("Customer {} identity", id + 1),
            Self::Credit => "Credit".into(),
            Self::Frame => "Prepared frames".into(),
            Self::Panel => "Prepared panel material".into(),
            Self::Offcut => "Recyclable offcuts".into(),
            Self::Table => "Finished tables".into(),
            Self::Durability => "Tool durability".into(),
            Self::Wear => "Tool wear".into(),
            Self::ToolPotential => "Upgrade capacity".into(),
            Self::RepairKit => "Repair kits".into(),
            Self::UsedRepairKit => "Used repair kits".into(),
            Self::UpgradeKit => "Available upgrade".into(),
            Self::Upgraded => "Precision tool upgrade".into(),
            Self::Idle => "Workbench available".into(),
            Self::Contract(id) => format!("Contract {} available", id + 1),
            Self::Raw(id) => format!("Contract {} awaiting components", id + 1),
            Self::Prepared(id) => format!("Contract {} components prepared", id + 1),
            Self::Assembled(id) => format!("Contract {} ready to deliver", id + 1),
            Self::Fulfilled(id) => format!("Contract {} fulfilled", id + 1),
            Self::Completed => "Contracts delivered".into(),
            Self::Wood => "Wood left".into(),
            Self::Labor => "Labor left".into(),
            Self::SpentLabor => "Labor used".into(),
            Self::Tool => "Reusable tool".into(),
            Self::Chair => "Finished chairs".into(),
            Self::Waste => "Scrap".into(),
            Self::Time => "Time left".into(),
            Self::SpentTime => "Time spent".into(),
            Self::Active => "Order in progress".into(),
            Self::Solved => "Order complete".into(),
        }
    }
}

impl Label for workshop::RateId {
    fn label(&self) -> String {
        match self {
            Self::Accept(id) => format!("Accept contract {}", id + 1),
            Self::Prepare {
                order,
                precise,
                upgraded,
            } => format!(
                "Prepare components for contract {} with {}",
                order + 1,
                if *upgraded {
                    "the upgraded tool"
                } else if *precise {
                    "a precision cut"
                } else {
                    "a fast cut"
                }
            ),
            Self::Assemble(id) => format!("Assemble contract {}", id + 1),
            Self::Deliver(id) => format!("Deliver contract {} and collect payment", id + 1),
            Self::Repair => "Restore worn tool with a repair kit".into(),
            Self::Upgrade => "Install the precision tool upgrade".into(),
            Self::Recycle => "Recover wood from two offcuts".into(),
            Self::BasicChair => "Build one chair, fast but with more scrap".into(),
            Self::EfficientBatch => "Build two chairs, slower but with less scrap".into(),
            Self::CounterfeitChair => "Counterfeit chair proposal".into(),
            Self::Finish => "Complete the order".into(),
        }
    }
}

debug_labels!(workshop::Role);
