use super::*;
use axionomy::{Exchange, Quantity, Trace};
use axionomy_problems::exact_cover::{self, AccountId, Asset, RateId, Role, SetId, World};
use axionomy_view::{MatrixCellView, TelemetryKindView, ViewId};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let showcase = !matches!(
        instance_profile(request, descriptor),
        InstanceProfile::Micro
    );
    let initial = if showcase {
        exact_cover::initial_showcase()
    } else {
        exact_cover::initial()
    };
    let bfs = exact_cover::solve_bfs(&initial)
        .ok_or_else(|| problem_error("exact_cover", "BFS found no cover"))?;
    let algorithm_x = exact_cover::algorithm_x(&initial)
        .ok_or_else(|| problem_error("exact_cover", "Algorithm X found no cover"))?;
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in [
        (
            "breadth_first",
            "Exact cover · generic BFS",
            "Generic graph search treats the economy as a state machine.",
            bfs.trace().clone(),
            "breadth-first search",
            Some(bfs.expanded() as u64),
        ),
        (
            "algorithm_x",
            "Exact cover · Algorithm X",
            "A traditional exact-cover algorithm reads the encoded set membership and proposes the same core-valid transitions.",
            algorithm_x,
            "Algorithm X",
            None,
        ),
    ] {
        let mut view = document(
            DocumentSpec {
                problem: "exact_cover",
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
        .map_err(|error| problem_error("exact_cover", error))?;
        let mut points = vec![(
            TelemetryKindView::Generated,
            trace.exchanges().len() as u64,
            "proposal exchanges".into(),
        )];
        if let Some(expanded) = expanded {
            points.insert(
                0,
                (
                    TelemetryKindView::Expanded,
                    expanded,
                    "states expanded".into(),
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
        view.proposals.push(proposal("exact_cover", ProposalSpec { id: "wrong-progress", label: "Select AB at progress 2", description: "The set is available, but the encoded progress token and already-covered elements make this infeasible." }, &initial, &overlapping));
        documents.push(view);
    }
    let unsatisfiable = if showcase {
        exact_cover::unsatisfiable_showcase()
    } else {
        exact_cover::unsatisfiable()
    };
    let mut unsat = document(DocumentSpec { problem: "exact_cover", strategy: "unsatisfiable", title: "Exact cover · unsatisfiable instance", description: "The same model surface exposes a replayable instance whose available sets cannot cover the universe exactly.", source_label: "Exact cover" }, &unsatisfiable, &exact_cover::goal(), &Trace::new(), Vec::new(), scene).map_err(|error| problem_error("exact_cover", error))?;
    unsat.telemetry.push(telemetry(
        "exhaustive search",
        true,
        [(TelemetryKindView::Message, 0, "no exact cover".into())],
    ));
    documents.push(unsat);
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let sets = exact_cover::sets(world);
    let elements = exact_cover::elements(world);
    let rows = sets
        .iter()
        .copied()
        .map(|set| ViewId::new(format!("set:{set:?}"), format!("{set:?}")))
        .collect();
    let columns = elements
        .iter()
        .copied()
        .map(|element| ViewId::new(format!("element:{element:?}"), format!("{element:?}")))
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
                    MatrixCellView {
                        row: format!("set:{set:?}"),
                        column: format!("element:{element:?}"),
                        label: "×".into(),
                        classes: if selected {
                            vec!["selected".into()]
                        } else {
                            Vec::new()
                        },
                    }
                })
        })
        .collect();
    Some(Scene::Matrix {
        title: "Encoded set × universe incidence".into(),
        rows,
        columns,
        cells,
    })
}
