//! Pareto fronts as views: the one way every problem shows an exact front,
//! an evaluated one or a sampled policy front.

use crate::{
    FrontierCompletenessView, ObjectiveAxisView, ObjectiveDirectionView, ParetoFrontView,
    ParetoPointView, ViewDocument,
};
use axionomy::Quantity;
use axionomy_search::{
    monte_carlo::PolicyEstimate,
    pareto::{ParetoEntry, ParetoFront},
};

/// The axes of a front over `document`'s objectives, in their order.
pub fn objective_axes(document: &ViewDocument) -> Vec<ObjectiveAxisView> {
    document
        .objectives
        .iter()
        .map(|objective| ObjectiveAxisView {
            key: objective.key.clone(),
            label: objective.label.clone(),
            direction: objective.direction,
        })
        .collect()
}

/// An entry's objective values, exact and in the front's order.
pub fn exact_values<T, K: Eq>(entry: &ParetoEntry<T, K, u64>) -> Vec<Quantity> {
    entry
        .objectives()
        .objectives()
        .iter()
        .map(|objective| Quantity::from(*objective.value()))
        .collect()
}

/// An entry's value of the objective `key`, zero if it has none.
pub fn objective_value<T, K: Eq>(entry: &ParetoEntry<T, K, u64>, key: &K) -> u64 {
    entry
        .objectives()
        .objectives()
        .iter()
        .find(|objective| objective.key() == key)
        .map_or(0, |objective| *objective.value())
}

/// An exact Pareto front: one point for each entry's values, in the order of
/// `axes` and labeled by `label`. The point whose values are the selected
/// document's objectives is selected.
pub fn exact_front(
    title: &str,
    axes: Vec<ObjectiveAxisView>,
    selected: &ViewDocument,
    points: impl IntoIterator<Item = Vec<Quantity>>,
    label: impl Fn(&[Quantity]) -> String,
) -> ParetoFrontView {
    let chosen = selected
        .objectives
        .iter()
        .map(|objective| objective.value)
        .collect::<Vec<_>>();
    ParetoFrontView {
        title: title.into(),
        completeness: FrontierCompletenessView::Exact,
        axes,
        points: points
            .into_iter()
            .map(|values| ParetoPointView {
                label: label(&values),
                selected: values == chosen,
                values: values.iter().map(ToString::to_string).collect(),
            })
            .collect(),
    }
}

/// Compare replayed outcomes without claiming an exhaustive frontier: the
/// documents no other document dominates on the selected one's objectives.
pub fn evaluated_front(
    documents: &[ViewDocument],
    selected: &ViewDocument,
    title: &str,
) -> ParetoFrontView {
    let rows: Vec<_> = documents
        .iter()
        .map(|document| {
            (
                document,
                document
                    .objectives
                    .iter()
                    .map(|objective| objective.value)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let dominates = |left: &[Quantity], right: &[Quantity]| {
        left.iter()
            .zip(right)
            .zip(&selected.objectives)
            .all(|((l, r), axis)| match axis.direction {
                ObjectiveDirectionView::Maximize => l >= r,
                ObjectiveDirectionView::Minimize => l <= r,
            })
            && left
                .iter()
                .zip(right)
                .zip(&selected.objectives)
                .any(|((l, r), axis)| match axis.direction {
                    ObjectiveDirectionView::Maximize => l > r,
                    ObjectiveDirectionView::Minimize => l < r,
                })
    };
    ParetoFrontView {
        title: title.into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: objective_axes(selected),
        points: rows
            .iter()
            .filter(|(_, candidate)| !rows.iter().any(|(_, other)| dominates(other, candidate)))
            .map(|(document, values)| ParetoPointView {
                label: document.title.clone(),
                selected: document.id == selected.id,
                values: values.iter().map(ToString::to_string).collect(),
            })
            .collect(),
    }
}

/// A front of fixed policies estimated by sampling: each point is a
/// policy's mean outcomes, written to three decimals and labeled by `label`.
pub fn sampled_policy_front<P: Copy, S, K: Eq>(
    title: &str,
    axes: Vec<ObjectiveAxisView>,
    front: &ParetoFront<PolicyEstimate<P, S>, K, f64>,
    label: impl Fn(P) -> String,
    selected: impl Fn(P) -> bool,
) -> ParetoFrontView {
    ParetoFrontView {
        title: title.into(),
        completeness: FrontierCompletenessView::Approximate,
        axes,
        points: front
            .entries()
            .iter()
            .map(|entry| {
                let policy = *entry.payload().policy();
                ParetoPointView {
                    label: label(policy),
                    values: entry
                        .objectives()
                        .objectives()
                        .iter()
                        .map(|objective| format!("{:.3}", objective.value()))
                        .collect(),
                    selected: selected(policy),
                }
            })
            .collect(),
    }
}
