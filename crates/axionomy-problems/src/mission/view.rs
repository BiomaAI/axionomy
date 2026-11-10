//! How the hidden-information mission is presented: its labels, units, scene and leaderboards.

use crate::mission::{self, AccountId, AgentId, Asset, Location, World};
use axionomy::path;
use axionomy_view::{
    Format, GraphEdgeView, GraphNodeView, Keyed, Label, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, ViewId, asset_metric, link_account,
    link_balance, visual_entity, visual_metric,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        (Location::Base, 310.0, 250.0),
        (Location::North, 110.0, 60.0),
        (Location::South, 510.0, 60.0),
    ];
    let nodes = locations
        .into_iter()
        .map(|(location, x, y)| {
            let occupied = [AgentId::Scout, AgentId::Medic].into_iter().any(|agent| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(location))
                    .is_zero()
            });
            GraphNodeView {
                id: ViewId::new(path!("location").then(&location), format!("{location:?}")),
                classes: if occupied {
                    vec![path!("current")]
                } else {
                    Vec::new()
                },
                x: Some(x),
                y: Some(y),
            }
        })
        .collect();
    let edges = [Location::North, Location::South]
        .into_iter()
        .map(|location| GraphEdgeView {
            id: path!("mission").then(&location),
            source: path!("location").then(&Location::Base),
            target: path!("location").then(&location),
            label: Some("hidden truth + hazard".into()),
            classes: if [AgentId::Scout, AgentId::Medic].into_iter().any(|agent| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(location))
                    .is_zero()
            }) {
                vec![path!("current"), path!("uncertain")]
            } else {
                vec![path!("uncertain")]
            },
        })
        .collect();
    let agent_entities = [AgentId::Scout, AgentId::Medic].into_iter().map(|agent| {
        let location = [Location::Base, Location::North, Location::South]
            .into_iter()
            .find(|location| {
                !world
                    .balance(&AccountId::Agent(agent), &Asset::At(*location))
                    .is_zero()
            })
            .expect("mission agents remain at an encoded location");
        let known = [Location::North, Location::South]
            .into_iter()
            .find_map(|location| {
                if !world
                    .balance(&AccountId::Agent(agent), &Asset::SharedIntel(location))
                    .is_zero()
                {
                    Some(format!("shared {location:?}"))
                } else if !world
                    .balance(&AccountId::Agent(agent), &Asset::Intel(location))
                    .is_zero()
                {
                    Some(format!(
                        "{} {location:?}",
                        if agent == AgentId::Scout {
                            "saw"
                        } else {
                            "briefed"
                        }
                    ))
                } else {
                    None
                }
            });
        let mut entity = link_balance::<mission::Ontology>(
            visual_entity(
                path!("agent").then(&agent),
                format!("{agent:?}"),
                if agent == AgentId::Scout {
                    SceneGlyphView::Sensor
                } else {
                    SceneGlyphView::Agent
                },
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&location),
                },
                SceneEntityRoleView::Occupant,
                SceneToneView::Active,
                Some(
                    if world
                        .balance(&AccountId::Agent(agent), &Asset::Injured)
                        .is_zero()
                    {
                        known.unwrap_or_else(|| "ready".into())
                    } else {
                        "injured".into()
                    },
                ),
            ),
            &AccountId::Agent(agent),
            &Asset::At(location),
        );
        entity.metrics = vec![asset_metric(
            path!("energy").then(&agent),
            "Energy",
            &Asset::Energy,
            world.balance(&AccountId::Agent(agent), &Asset::Energy),
        )];
        entity
    });
    let system_entities = [
        link_account::<mission::Ontology>(
            visual_entity(
                path!("system/mission"),
                "Mission control",
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&Location::Base),
                },
                SceneEntityRoleView::State,
                SceneToneView::Neutral,
                Some("shared state".into()),
            ),
            &AccountId::Mission,
        ),
        link_account::<mission::Ontology>(
            visual_entity(
                path!("system/nature"),
                "Hidden scenario",
                SceneGlyphView::Weather,
                SceneAnchorView::Unanchored,
                SceneEntityRoleView::Context,
                SceneToneView::Uncertain,
                Some("private truth".into()),
            ),
            &AccountId::Nature,
        ),
        link_account::<mission::Ontology>(
            visual_entity(
                path!("system/success"),
                "Mission outcome",
                SceneGlyphView::Goal,
                SceneAnchorView::Unanchored,
                SceneEntityRoleView::Context,
                SceneToneView::Goal,
                Some(
                    if world.matches(&mission::goal()) {
                        "complete"
                    } else {
                        "rescue objective"
                    }
                    .into(),
                ),
            ),
            &AccountId::Success,
        ),
    ];
    Some(
        Scene::graph("Where each agent is, and what it knows", nodes, edges, None)
            .with_entities(agent_entities.chain(system_entities))
            .with_metrics([
                asset_metric(
                    path!("time"),
                    "Mission time remaining",
                    &Asset::TimeRemaining,
                    world.balance(&AccountId::Mission, &Asset::TimeRemaining),
                ),
                visual_metric(
                    path!("shared-intel"),
                    "Shared intel",
                    [Location::North, Location::South]
                        .into_iter()
                        .filter(|location| {
                            !world
                                .balance(
                                    &AccountId::Agent(AgentId::Scout),
                                    &Asset::SharedIntel(*location),
                                )
                                .is_zero()
                        })
                        .count(),
                    Some("reports"),
                ),
            ]),
    )
}

