use super::*;
use axionomy_problems::workshop::{self, AccountId, Asset, ObjectiveKey, RateId, World};
use axionomy_search::pareto::Objective;
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, SceneAnchorView, SceneGlyphView,
    SceneToneView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let initial = match study.instance {
        InstanceProfile::Micro => workshop::initial(),
        InstanceProfile::Showcase => workshop::initial_showcase(),
        InstanceProfile::Stress => workshop::initial_stress(),
    };
    let bfs = workshop::solve_bfs(&initial)
        .ok_or_else(|| problem_error("workshop", "BFS found no plan"))?;
    let best = workshop::minimize_waste(&initial)
        .ok_or_else(|| problem_error("workshop", "best-first found no plan"))?;
    let pareto =
        workshop::pareto_front(&initial).map_err(|error| problem_error("workshop", error))?;
    let waste_trace = frontier_trace(&pareto, true)?;
    let time_trace = frontier_trace(&pareto, false)?;
    let mut traces = vec![
        (
            "breadth-first",
            "Workshop · fewest steps",
            "Breadth-first search minimises the number of steps, ignoring how much material is wasted.",
            bfs.trace().clone(),
            "breadth-first search",
            bfs.expanded() as u64,
        ),
        (
            "minimum-waste",
            "Workshop · least waste",
            "Best-first search minimizes irreversible scrap; unused offcuts become scrap when the shift closes.",
            best.trace().clone(),
            "best-first search",
            best.expanded() as u64,
        ),
        (
            "pareto-waste",
            "Workshop Pareto · least waste",
            "A least-waste plan on the frontier of scrap, time and earned revenue.",
            waste_trace,
            "exact Pareto search",
            pareto.progress().expanded() as u64,
        ),
        (
            "pareto-time",
            "Workshop Pareto · least time",
            "A fastest plan on the frontier, including setup, component production, repair and delivery.",
            time_trace,
            "exact Pareto search",
            pareto.progress().expanded() as u64,
        ),
    ];
    if initial.account(&AccountId::Customer(0)).is_some() {
        let entry = pareto
            .front()
            .entries()
            .iter()
            .max_by_key(|entry| {
                objective_value(entry.objectives().objectives(), ObjectiveKey::Revenue)
            })
            .unwrap();
        traces.push(("pareto-revenue", "Workshop Pareto · highest revenue",
            "The highest contract revenue on the exact frontier; tables require more material, labor and tooling than chairs.",
            entry.payload().clone(), "exact Pareto search", pareto.progress().expanded() as u64));
    }
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("workshop", error))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: "workshop",
                strategy,
                title,
                description,
                source_label: "Stoichiometric workshop",
            },
            &initial,
            &workshop::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error("workshop", error))?;
        view.pareto_fronts.push(front_view(&pareto, &view));
        view.telemetry.push(telemetry(
            algorithm,
            true,
            [
                (
                    TelemetryKindView::Expanded,
                    expanded,
                    "states explored".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "production exchanges".into(),
                ),
            ],
        ));
        let counterfeit = workshop::action(RateId::CounterfeitChair);
        view.proposals.push(proposal(ProposalSpec { id: "counterfeit", label: "Counterfeit chair", description: "This rule would create material out of nothing, and the conservation law rejects it." }, &initial, &counterfeit));
        documents.push(view);
    }
    assemble(study, descriptor, documents)
}

fn frontier_trace(
    result: &workshop::ParetoResult,
    waste_first: bool,
) -> Result<axionomy::Trace<workshop::Ontology>, ServiceError> {
    result
        .front()
        .entries()
        .iter()
        .min_by_key(|entry| {
            let waste = objective_value(entry.objectives().objectives(), ObjectiveKey::Waste);
            let time = objective_value(entry.objectives().objectives(), ObjectiveKey::Time);
            if waste_first {
                (waste, time)
            } else {
                (time, waste)
            }
        })
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error("workshop", "empty Pareto frontier"))
}

fn objective_value(values: &[Objective<ObjectiveKey, u64>], key: ObjectiveKey) -> u64 {
    values
        .iter()
        .find(|objective| objective.key() == &key)
        .map_or(0, |objective| *objective.value())
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    workshop::objectives(world)
        .objectives()
        .iter()
        .map(|objective| {
            let (key, label, direction) = match objective.key() {
                ObjectiveKey::Waste => ("waste", "Waste", ObjectiveDirectionView::Minimize),
                ObjectiveKey::Time => ("time", "Process time", ObjectiveDirectionView::Minimize),
                ObjectiveKey::Revenue => (
                    "revenue",
                    "Contract revenue",
                    ObjectiveDirectionView::Maximize,
                ),
            };
            ObjectiveView {
                key: slug(key),
                label: label.into(),
                direction,
                value: objective.value().to_string(),
            }
        })
        .collect()
}

