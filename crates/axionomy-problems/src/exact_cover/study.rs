//! Exact cover as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    exact_cover::{self, AccountId, RateId, Role, SetId},
    support::{ProblemCopy, StrategyCopy, problem},
};
use axionomy::{Exchange, Quantity, Trace, path};
use axionomy_study::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, StudyError, StudyKey, StudyProgress, document, problem_error, proposal,
    telemetry,
};
use axionomy_view::{TelemetryKindView, ViewDocument};

/// Exact cover: its catalog entry and how its studies are computed.
pub struct ExactCover {
    descriptor: ProblemDescriptor,
}

impl Default for ExactCover {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("exact-cover"),
                    title: "Exact cover",
                    summary: "Choose subsets that cover every element exactly once. A plain search and Algorithm X reach the same answer by different routes.",
                    instances: [
                        "Four elements and four competing subsets",
                        "Eight coupled elements, fourteen irregular subsets, legal dead ends and covers of different sizes",
                        "Twelve globally coupled elements and twenty-five pairs, triples and quadruples",
                    ],
                },
                ProblemFamily::Constraint,
                path!("algorithm-x"),
                &[
                    StrategyCopy {
                        key: path!("breadth-first"),
                        label: "Plain search",
                        description: "Breadth-first search with no knowledge of cover structure; it just tries selections.",
                        algorithm: "breadth-first search",
                    },
                    StrategyCopy {
                        key: path!("algorithm-x"),
                        label: "Algorithm X",
                        description: "Knuth's Algorithm X chooses the moves; the economy still validates every one.",
                        algorithm: "Algorithm X",
                    },
                ],
                &[
                    Capability::DeterministicSearch,
                    Capability::SpecializedAlgorithm,
                    Capability::FeasibilityAssessment,
                ],
            ),
        }
    }
}

impl Problem for ExactCover {
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
        InstanceProfile::Micro => exact_cover::initial(),
        InstanceProfile::Showcase => exact_cover::initial_showcase(),
        InstanceProfile::Stress => exact_cover::initial_stress(),
    };
    let bfs = exact_cover::solve_bfs(&initial)
        .ok_or_else(|| problem_error(&study.problem, "BFS found no cover"))?;
    let algorithm_x = exact_cover::algorithm_x(&initial)
        .ok_or_else(|| problem_error(&study.problem, "Algorithm X found no cover"))?;
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in [
        (
            path!("breadth-first"),
            "Exact cover · plain search",
            "Breadth-first search finds a cover with the fewest selections. Larger sets can beat a cover made from pairs.",
            bfs.trace().clone(),
            "breadth-first search",
            Some(bfs.expanded() as u64),
        ),
        (
            path!("algorithm-x"),
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
                problem: &study.problem,
                document: strategy,
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
        .map_err(|error| problem_error(&study.problem, error))?;
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
        view.proposals.push(proposal(ProposalSpec { id: path!("wrong-progress"), label: "Select AB at progress 2", description: "This proposal claims two elements are already covered, but the initial state has zero covered elements." }, &initial, &overlapping));
        documents.push(view);
    }
    let unsatisfiable = match profile {
        InstanceProfile::Micro => exact_cover::unsatisfiable(),
        InstanceProfile::Showcase => exact_cover::unsatisfiable_showcase(),
        InstanceProfile::Stress => exact_cover::unsatisfiable_stress(),
    };
    let mut unsat = document(progress, DocumentSpec { problem: &study.problem, document: path!("unsatisfiable"), title: "Exact cover · no solution exists", description: "Same rules, different subsets: no combination covers every element exactly once. The impossibility is a result, not an error.", source_label: "Exact cover" }, &unsatisfiable, &exact_cover::goal(), &Trace::new(), Vec::new(), scene).map_err(|error| problem_error(&study.problem, error))?;
    if exact_cover::algorithm_x(&unsatisfiable).is_some()
        || exact_cover::solve_bfs(&unsatisfiable).is_some()
    {
        return Err(problem_error(
            &study.problem,
            "supposed unsatisfiable instance has a cover",
        ));
    }
    unsat.telemetry.push(telemetry(
        "exhaustive search (proved no solution)",
        true,
        [(TelemetryKindView::Message, 0, "no exact cover".into())],
    ));
    documents.push(unsat);
    Ok(documents)
}
