//! How the key-door maze is presented: its labels, units, scene and leaderboards.

use crate::maze::{self, AccountId, Asset, Node, RateId, World};
use axionomy::path;
use axionomy_view::{
    ClassPath, Format, GraphEdgeView, GraphNodeView, ItemPath, Keyed, Label, QuantityFormat, Scene,
    SceneAnchorView, SceneEntityRoleView, SceneGlyphView, SceneLegendView, ScenePathStatusView,
    SceneSurfaceView, SceneToneView, ViewDocument, ViewId, asset_metric, debug_labels,
    link_balance, visual_entity, visual_metric, words,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let nodes = maze::nodes(world);
    let stress = nodes.contains(&Node::Archive);
    let focus = nodes.iter().copied().find(|node| {
        !world
            .balance(&AccountId::Agent, &Asset::At(*node))
            .is_zero()
    });
    let positions = |node| match (stress, node) {
        (_, Node::Start) => (40.0, 300.0),
        (_, Node::Atrium) => (220.0, 260.0),
        (_, Node::Library) => (400.0, 90.0),
        (_, Node::KeyRoom) => (590.0, 40.0),
        (_, Node::Gallery) => (500.0, 260.0),
        (_, Node::Gate) => (690.0, 260.0),
        (_, Node::Vault) => (870.0, 260.0),
        (_, Node::Exit) => (1060.0, 300.0),
        (_, Node::Garden) => (220.0, 460.0),
        (_, Node::Market) => (410.0, 460.0),
        (_, Node::Canal) => (610.0, 460.0),
        (_, Node::Bridge) => (820.0, 460.0),
        (_, Node::Workshop) => (410.0, 650.0),
        (_, Node::Tunnel) => (110.0, 650.0),
        (_, Node::Detour) => (720.0, 650.0),
        (true, Node::Archive) => (220.0, 50.0),
        (true, Node::Scriptorium) => (400.0, -90.0),
        (true, Node::Docks) => (300.0, 840.0),
        (true, Node::Foundry) => (500.0, 840.0),
        (true, Node::Tower) => (700.0, 840.0),
        (true, Node::Observatory) => (980.0, 650.0),
        (true, Node::Ridge) => (80.0, 850.0),
        (true, Node::Ruins) => (180.0, 1030.0),
        (true, Node::Chapel) => (600.0, 1030.0),
        // Nodes outside a profile are never rendered, but retaining a total
        // match keeps the projection robust if another profile is introduced.
        (false, Node::Archive) => (220.0, 50.0),
        (false, Node::Scriptorium) => (400.0, -90.0),
        (false, Node::Docks) => (300.0, 840.0),
        (false, Node::Foundry) => (500.0, 840.0),
        (false, Node::Tower) => (700.0, 840.0),
        (false, Node::Observatory) => (980.0, 650.0),
        (false, Node::Ridge) => (80.0, 850.0),
        (false, Node::Ruins) => (180.0, 1030.0),
        (false, Node::Chapel) => (600.0, 1030.0),
    };
    let node_key = |node| path!("node").then(&node);
    let graph_nodes = nodes
        .iter()
        .copied()
        .map(|node| {
            let (x, y) = positions(node);
            let mut classes = if node == Node::Gate {
                vec![path!("facility")]
            } else {
                vec![path!("location")]
            };
            if focus == Some(node) {
                classes.push(path!("current"));
            }
            if node == Node::Exit {
                classes.push(path!("goal"));
            }
            GraphNodeView {
                id: ViewId::new(node_key(node), node.label()),
                classes,
                x: Some(x),
                y: Some(y),
            }
        })
        .collect();
    let graph_edges = world
        .rate_ids()
        .filter_map(|rate| match rate {
            RateId::Move {
                from,
                to,
                energy,
                needs_open_door,
            } => Some(GraphEdgeView {
                id: path!("edge").then(from).then(to),
                source: node_key(*from),
                target: node_key(*to),
                label: Some(if *needs_open_door {
                    format!("{energy} E · gate")
                } else {
                    format!("{energy} E")
                }),
                classes: if (*needs_open_door
                    && (world.balance(&AccountId::World, &Asset::Open).is_zero()
                        || (!world.balance(&AccountId::Agent, &Asset::Health).is_zero()
                            && world
                                .balance(&AccountId::World, &Asset::PowerVault)
                                .is_zero())))
                    || ((*from, *to) == (Node::Canal, Node::Bridge)
                        && world
                            .balance(&AccountId::World, &Asset::PowerTransit)
                            .is_zero()
                        && !world.balance(&AccountId::Agent, &Asset::Health).is_zero())
                {
                    vec![path!("locked")]
                } else {
                    Vec::new()
                },
            }),
            _ => None,
        })
        .collect();
    let agent = focus.map(|node| {
        let energy = world.balance(&AccountId::Agent, &Asset::Energy);
        let time = world.balance(&AccountId::Agent, &Asset::Time);
        let mut entity = link_balance::<maze::Ontology>(
            visual_entity(
                path!("agent/explorer"),
                "Explorer",
                SceneGlyphView::Agent,
                SceneAnchorView::GraphNode {
                    node: node_key(node),
                },
                SceneEntityRoleView::Occupant,
                if node == Node::Exit {
                    SceneToneView::Success
                } else {
                    SceneToneView::Active
                },
                Some(if node == Node::Exit {
                    format!("escaped · E {} · T {}", energy, time)
                } else {
                    format!("E {} · T {}", energy, time)
                }),
            ),
            &AccountId::Agent,
            &Asset::At(node),
        );
        entity.metrics = vec![
            asset_metric(path!("energy"), "Energy", &Asset::Energy, energy),
            asset_metric(path!("time"), "Time", &Asset::Time, time),
        ];
        entity
    });
    let key = if !world.balance(&AccountId::World, &Asset::Key).is_zero() {
        Some(link_balance::<maze::Ontology>(
            visual_entity(
                path!("maze/key/brass"),
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::GraphNode {
                    node: node_key(Node::KeyRoom),
                },
                SceneEntityRoleView::Attachment,
                SceneToneView::Warning,
                Some("available".into()),
            ),
            &AccountId::World,
            &Asset::Key,
        ))
    } else if !world.balance(&AccountId::Agent, &Asset::Key).is_zero() {
        Some(link_balance::<maze::Ontology>(
            visual_entity(
                path!("maze/key/brass"),
                "Brass key",
                SceneGlyphView::Key,
                SceneAnchorView::Entity {
                    entity: path!("agent/explorer"),
                },
                SceneEntityRoleView::Attachment,
                SceneToneView::Active,
                Some("carried".into()),
            ),
            &AccountId::Agent,
            &Asset::Key,
        ))
    } else {
        None
    };
    let enhanced = !world.balance(&AccountId::Agent, &Asset::Health).is_zero();
    let vault_power = !world
        .balance(&AccountId::World, &Asset::PowerVault)
        .is_zero();
    let unlocked = !world.balance(&AccountId::World, &Asset::Open).is_zero();
    let gate_open = unlocked && (!enhanced || vault_power);
    let gate_status = if gate_open {
        "open"
    } else if unlocked {
        "unlocked · needs power"
    } else {
        "locked"
    };
    let gate = link_balance::<maze::Ontology>(
        visual_entity(
            path!("maze/gate/state"),
            "Vault gate",
            SceneGlyphView::Door,
            SceneAnchorView::GraphNode {
                node: node_key(Node::Gate),
            },
            SceneEntityRoleView::State,
            if gate_open {
                SceneToneView::Success
            } else {
                SceneToneView::Danger
            },
            Some(gate_status.into()),
        ),
        &AccountId::World,
        if gate_open {
            &Asset::Open
        } else {
            &Asset::Locked
        },
    );
    let mut scene = Scene::graph(
        "The Vault District — find the key, open the gate, reach the exit",
        graph_nodes,
        graph_edges,
        focus.map(node_key),
    )
    .with_entities(agent.into_iter().chain(key).chain([gate]))
    .with_metrics([
        asset_metric(
            path!("energy-left"),
            "Explorer energy",
            &Asset::Energy,
            world.balance(&AccountId::Agent, &Asset::Energy),
        ),
        asset_metric(
            path!("time-left"),
            "Time remaining",
            &Asset::Time,
            world.balance(&AccountId::Agent, &Asset::Time),
        ),
        visual_metric(
            path!("key-state"),
            "Key",
            if world.balance(&AccountId::World, &Asset::Key).is_zero() {
                if world.balance(&AccountId::Agent, &Asset::Key).is_zero() {
                    "used"
                } else {
                    "carried"
                }
            } else {
                "waiting"
            },
            None,
        ),
        visual_metric(path!("gate-state"), "Vault gate", gate_status, None),
    ]);
    if enhanced {
        scene.metrics.extend([
            visual_metric(
                path!("power"),
                "Shared power",
                if vault_power {
                    "vault shutter"
                } else {
                    "canal crossing"
                },
                None,
            ),
            asset_metric(
                path!("injury"),
                "Injury delay per action",
                &Asset::Injury,
                world.balance(&AccountId::Agent, &Asset::Injury),
            ),
            asset_metric(
                path!("shield"),
                "Protection remaining",
                &Asset::Shield,
                world.balance(&AccountId::Agent, &Asset::Shield),
            ),
            asset_metric(
                path!("health"),
                "Health",
                &Asset::Health,
                world.balance(&AccountId::Agent, &Asset::Health),
            ),
        ]);
        for (key, label, at, glyph, state) in [
            (
                path!("power-library"),
                "Power switch",
                Node::Library,
                SceneGlyphView::Energy,
                if vault_power {
                    "vault powered"
                } else {
                    "canal powered"
                },
            ),
            (
                path!("power-workshop"),
                "Power switch",
                Node::Workshop,
                SceneGlyphView::Energy,
                if vault_power {
                    "vault powered"
                } else {
                    "canal powered"
                },
            ),
            (
                path!("medical"),
                "Aid station",
                Node::Garden,
                SceneGlyphView::Repair,
                "treat injuries · 2 ticks",
            ),
            (
                path!("protection"),
                "Protective equipment",
                Node::Start,
                SceneGlyphView::Shield,
                if world.balance(&AccountId::World, &Asset::Shield).is_zero() {
                    "collected"
                } else {
                    "2 charges · 1 tick"
                },
            ),
        ] {
            scene.entities.push(visual_entity(
                key,
                label,
                glyph,
                SceneAnchorView::GraphNode { node: node_key(at) },
                SceneEntityRoleView::Attachment,
                SceneToneView::Neutral,
                Some(state.into()),
            ));
        }
    }
    scene.legend = vec![
        SceneLegendView {
            label: "Room".into(),
            glyph: SceneGlyphView::Location,
            tone: SceneToneView::Neutral,
        },
        SceneLegendView {
            label: "Explorer".into(),
            glyph: SceneGlyphView::Agent,
            tone: SceneToneView::Active,
        },
        SceneLegendView {
            label: "Key".into(),
            glyph: SceneGlyphView::Key,
            tone: SceneToneView::Warning,
        },
        SceneLegendView {
            label: if gate_open {
                "Open gate"
            } else {
                "Locked gate"
            }
            .into(),
            glyph: SceneGlyphView::Door,
            tone: if gate_open {
                SceneToneView::Success
            } else {
                SceneToneView::Danger
            },
        },
        SceneLegendView {
            label: "Exit".into(),
            glyph: SceneGlyphView::Goal,
            tone: SceneToneView::Goal,
        },
    ];
    Some(scene)
}

