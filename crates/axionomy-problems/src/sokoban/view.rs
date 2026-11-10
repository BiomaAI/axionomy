//! How the Sokoban warehouse is presented: its labels, units, scene and leaderboards.

use crate::sokoban::{self, AccountId, Asset, World};
use axionomy::path;
use axionomy_view::{
    GridCellView, Keyed, Label, Scene, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, account_key, counted, link_balance, visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let (width, height) = sokoban::dimensions(world);
    let crates = sokoban::crate_cells(world);
    let player = sokoban::player_cell(world);
    let mut cells = Vec::with_capacity(usize::from(width * height));
    let mut entities = Vec::with_capacity(crates.len() + usize::from(player.is_some()));

    for cell in 0..width * height {
        let account = AccountId::Cell(cell);
        let wall = sokoban::is_wall(world, cell);
        let goal = !world.balance(&account, &Asset::GoalCell).is_zero();
        let dead = sokoban::is_dead_square(world, cell);
        let mut classes = vec![if wall { path!("wall") } else { path!("floor") }];
        if goal {
            classes.push(path!("goal"));
        }
        if dead {
            classes.push(path!("dead-square"));
        }
        cells.push(GridCellView {
            x: u32::from(cell % width),
            y: u32::from(cell / width),
            label: if wall {
                "Wall".into()
            } else if goal {
                "Goal floor".into()
            } else if dead {
                "No crate route to any goal".into()
            } else {
                "Warehouse floor".into()
            },
            classes,
            account: Some(account_key::<sokoban::Ontology>(&account)),
        });

        if !world.balance(&account, &Asset::ExitCell).is_zero() {
            entities.push(visual_entity(
                path!("exit"),
                "Exit",
                SceneGlyphView::Goal,
                SceneAnchorView::GridCell {
                    x: u32::from(cell % width),
                    y: u32::from(cell / width),
                },
                SceneEntityRoleView::Structure,
                SceneToneView::Goal,
                Some("leave after all deliveries".into()),
            ));
        }
        for circuit in 0..2 {
            for (asset, name, glyph) in [
                (Asset::Plate(circuit), "Plate", SceneGlyphView::Constraint),
                (Asset::Door(circuit), "Door", SceneGlyphView::Door),
                (
                    Asset::Panel(circuit),
                    "Latch panel",
                    SceneGlyphView::Constraint,
                ),
            ] {
                if world.balance(&account, &asset).is_zero() {
                    continue;
                }
                let pressed = !world
                    .balance(&AccountId::Success, &Asset::Pressed(circuit))
                    .is_zero();
                let latched = !world
                    .balance(&AccountId::Success, &Asset::Latched(circuit))
                    .is_zero();
                let state = match asset {
                    Asset::Plate(_) => {
                        if pressed {
                            "holding door open"
                        } else {
                            "needs a crate"
                        }
                    }
                    Asset::Door(_) => {
                        if latched {
                            "latched open"
                        } else if pressed {
                            "open under pressure"
                        } else {
                            "closed"
                        }
                    }
                    _ => {
                        if latched {
                            "latched"
                        } else {
                            "ready to latch"
                        }
                    }
                };
                entities.push(visual_entity(
                    path!("mechanism").then(&asset),
                    format!("{name} {}", circuit + 1),
                    glyph,
                    SceneAnchorView::GridCell {
                        x: u32::from(cell % width),
                        y: u32::from(cell / width),
                    },
                    SceneEntityRoleView::Structure,
                    if latched || pressed {
                        SceneToneView::Success
                    } else {
                        SceneToneView::Warning
                    },
                    Some(state.into()),
                ));
            }
        }
        if player == Some(cell) {
            entities.push(link_balance::<sokoban::Ontology>(
                visual_entity(
                    path!("player"),
                    "Player",
                    SceneGlyphView::Agent,
                    SceneAnchorView::GridCell {
                        x: u32::from(cell % width),
                        y: u32::from(cell / width),
                    },
                    SceneEntityRoleView::Occupant,
                    SceneToneView::Active,
                    Some("player".into()),
                ),
                &account,
                &Asset::Player,
            ));
        }
        if let Some((crate_id, _)) = crates.iter().find(|(_, at)| *at == cell) {
            let on_goal = goal;
            let trapped = dead && !on_goal;
            entities.push(link_balance::<sokoban::Ontology>(
                visual_entity(
                    path!("crate").then(crate_id),
                    format!("Crate {}", crate_id + 1),
                    SceneGlyphView::Package,
                    SceneAnchorView::GridCell {
                        x: u32::from(cell % width),
                        y: u32::from(cell / width),
                    },
                    SceneEntityRoleView::Occupant,
                    if on_goal {
                        SceneToneView::Success
                    } else if trapped {
                        SceneToneView::Danger
                    } else {
                        SceneToneView::Warning
                    },
                    Some(if on_goal {
                        "on-goal".into()
                    } else if trapped {
                        "deadlocked".into()
                    } else {
                        "crate".into()
                    }),
                ),
                &account,
                &Asset::Crate(*crate_id),
            ));
        }
    }

    Some(
        Scene::grid(
            "Warehouse floor, stable crates, goals, and dead squares",
            u32::from(width),
            u32::from(height),
            cells,
        )
        .with_entities(entities)
        .with_metrics([
            visual_metric(
                path!("held-plates"),
                "Crates holding plates",
                (0..2)
                    .filter(|circuit| {
                        !world
                            .balance(&AccountId::Success, &Asset::Pressed(*circuit))
                            .is_zero()
                    })
                    .count(),
                None,
            ),
            visual_metric(
                path!("latched"),
                "Doors latched open",
                (0..2)
                    .filter(|circuit| {
                        !world
                            .balance(&AccountId::Success, &Asset::Latched(*circuit))
                            .is_zero()
                    })
                    .count(),
                None,
            ),
            visual_metric(
                path!("goals"),
                "Crates on goals",
                crates
                    .iter()
                    .filter(|(_, at)| {
                        !world
                            .balance(&AccountId::Cell(*at), &Asset::GoalCell)
                            .is_zero()
                    })
                    .count(),
                None,
            ),
        ]),
    )
}

