use super::*;
use axionomy_problems::workshop::{self, AccountId, Asset, ObjectiveKey, RateId, World};
use axionomy_search::pareto::Objective;
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ParetoFrontView, ParetoPointView, SceneAnchorView, SceneGlyphView,
    SceneToneView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let initial = match instance_profile(request, descriptor) {
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
    let traces = [
        (
            "breadth_first",
            "Workshop · fewest steps",
            "Breadth-first search minimises the number of steps, ignoring how much material is wasted.",
            bfs.trace().clone(),
            "breadth-first search",
            bfs.expanded() as u64,
        ),
        (
            "minimum_waste",
            "Workshop · least waste",
            "Best-first search follows the scrap accumulating in the workshop account.",
            best.trace().clone(),
            "best-first search",
            best.expanded() as u64,
        ),
        (
            "pareto_waste",
            "Workshop Pareto · least waste",
            "The least wasteful plan on the frontier.",
            waste_trace,
            "exact Pareto search",
            pareto.progress().expanded() as u64,
        ),
        (
            "pareto_time",
            "Workshop Pareto · least time",
            "The fastest plan on the frontier.",
            time_trace,
            "exact Pareto search",
            pareto.progress().expanded() as u64,
        ),
    ];
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error("workshop", error))?;
        let mut view = document(
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
        view.proposals.push(proposal("workshop", ProposalSpec { id: "counterfeit", label: "Counterfeit chair", description: "This rule would create material out of nothing, and the conservation law rejects it." }, &initial, &counterfeit));
        documents.push(view);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn frontier_trace(
    result: &workshop::ParetoResult,
    waste_first: bool,
) -> Result<axionomy::Trace<RateId, workshop::Role, AccountId>, ServiceError> {
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
    vec![
        ObjectiveView {
            key: "waste".into(),
            label: "Waste".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: workshop::waste(world).to_string(),
        },
        ObjectiveView {
            key: "time".into(),
            label: "Process time".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: workshop::spent_time(world).to_string(),
        },
    ]
}

fn front_view(result: &workshop::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    let selected = selected
        .objectives
        .iter()
        .map(|objective| objective.value.as_str())
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: "Waste vs. time — every non-dominated plan".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: "waste".into(),
                label: "Waste".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "time".into(),
                label: "Time".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
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
                    label: format!("{} waste · {} time", values[0], values[1]),
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
    let assets = [
        ("wood", "Wood", Asset::Wood),
        ("labor", "Labor", Asset::Labor),
        ("tool", "Tool", Asset::Tool),
        ("chair", "Chair", Asset::Chair),
        ("waste", "Waste", Asset::Waste),
    ];
    let nodes = assets
        .into_iter()
        .enumerate()
        .map(|(index, (key, label, asset))| GraphNodeView {
            id: ViewId::new(
                key,
                format!("{label} · {}", world.balance(&AccountId::Workshop, &asset)),
            ),
            classes: if matches!(asset, Asset::Chair) {
                vec!["goal".into()]
            } else {
                Vec::new()
            },
            x: Some((index as f64 % 3.0) * 220.0 + 80.0),
            y: Some((index / 3) as f64 * 150.0 + 70.0),
        })
        .collect();
    let edges = vec![
        GraphEdgeView {
            id: "basic:wood".into(),
            source: "wood".into(),
            target: "chair".into(),
            label: Some("basic recipe".into()),
            classes: Vec::new(),
        },
        GraphEdgeView {
            id: "efficient:wood".into(),
            source: "wood".into(),
            target: "chair".into(),
            label: Some("batch recipe".into()),
            classes: Vec::new(),
        },
        GraphEdgeView {
            id: "waste".into(),
            source: "wood".into(),
            target: "waste".into(),
            label: Some("by-product".into()),
            classes: Vec::new(),
        },
        GraphEdgeView {
            id: "tool".into(),
            source: "tool".into(),
            target: "chair".into(),
            label: Some("preserved catalyst".into()),
            classes: Vec::new(),
        },
    ];
    let stocks = assets.into_iter().filter_map(|(key, label, asset)| {
        let quantity = world.balance(&AccountId::Workshop, &asset);
        if quantity.is_zero() {
            return None;
        }
        let mut entity = link_balance(
            visual_entity(
                format!("stock:{key}"),
                label,
                match asset {
                    Asset::Tool => SceneGlyphView::Tool,
                    Asset::Chair => SceneGlyphView::Product,
                    Asset::Waste => SceneGlyphView::Material,
                    _ => SceneGlyphView::Material,
                },
                SceneAnchorView::GraphNode { node: key.into() },
                if matches!(asset, Asset::Chair) {
                    SceneToneView::Success
                } else if matches!(asset, Asset::Waste) {
                    SceneToneView::Warning
                } else {
                    SceneToneView::Active
                },
                Some(format!("{quantity} available")),
            ),
            "workshop:account:workshop",
            format!("workshop:asset:{asset:?}").to_ascii_lowercase(),
        );
        entity.metrics = vec![visual_metric(key, label, quantity, Some("units"))];
        Some(entity)
    });
    Some(Scene::graph("Materials and recipe flow", nodes, edges, None).with_entities(stocks))
}