fn route_id(rate: &RateId) -> Option<ItemPath> {
    match rate {
        RateId::Move { from, to, .. } | RateId::ProtectedMove { from, to, .. } => {
            Some(path!("edge").then(from).then(to))
        }
        _ => None,
    }
}

pub(super) fn decorate_route_evidence(
    document: &mut ViewDocument,
    trace: &axionomy::Trace<maze::Ontology>,
) {
    let planned = trace
        .exchanges()
        .iter()
        .filter_map(|exchange| route_id(exchange.rate()))
        .collect::<std::collections::BTreeSet<_>>();
    let mut traversed = std::collections::BTreeSet::new();
    decorate_scene_paths(document.initial.scene.as_mut(), &planned, &traversed, None);
    for (frame, exchange) in document.frames.iter_mut().zip(trace.exchanges()) {
        let current = route_id(exchange.rate());
        decorate_scene_paths(
            frame.after.scene.as_mut(),
            &planned,
            &traversed,
            current.as_ref(),
        );
        if let Some(current) = current {
            traversed.insert(current);
        }
    }
}

fn decorate_scene_paths(
    scene: Option<&mut Scene>,
    planned: &std::collections::BTreeSet<ItemPath>,
    traversed: &std::collections::BTreeSet<ItemPath>,
    current: Option<&ItemPath>,
) {
    let Some(scene) = scene else { return };
    scene.legend.retain(|entry| {
        entry.label != "Planned next"
            && entry.label != "Traversed route"
            && entry.label != "Current move"
    });
    let remaining = planned
        .iter()
        .any(|path| !traversed.contains(path) && current != Some(path));
    if remaining {
        scene.legend.push(SceneLegendView {
            label: "Planned next".into(),
            glyph: SceneGlyphView::Move,
            tone: SceneToneView::Warning,
        });
    }
    if !traversed.is_empty() {
        scene.legend.push(SceneLegendView {
            label: "Traversed route".into(),
            glyph: SceneGlyphView::Move,
            tone: SceneToneView::Active,
        });
    }
    if current.is_some() {
        scene.legend.push(SceneLegendView {
            label: "Current move".into(),
            glyph: SceneGlyphView::Move,
            tone: SceneToneView::Success,
        });
    }
    if let SceneSurfaceView::Graph { edges, .. } = &mut scene.surface {
        let route: [ClassPath; 3] = [path!("current"), path!("traversed"), path!("candidate")];
        let [current_class, traversed_class, candidate_class] = &route;
        for edge in edges {
            edge.classes.retain(|class| !route.contains(class));
            if current == Some(&edge.id) {
                edge.classes.push(current_class.clone());
            } else if traversed.contains(&edge.id) {
                edge.classes.push(traversed_class.clone());
            } else if planned.contains(&edge.id) {
                edge.classes.push(candidate_class.clone());
            }
        }
    }
    for path in &mut scene.paths {
        path.status = if current == Some(&path.id) {
            ScenePathStatusView::Current
        } else if traversed.contains(&path.id) {
            ScenePathStatusView::Traversed
        } else if planned.contains(&path.id) {
            ScenePathStatusView::Candidate
        } else if matches!(path.status, ScenePathStatusView::Blocked) {
            ScenePathStatusView::Blocked
        } else {
            ScenePathStatusView::Available
        };
    }
}