// How Studio names and writes this problem's values.

counted!(sokoban::Asset);

impl Label for sokoban::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Cell(cell) => format!("Cell {cell}"),
            Self::Success => "Puzzle".into(),
        }
    }
}

impl Label for sokoban::Asset {
    fn label(&self) -> String {
        match self {
            Self::DeadSquare => "A crate cannot reach a goal from here".into(),
            Self::ExitCell => "Warehouse exit".into(),
            Self::AtExit => "Player at the exit".into(),
            Self::AwayFromExit => "Player inside the warehouse".into(),
            Self::Plate(id) => format!("Pressure plate {}", id + 1),
            Self::Door(id) => format!("Door {}", id + 1),
            Self::Panel(id) => format!("Latch panel {}", id + 1),
            Self::Pressed(id) => format!("Plate {} has a crate", id + 1),
            Self::Released(id) => format!("Plate {} is released", id + 1),
            Self::LatchReady(id) => format!("Door {} can be latched", id + 1),
            Self::Latched(id) => format!("Door {} latched open", id + 1),
            Self::CellIdentity(cell) => format!("This is cell {cell}"),
            Self::BoardWidth(width) => format!("Board width is {width}"),
            Self::BoardHeight(height) => format!("Board height is {height}"),
            Self::Floor => "Walkable floor".into(),
            Self::Wall => "Wall".into(),
            Self::Player => "Player".into(),
            Self::Crate(crate_id) => format!("Crate {}", crate_id + 1),
            Self::Empty => "Empty cell".into(),
            Self::GoalCell => "Goal square".into(),
            Self::Active => "Puzzle in progress".into(),
            Self::Solved => "Puzzle solved".into(),
        }
    }
}

impl Label for sokoban::RateId {
    fn label(&self) -> String {
        match self {
            Self::Latch { circuit, at } => {
                format!("Latch door {} from panel at cell {at}", circuit + 1)
            }
            Self::Move { from, to } => format!("Move from cell {from} to cell {to}"),
            Self::Push {
                behind,
                crate_at,
                to,
                crate_id,
            } => format!(
                "Push crate {} from cell {crate_at} to cell {to}, standing at cell {behind}",
                crate_id + 1
            ),
            // Where each crate rests is the exchange's binding of its goal role.
            Self::Finish => "Finish".into(),
        }
    }
}

impl Label for sokoban::Role {
    fn label(&self) -> String {
        match self {
            Self::From => "Player's cell".into(),
            Self::Middle => "Crate's cell".into(),
            Self::To => "Destination cell".into(),
            Self::Puzzle => "Puzzle".into(),
            Self::Goal(crate_id) => format!("Goal for crate {}", crate_id + 1),
        }
    }
}