// How Studio names and writes this problem's values.

impl Label for mission::Hazard {
    fn label(&self) -> String {
        match self {
            Self::Safe => "no hazard",
            Self::Injury => "injury hazard",
        }
        .into()
    }
}

impl Format for mission::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy => QuantityFormat::COUNT.unit("units"),
            Self::TimeRemaining | Self::ElapsedTime => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for mission::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("{agent:?}"),
            Self::Nature => "Hidden mission scenario".into(),
            Self::Mission => "Mission clock and state".into(),
            Self::Success => "Mission objective".into(),
        }
    }
}

impl Label for mission::Asset {
    fn label(&self) -> String {
        match self {
            Self::AgentIdentity(agent) => format!("Identity: {agent:?}"),
            Self::NatureIdentity => "Nature identity".into(),
            Self::MissionIdentity => "Mission identity".into(),
            Self::SuccessIdentity => "Objective identity".into(),
            Self::At(location) => format!("At {location:?}"),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Sensor => "Sensor available".into(),
            Self::UsedSensor => "Sensor used".into(),
            Self::MedicalKit => "Medical kit available".into(),
            Self::UsedMedicalKit => "Medical kit used".into(),
            Self::Intel(location) => format!("Private sighting: {location:?}"),
            Self::SharedIntel(location) => format!("Shared sighting: {location:?}"),
            Self::Injured => "Agent injured".into(),
            Self::Unresolved => "Scenario not yet drawn".into(),
            Self::ScenarioWeight(location, seed, danger) => format!(
                "Odds: objective at {location:?}, sensor state {seed}, {}",
                danger.label()
            ),
            Self::Truth(location) => format!("Objective at {location:?}"),
            Self::Seed(seed) => format!("Sensor state {seed}"),
            Self::Hazard(danger) => format!("Drawn: {}", danger.label()),
            Self::HazardResolved => "Hazard resolved".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::Planning => "Choosing an action".into(),
            Self::AwaitingScan => "Waiting for a sighting".into(),
            Self::AwaitingEncounter => "Waiting for a hazard outcome".into(),
            Self::NeedsTreatment => "Treatment required".into(),
            Self::AreaSafe => "Area made safe".into(),
            Self::Rescued => "Objective rescued".into(),
            Self::Solved => "Mission complete".into(),
        }
    }
}

impl Label for mission::RateId {
    fn label(&self) -> String {
        match self {
            Self::Instantiate {
                truth,
                seed,
                hazard: danger,
            } => format!(
                "Draw scenario: objective at {truth:?}, sensor state {seed}, {}",
                danger.label()
            ),
            Self::BeginScan => "Scout scans the area".into(),
            Self::ResolveScan {
                truth,
                seed,
                report,
                ..
            } => format!("Scout sees {report:?} (objective at {truth:?}, sensor state {seed})"),
            Self::Share(location) => format!("Scout shares sighting: {location:?}"),
            Self::MoveTogether(location) => format!("Scout and Medic move to {location:?}"),
            Self::MoveDirect(location) => format!("Move directly to {location:?}"),
            Self::Encounter { location, hazard } => match hazard {
                mission::Hazard::Safe => format!("{location:?} turns out safe"),
                mission::Hazard::Injury => format!("Hazard at {location:?} injures the Scout"),
            },
            Self::Treat => "Medic treats the injury".into(),
            Self::Rescue(location) => format!("Rescue objective at {location:?}"),
            Self::Finish => "Complete the mission".into(),
        }
    }
}

impl Label for mission::Role {
    fn label(&self) -> String {
        match self {
            Self::Scout => "Scout",
            Self::Medic => "Medic",
            Self::Nature => "Nature",
            Self::Mission => "Mission",
            Self::Goal => "Goal",
        }
        .into()
    }
}