fn front_view(result: &workshop::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let axes = selected
        .objectives
        .iter()
        .map(|objective| ObjectiveAxisView {
            key: objective.key.clone(),
            label: objective.label.clone(),
            direction: objective.direction,
        })
        .collect();
    let selected = selected
        .objectives
        .iter()
        .map(|objective| objective.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Production tradeoffs — every non-dominated plan".into(),
        completeness: FrontierCompletenessView::Exact,
        axes,
        points: result
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|objective| objective.value().to_string())
                    .collect::<Vec<_>>();
                ParetoPointView {
                    label: if values.len() == 3 {
                        format!(
                            "{} waste · {} time · {} credit",
                            values[0], values[1], values[2]
                        )
                    } else {
                        format!("{} waste · {} time", values[0], values[1])
                    },
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
    let mut assets = vec![
        ("wood", "Wood", Asset::Wood),
        ("labor", "Labor", Asset::Labor),
        ("tool", "Tool", Asset::Tool),
        ("chair", "Chair", Asset::Chair),
        ("waste", "Waste", Asset::Waste),
    ];
    if world.account(&AccountId::Customer(0)).is_some() {
        assets.extend([
            ("frame", "Frames", Asset::Frame),
            ("panel", "Panels", Asset::Panel),
            ("table", "Tables", Asset::Table),
            ("offcut", "Offcuts", Asset::Offcut),
            ("durability", "Durability", Asset::Durability),
            ("wear", "Wear", Asset::Wear),
            ("credit", "Revenue", Asset::Credit),
        ]);
    }
    let nodes = assets
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (key, label, asset))| GraphNodeView {
            id: ViewId::new(slug(key), label),
            classes: if matches!(asset, Asset::Chair) {
                vec![slug("goal")]
            } else {
                vec![slug("resource")]
            },
            x: Some((index as f64 % 3.0) * 220.0 + 80.0),
            y: Some((index / 3) as f64 * 150.0 + 70.0),
        })
        .collect();
    let routes = if world.account(&AccountId::Customer(0)).is_some() {
        vec![
            ("wood", "frame", "cut"),
            ("wood", "panel", "cut"),
            ("wood", "offcut", "fast-cut residue"),
            ("frame", "chair", "assemble"),
            ("panel", "chair", "assemble"),
            ("frame", "table", "assemble"),
            ("panel", "table", "assemble"),
            ("offcut", "wood", "recycle with loss"),
            ("offcut", "waste", "recycling loss"),
            ("durability", "wear", "production"),
            ("wear", "durability", "repair"),
            ("chair", "credit", "deliver"),
            ("table", "credit", "deliver"),
        ]
    } else {
        vec![
            ("wood", "chair", "basic recipe"),
            ("wood", "chair", "batch recipe"),
            ("wood", "waste", "by-product"),
            ("tool", "chair", "preserved catalyst"),
        ]
    };
    let edges = routes
        .into_iter()
        .enumerate()
        .map(|(index, (from, to, label))| GraphEdgeView {
            id: slug(format!("recipe/{index}")),
            source: slug(from),
            target: slug(to),
            label: Some(label.into()),
            classes: vec![],
        })
        .collect();
    let stocks = assets.into_iter().filter_map(|(key, label, asset)| {
        let quantity = world.balance(&AccountId::Workshop, &asset);
        if quantity.is_zero() {
            return None;
        }
        let mut entity = link_balance::<workshop::Ontology>(
            visual_entity(
                slug(format!("stock/{key}")),
                label,
                match asset {
                    Asset::Tool => SceneGlyphView::Tool,
                    Asset::Chair => SceneGlyphView::Product,
                    Asset::Waste => SceneGlyphView::Material,
                    _ => SceneGlyphView::Material,
                },
                SceneAnchorView::GraphNode { node: slug(key) },
                SceneEntityRoleView::State,
                if matches!(asset, Asset::Chair) {
                    SceneToneView::Success
                } else if matches!(asset, Asset::Waste) {
                    SceneToneView::Warning
                } else {
                    SceneToneView::Active
                },
                Some(format!(
                    "{} available",
                    asset.studio_format().write(quantity)
                )),
            ),
            &AccountId::Workshop,
            &asset,
        );
        entity.metrics = vec![asset_metric(slug(key), label, &asset, quantity)];
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
                slug(format!("contract/{order}")),
                format!("Contract {}", order + 1),
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: slug("credit"),
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
            slug("escrow"),
            "Customer funds",
            &Asset::Credit,
            world.balance(id, &Asset::Credit),
        )];
        Some(entity)
    });
    let metrics = if world.account(&AccountId::Customer(0)).is_some() {
        vec![
            asset_metric(
                slug("completed"),
                "Contracts delivered",
                &Asset::Completed,
                world.balance(&AccountId::Workshop, &Asset::Completed),
            ),
            asset_metric(
                slug("revenue"),
                "Revenue",
                &Asset::Credit,
                world.balance(&AccountId::Workshop, &Asset::Credit),
            ),
            asset_metric(
                slug("durability"),
                "Tool durability",
                &Asset::Durability,
                world.balance(&AccountId::Workshop, &Asset::Durability),
            ),
            asset_metric(
                slug("repairs"),
                "Repair kits remaining",
                &Asset::RepairKit,
                world.balance(&AccountId::Workshop, &Asset::RepairKit),
            ),
            visual_metric(
                slug("upgrade"),
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
                slug("waste"),
                "Waste",
                &Asset::Waste,
                workshop::waste(world),
            ),
            asset_metric(
                slug("time"),
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

impl StudioFormat for workshop::Asset {
    fn studio_format(&self) -> QuantityFormat {
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

impl StudioLabel for workshop::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Workshop => "Workshop".into(),
            Self::Customer(id) => format!("Customer {}", id + 1),
        }
    }
}

impl StudioLabel for workshop::Asset {
    fn studio_label(&self) -> String {
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

impl StudioLabel for workshop::RateId {
    fn studio_label(&self) -> String {
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
