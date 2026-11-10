use super::*;
use axionomy::{Exchange, Quantity, Trace};
use axionomy_problems::exact_cover::{self, AccountId, Asset, RateId, Role, SetId, World};
use axionomy_view::{MatrixCellView, TelemetryKindView, ViewId};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    let initial = match profile {
        InstanceProfile::Micro => exact_cover::initial(),
        InstanceProfile::Showcase => exact_cover::initial_showcase(),
        InstanceProfile::Stress => exact_cover::initial_stress(),
    };
    let bfs = exact_cover::solve_bfs(&initial)
        .ok_or_else(|| problem_error("exact-cover", "BFS found no cover"))?;
    let algorithm_x = exact_cover::algorithm_x(&initial)
        .ok_or_else(|| problem_error("exact-cover", "Algorithm X found no cover"))?;
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in [
        (
            "breadth-first",
            "Exact cover · plain search",
            "Breadth-first search finds a cover with the fewest selections. Larger sets can beat a cover made from pairs.",
            bfs.trace().clone(),
            "breadth-first search",
            Some(bfs.expanded() as u64),
        ),
        (
            "algorithm-x",
            "Exact cover · Algorithm X",
            "Algorithm X branches on the most constrained uncovered element. It can select a different complete cover than shortest-path search.",
            algorithm_x,
            "Algorithm X",
            None,
        ),
    ] {
        let mut view = document(
            progress,
            DocumentSpec {
                problem: "exact-cover",
                strategy,
                title,
                description,
                source_label: "Exact cover",
            },
            &initial,
            &exact_cover::goal(),
            &trace,
            Vec::new(),
            scene,
        )
        .map_err(|error| problem_error("exact-cover", error))?;
        let mut points = vec![(
            TelemetryKindView::Generated,
            trace.exchanges().len() as u64,
            "exchanges proposed".into(),
        )];
        if let Some(expanded) = expanded {
            points.insert(
                0,
                (
                    TelemetryKindView::Expanded,
                    expanded,
                    "states explored".into(),
                ),
            );
        }
        view.telemetry.push(telemetry(algorithm, true, points));
        let overlapping = Exchange::new(
            RateId::Select {
                set: SetId::Ab,
                before: 2,
            },
            Quantity::new(1),
        )
        .bind(Role::Problem, AccountId::Problem);
        view.proposals.push(proposal(ProposalSpec { id: "wrong-progress", label: "Select AB at progress 2", description: "This proposal claims two elements are already covered, but the initial state has zero covered elements." }, &initial, &overlapping));
        documents.push(view);
    }
    let unsatisfiable = match profile {
        InstanceProfile::Micro => exact_cover::unsatisfiable(),
        InstanceProfile::Showcase => exact_cover::unsatisfiable_showcase(),
        InstanceProfile::Stress => exact_cover::unsatisfiable_stress(),
    };
    let mut unsat = document(progress, DocumentSpec { problem: "exact-cover", strategy: "unsatisfiable", title: "Exact cover · no solution exists", description: "Same rules, different subsets: no combination covers every element exactly once. The impossibility is a result, not an error.", source_label: "Exact cover" }, &unsatisfiable, &exact_cover::goal(), &Trace::new(), Vec::new(), scene).map_err(|error| problem_error("exact-cover", error))?;
    if exact_cover::algorithm_x(&unsatisfiable).is_some()
        || exact_cover::solve_bfs(&unsatisfiable).is_some()
    {
        return Err(problem_error(
            "exact-cover",
            "supposed unsatisfiable instance has a cover",
        ));
    }
    unsat.telemetry.push(telemetry(
        "exhaustive search (proved no solution)",
        true,
        [(TelemetryKindView::Message, 0, "no exact cover".into())],
    ));
    documents.push(unsat);
    assemble(study, descriptor, documents)
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let sets = exact_cover::sets(world);
    let elements = exact_cover::elements(world);
    let rows = sets
        .iter()
        .copied()
        .map(|set| ViewId::new(path!("set").then(&set), subset(set)))
        .collect();
    let columns = elements
        .iter()
        .copied()
        .map(|element| ViewId::new(path!("element").then(&element), format!("{element:?}")))
        .collect();
    let cells = sets
        .iter()
        .copied()
        .flat_map(|set| {
            elements
                .iter()
                .copied()
                .filter(move |element| exact_cover::members(world, set).contains(element))
                .map(move |element| {
                    let selected = !world
                        .balance(&AccountId::Problem, &Asset::Selected(set))
                        .is_zero();
                    let blocked = !selected
                        && exact_cover::members(world, set).iter().any(|member| {
                            !world
                                .balance(&AccountId::Problem, &Asset::Covered(*member))
                                .is_zero()
                        });
                    MatrixCellView {
                        row: path!("set").then(&set),
                        column: path!("element").then(&element),
                        label: "×".into(),
                        classes: if selected {
                            vec![path!("selected")]
                        } else if blocked {
                            vec![path!("blocked")]
                        } else {
                            Vec::new()
                        },
                    }
                })
        })
        .collect();
    Some(
        Scene::matrix("Which subset contains which element", rows, columns, cells).with_metrics([
            visual_metric(
                path!("covered"),
                "Elements covered",
                elements
                    .iter()
                    .filter(|element| {
                        !world
                            .balance(&AccountId::Problem, &Asset::Covered(**element))
                            .is_zero()
                    })
                    .count(),
                None,
            ),
            visual_metric(
                path!("selected"),
                "Subsets selected",
                sets.iter()
                    .filter(|set| {
                        !world
                            .balance(&AccountId::Problem, &Asset::Selected(**set))
                            .is_zero()
                    })
                    .count(),
                None,
            ),
            visual_metric(
                path!("choices"),
                "Legal next selections",
                exact_cover::candidates(world)
                    .iter()
                    .filter(|action| matches!(action.rate(), RateId::Select { .. }))
                    .count(),
                None,
            ),
            visual_metric(
                path!("completion"),
                "Completion possible",
                if exact_cover::algorithm_x(world).is_some() {
                    "yes"
                } else {
                    "no"
                },
                None,
            ),
        ]),
    )
}

// How Studio names and writes this problem's values.

counted!(exact_cover::Asset);

/// A subset named by its elements, as the matrix and proposals write it: `AB`.
fn subset(set: SetId) -> String {
    format!("{set:?}").to_ascii_uppercase()
}

impl StudioLabel for exact_cover::AccountId {
    fn studio_label(&self) -> String {
        "Exact-cover problem".into()
    }
}

impl StudioLabel for exact_cover::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::Uncovered(element) => format!("Element {element:?} is uncovered"),
            Self::Covered(element) => format!("Element {element:?} is covered"),
            Self::Available(set) => format!("Subset {} is available", subset(*set)),
            Self::Selected(set) => format!("Subset {} selected", subset(*set)),
            Self::Progress(count) => format!("{count} elements covered"),
            Self::Active => "Cover in progress".into(),
            Self::Solved => "Exact cover found".into(),
        }
    }
}

impl StudioLabel for exact_cover::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Select { set, before } => format!(
                "Select subset {} after covering {before} elements",
                subset(*set)
            ),
            Self::Finish => "Accept the exact cover".into(),
        }
    }
}

debug_labels!(exact_cover::Role);
