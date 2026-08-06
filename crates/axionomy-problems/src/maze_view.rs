//! Read-only Studio projection for the key-door maze.

use crate::maze::{self, AccountId, Asset, Node, ObjectiveKey, RateId, Role, World};
use axionomy::Trace;
use axionomy_view::{
    FrontierCompletenessView, GraphEdgeView, GraphNodeView, ObjectiveAxisView,
    ObjectiveDirectionView, ObjectiveView, ParetoFrontView, ParetoPointView, PlaybackError, Scene,
    ViewDocument, ViewId, ViewOntology, ViewSource, derive_document,
};
use std::fmt;
use thiserror::Error;

type MazeTrace = Trace<RateId, Role, AccountId>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MazeStrategy {
    BreadthFirst,
    AStar,
    ParetoEnergy,
    ParetoTime,
}

impl MazeStrategy {
    pub const ALL: [Self; 4] = [
        Self::BreadthFirst,
        Self::AStar,
        Self::ParetoEnergy,
        Self::ParetoTime,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::BreadthFirst => "maze_breadth_first",
            Self::AStar => "maze_a_star",
            Self::ParetoEnergy => "maze_pareto_energy",
            Self::ParetoTime => "maze_pareto_time",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::BreadthFirst => "Maze · fewest exchanges",
            Self::AStar => "Maze · least energy",
            Self::ParetoEnergy => "Maze Pareto · least energy",
            Self::ParetoTime => "Maze Pareto · least time",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::BreadthFirst => {
                "Breadth-first search selects the short detour, spending more energy in fewer exchanges."
            }
            Self::AStar => {
                "A* uses encoded distance and energy to select the longer key-and-door route."
            }
            Self::ParetoEnergy => {
                "The energy-minimizing member of the exact replay-verified Pareto frontier."
            }
            Self::ParetoTime => {
                "The time-minimizing member of the exact replay-verified Pareto frontier."
            }
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|strategy| strategy.key() == key)
    }
}

#[derive(Debug, Error)]
pub enum MazeViewError {
    #[error("maze strategy produced no solution")]
    NoSolution,
    #[error("maze Pareto search failed: {0}")]
    Pareto(String),
    #[error(transparent)]
    Playback(#[from] PlaybackError),
}

pub fn document(strategy: MazeStrategy) -> Result<ViewDocument, MazeViewError> {
    let initial = maze::initial();
    let trace = trace_for(strategy, &initial)?;
    let final_world = initial.replayed(&trace).map_err(|error| {
        MazeViewError::Playback(PlaybackError::Replay {
            index: trace.exchanges().len() as u64,
            message: error.to_string(),
        })
    })?;
    let objectives = vec![
        ObjectiveView {
            key: "energy".into(),
            label: "Energy spent".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: maze::spent_energy(&final_world).to_string(),
        },
        ObjectiveView {
            key: "time".into(),
            label: "Time spent".into(),
            direction: ObjectiveDirectionView::Minimize,
            value: maze::spent_time(&final_world).to_string(),
        },
    ];

    let mut document = derive_document(
        strategy.key(),
        strategy.label(),
        strategy.description(),
        ViewSource {
            key: "maze".into(),
            label: "Key-door maze".into(),
        },
        &initial,
        &trace,
        &MazeOntology,
        objectives,
    )
    .map_err(MazeViewError::from)?;
    document
        .pareto_fronts
        .push(pareto_view(&initial, &document)?);
    Ok(document)
}

fn pareto_view(initial: &World, document: &ViewDocument) -> Result<ParetoFrontView, MazeViewError> {
    let result =
        maze::pareto_front(initial).map_err(|error| MazeViewError::Pareto(error.to_string()))?;
    let selected = document
        .objectives
        .iter()
        .map(|objective| objective.value.as_str())
        .collect::<Vec<_>>();
    let points = result
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
            let energy = &values[0];
            let time = &values[1];
            ParetoPointView {
                label: format!("{energy} energy · {time} time"),
                selected: values
                    .iter()
                    .map(String::as_str)
                    .eq(selected.iter().copied()),
                values,
            }
        })
        .collect();
    Ok(ParetoFrontView {
        title: "Replay-verified energy/time frontier".into(),
        completeness: FrontierCompletenessView::Exact,
        axes: vec![
            ObjectiveAxisView {
                key: "energy".into(),
                label: "Energy spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "time".into(),
                label: "Time spent".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points,
    })
}

fn trace_for(strategy: MazeStrategy, initial: &World) -> Result<MazeTrace, MazeViewError> {
    match strategy {
        MazeStrategy::BreadthFirst => maze::solve_bfs(initial)
            .map(|solution| solution.trace().clone())
            .ok_or(MazeViewError::NoSolution),
        MazeStrategy::AStar => maze::solve_astar(initial)
            .map(|solution| solution.trace().clone())
            .ok_or(MazeViewError::NoSolution),
        MazeStrategy::ParetoEnergy | MazeStrategy::ParetoTime => {
            let result = maze::pareto_front(initial)
                .map_err(|error| MazeViewError::Pareto(error.to_string()))?;
            result
                .front()
                .entries()
                .iter()
                .min_by_key(|entry| {
                    let values = entry.objectives().objectives();
                    let energy = objective_value(values, ObjectiveKey::Energy);
                    let time = objective_value(values, ObjectiveKey::Time);
                    match strategy {
                        MazeStrategy::ParetoEnergy => (energy, time),
                        MazeStrategy::ParetoTime => (time, energy),
                        MazeStrategy::BreadthFirst | MazeStrategy::AStar => unreachable!(),
                    }
                })
                .map(|entry| entry.payload().clone())
                .ok_or(MazeViewError::NoSolution)
        }
    }
}

fn objective_value(
    objectives: &[axionomy_search::pareto::Objective<ObjectiveKey, u64>],
    key: ObjectiveKey,
) -> u64 {
    objectives
        .iter()
        .find(|objective| objective.key() == &key)
        .map(|objective| *objective.value())
        .expect("maze objective is present")
}

struct MazeOntology;

impl ViewOntology<AccountId, Asset, RateId, Role> for MazeOntology {
    fn account(&self, id: &AccountId) -> ViewId {
        match id {
            AccountId::Agent => ViewId::new("account:agent", "Agent"),
            AccountId::World => ViewId::new("account:world", "World"),
        }
    }

    fn asset(&self, id: &Asset) -> ViewId {
        ViewId::new(format!("asset:{id:?}"), asset_label(id))
    }

    fn rate(&self, id: &RateId) -> ViewId {
        ViewId::new(format!("rate:{id:?}"), rate_label(id))
    }

    fn role(&self, id: &Role) -> ViewId {
        match id {
            Role::Actor => ViewId::new("role:actor", "Actor"),
            Role::Environment => ViewId::new("role:environment", "Environment"),
        }
    }

    fn scene(&self, _: u64, economy: &World) -> Option<Scene> {
        let focus = Node::ALL.into_iter().find(|node| {
            !economy
                .balance(&AccountId::Agent, &Asset::At(*node))
                .is_zero()
        });
        let nodes = Node::ALL
            .into_iter()
            .map(|node| {
                let (x, y) = node.position();
                let mut classes = Vec::new();
                if focus == Some(node) {
                    classes.push("current".into());
                }
                if node == Node::Exit {
                    classes.push("goal".into());
                }
                GraphNodeView {
                    id: ViewId::new(node.key(), node.to_string()),
                    classes,
                    x: Some(x),
                    y: Some(y),
                }
            })
            .collect();
        let edges = maze::EDGES
            .into_iter()
            .enumerate()
            .map(
                |(index, (from, to, energy, needs_open_door))| GraphEdgeView {
                    id: format!("edge:{index}"),
                    source: from.key().into(),
                    target: to.key().into(),
                    label: Some(if needs_open_door {
                        format!("{energy} energy · door")
                    } else {
                        format!("{energy} energy")
                    }),
                    classes: if needs_open_door
                        && economy.balance(&AccountId::World, &Asset::Open).is_zero()
                    {
                        vec!["locked".into()]
                    } else {
                        Vec::new()
                    },
                },
            )
            .collect();

        Some(Scene::Graph {
            title: "Encoded maze topology".into(),
            nodes,
            edges,
            focus: focus.map(|node| node.key().into()),
        })
    }
}

impl Node {
    const ALL: [Self; 5] = [
        Self::Start,
        Self::KeyRoom,
        Self::Door,
        Self::Detour,
        Self::Exit,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::Start => "node:start",
            Self::KeyRoom => "node:key_room",
            Self::Door => "node:door",
            Self::Detour => "node:detour",
            Self::Exit => "node:exit",
        }
    }

    const fn position(self) -> (f64, f64) {
        match self {
            Self::Start => (80.0, 150.0),
            Self::KeyRoom => (240.0, 65.0),
            Self::Door => (410.0, 65.0),
            Self::Detour => (310.0, 240.0),
            Self::Exit => (570.0, 150.0),
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Start => "Start",
            Self::KeyRoom => "Key room",
            Self::Door => "Door",
            Self::Detour => "Detour",
            Self::Exit => "Exit",
        })
    }
}

