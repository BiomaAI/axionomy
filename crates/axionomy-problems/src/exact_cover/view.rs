//! How exact cover is presented: its labels, units, scene and leaderboards.

use crate::exact_cover::{self, AccountId, Asset, RateId, SetId, World};
use axionomy::path;
use axionomy_view::{
    Keyed, Label, MatrixCellView, Scene, ViewId, counted, debug_labels, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
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

impl Label for exact_cover::AccountId {
    fn label(&self) -> String {
        "Exact-cover problem".into()
    }
}

impl Label for exact_cover::Asset {
    fn label(&self) -> String {
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

impl Label for exact_cover::RateId {
    fn label(&self) -> String {
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