// How Studio names and writes this problem's values.

impl Format for maze::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy | Self::Distance(_) => {
                QuantityFormat::COUNT.unit("units")
            }
            Self::Time | Self::SpentTime | Self::Injury => QuantityFormat::COUNT.unit("ticks"),
            Self::Shield | Self::UsedShield => QuantityFormat::COUNT.unit("charges"),
            _ => QuantityFormat::COUNT,
        }
    }
}

fn maze_node(node: maze::Node) -> String {
    words(&node)
}

impl Label for maze::Node {
    fn label(&self) -> String {
        maze_node(*self)
    }
}

impl Label for maze::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Agent => "Explorer".into(),
            Self::World => "Vault District".into(),
        }
    }
}

impl Label for maze::Asset {
    fn label(&self) -> String {
        match *self {
            Self::PowerTransit => "Power routed to the canal crossing".into(),
            Self::PowerVault => "Power routed to the vault shutter".into(),
            Self::Health => "Health reserve".into(),
            Self::Injury => "Injury delay per action".into(),
            Self::Shield => "Protective equipment".into(),
            Self::UsedShield => "Spent protection".into(),
            Self::Medicine => "Available medicine".into(),
            Self::UsedMedicine => "Medicine used".into(),
            Self::At(node) => format!("Standing in {}", maze_node(node)),
            Self::Edge(from, to) => {
                format!("Passage {} → {}", maze_node(from), maze_node(to))
            }
            Self::Key => "Key".into(),
            Self::Locked => "Vault gate locked".into(),
            Self::Open => "Vault gate open".into(),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Time => "Time left".into(),
            Self::SpentTime => "Time spent".into(),
            Self::Target(node) => format!("Goal: reach {}", maze_node(node)),
            Self::Distance(node) => format!("Distance from {} to the exit", maze_node(node)),
            Self::Active => "Expedition in progress".into(),
            Self::Solved => "Explorer escaped".into(),
        }
    }
}

impl Label for maze::RateId {
    fn label(&self) -> String {
        match *self {
            Self::DivertPower { at, to_vault } => format!(
                "Divert power at {} to {}",
                maze_node(at),
                if to_vault { "vault" } else { "canal" }
            ),
            Self::TakeProtection { at } => {
                format!("Collect protective equipment at {}", maze_node(at))
            }
            Self::Treat { at } => format!("Treat injuries at {}", maze_node(at)),
            Self::ProtectedMove { from, to, energy } => format!(
                "Protected crossing {} → {} for {energy} energy",
                maze_node(from),
                maze_node(to)
            ),
            Self::Move {
                from,
                to,
                energy,
                needs_open_door,
            } => format!(
                "Walk {} → {} for {} energy{}",
                maze_node(from),
                maze_node(to),
                energy,
                if needs_open_door {
                    " once the vault gate is open"
                } else {
                    ""
                }
            ),
            Self::TakeKey { at } => format!("Pick up the key in {}", maze_node(at)),
            Self::UnlockDoor { at } => format!("Unlock the gate from {}", maze_node(at)),
            Self::Finish => "Reach the exit".into(),
        }
    }
}

debug_labels!(maze::Role);