fn asset_label(asset: &Asset) -> String {
    match asset {
        Asset::At(node) => format!("At {node}"),
        Asset::Edge(from, to) => format!("Edge {from} → {to}"),
        Asset::Key => "Key".into(),
        Asset::Locked => "Locked".into(),
        Asset::Open => "Open".into(),
        Asset::Energy => "Energy".into(),
        Asset::SpentEnergy => "Spent energy".into(),
        Asset::Time => "Time".into(),
        Asset::SpentTime => "Spent time".into(),
        Asset::Target(node) => format!("Target {node}"),
        Asset::Distance(node) => format!("Distance from {node}"),
        Asset::Active => "Active".into(),
        Asset::Solved => "Solved".into(),
    }
}

fn rate_label(rate: &RateId) -> String {
    match rate {
        RateId::Move { from, to, .. } => format!("Move {from} → {to}"),
        RateId::TakeKey => "Take key".into(),
        RateId::UnlockDoor => "Unlock door".into(),
        RateId::Finish => "Finish".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_maze_strategy_builds_a_replay_verified_document() {
        for strategy in MazeStrategy::ALL {
            let document = document(strategy).unwrap();
            assert!(!document.frames.is_empty());
            assert_eq!(document.initial.index, 0);
            assert_eq!(
                document.frames.last().unwrap().after.index,
                document.frames.len() as u64
            );
            assert!(
                document
                    .frames
                    .iter()
                    .all(|frame| { frame.assessment.projected_deltas == frame.receipt.deltas })
            );
        }
    }

    #[test]
    fn pareto_documents_expose_distinct_tradeoffs() {
        let energy = document(MazeStrategy::ParetoEnergy).unwrap();
        let time = document(MazeStrategy::ParetoTime).unwrap();
        let values = |document: &ViewDocument| {
            document
                .objectives
                .iter()
                .map(|objective| objective.value.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(values(&energy), ["6".to_owned(), "6".to_owned()]);
        assert_eq!(values(&time), ["9".to_owned(), "3".to_owned()]);
    }
}
