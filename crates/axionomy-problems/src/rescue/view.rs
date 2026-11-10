//! How the uncertain rescue is presented: its labels, units, scene and leaderboards.

use crate::rescue::{self, AccountId, Asset, Location, World};
use axionomy::path;
use axionomy_view::{
    Format, GraphEdgeView, GraphNodeView, Keyed, Label, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneGlyphView, SceneToneView, ViewId, asset_metric, link_account,
    link_balance, visual_entity,
};

pub(super) fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        (Location::Base, 300.0, 250.0),
        (Location::North, 120.0, 60.0),
        (Location::South, 480.0, 60.0),
        (Location::East, 570.0, 170.0),
        (Location::West, 30.0, 170.0),
        (Location::Harbor, 120.0, 360.0),
        (Location::Hills, 480.0, 360.0),
    ];
    let focus = locations
        .into_iter()
        .find(|(location, _, _)| {
            !world
                .balance(&AccountId::Agent, &Asset::At(*location))
                .is_zero()
        })
        .map(|(location, _, _)| location);
    let nodes = locations
        .into_iter()
        .map(|(location, x, y)| GraphNodeView {
            id: ViewId::new(path!("location").then(&location), format!("{location:?}")),
            classes: if focus == Some(location) {
                vec![path!("current")]
            } else {
                Vec::new()
            },
            x: Some(x),
            y: Some(y),
        })
        .collect();
    let edges = [
        Location::North,
        Location::South,
        Location::East,
        Location::West,
        Location::Harbor,
        Location::Hills,
    ]
    .into_iter()
    .map(|location| GraphEdgeView {
        id: path!("route").then(&location),
        source: path!("location").then(&Location::Base),
        target: path!("location").then(&location),
        label: Some("hidden victim prior".into()),
        classes: if focus == Some(location) {
            vec![path!("current"), path!("uncertain")]
        } else {
            vec![path!("uncertain")]
        },
    })
    .collect();
    let agent_location = focus.expect("the responder remains at an encoded location");
    let mut agent = link_balance::<rescue::Ontology>(
        visual_entity(
            path!("agent/responder"),
            "Responder",
            SceneGlyphView::Agent,
            SceneAnchorView::GraphNode {
                node: path!("location").then(&agent_location),
            },
            SceneEntityRoleView::Occupant,
            SceneToneView::Active,
            Some(
                if world
                    .balance(&AccountId::Agent, &Asset::AwaitingObservation)
                    .is_zero()
                {
                    "acting".into()
                } else {
                    "awaiting observation".into()
                },
            ),
        ),
        &AccountId::Agent,
        &Asset::At(agent_location),
    );
    agent.metrics = vec![asset_metric(
        path!("energy"),
        "Energy",
        &Asset::Energy,
        world.balance(&AccountId::Agent, &Asset::Energy),
    )];
    let belief_entities = [
        Location::North,
        Location::South,
        Location::East,
        Location::West,
        Location::Harbor,
        Location::Hills,
    ]
    .into_iter()
    .filter(|location| {
        !world
            .balance(&AccountId::Agent, &Asset::Belief(*location))
            .is_zero()
    })
    .map(|location| {
        link_balance::<rescue::Ontology>(
            visual_entity(
                path!("belief").then(&location),
                format!("Belief: {location:?}"),
                SceneGlyphView::Information,
                SceneAnchorView::GraphNode {
                    node: path!("location").then(&location),
                },
                SceneEntityRoleView::State,
                SceneToneView::Uncertain,
                Some("actor belief".into()),
            ),
            &AccountId::Agent,
            &Asset::Belief(location),
        )
    });
    let nature = link_account::<rescue::Ontology>(
        visual_entity(
            path!("system/nature"),
            "Hidden victim state",
            SceneGlyphView::Information,
            SceneAnchorView::GraphNode {
                node: path!("location").then(&Location::Base),
            },
            SceneEntityRoleView::Context,
            SceneToneView::Uncertain,
            Some("not visible to actor".into()),
        ),
        &AccountId::Nature,
    );
    Some(
        Scene::graph(
            "What the agent can see (the truth is hidden)",
            nodes,
            edges,
            focus.map(|location| path!("location").then(&location)),
        )
        .with_entities(
            std::iter::once(agent)
                .chain(belief_entities)
                .chain(std::iter::once(nature)),
        )
        .with_metrics([
            asset_metric(
                path!("energy"),
                "Energy remaining",
                &Asset::Energy,
                world.balance(&AccountId::Agent, &Asset::Energy),
            ),
            asset_metric(
                path!("sensor"),
                "Sensor charges",
                &Asset::Sensor,
                world.balance(&AccountId::Agent, &Asset::Sensor),
            ),
        ]),
    )
}

// How Studio names and writes this problem's values.

impl Format for rescue::Asset {
    fn format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy => QuantityFormat::COUNT.unit("units"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl Label for rescue::AccountId {
    fn label(&self) -> String {
        match self {
            Self::Agent => "Responder".into(),
            Self::Nature => "Hidden scenario".into(),
        }
    }
}

impl Label for rescue::Asset {
    fn label(&self) -> String {
        match self {
            Self::At(location) => format!("Responder at {location:?}"),
            Self::Energy => "Energy left".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::Sensor => "Sensor available".into(),
            Self::UsedSensor => "Sensor used".into(),
            Self::Unresolved => "Location not yet drawn".into(),
            Self::ScenarioWeight(location, seed) => {
                format!("Weight for {location:?}, sensor state {seed}")
            }
            Self::Truth(location) => format!("Survivor is at {location:?}"),
            Self::Seed(seed) => format!("Sensor state {seed}"),
            Self::Belief(location) => format!("Reading points to {location:?}"),
            Self::Planning => "Choosing an action".into(),
            Self::AwaitingObservation => "Waiting for a sensor reading".into(),
            Self::Committed => "Destination chosen".into(),
            Self::Rescued => "Survivor reached".into(),
            Self::Evacuated => "Survivor evacuated".into(),
            Self::Solved => "Rescue complete".into(),
        }
    }
}

impl Label for rescue::RateId {
    fn label(&self) -> String {
        match self {
            Self::Instantiate { truth, seed } => {
                format!("Draw scenario: {truth:?}, sensor state {seed}")
            }
            Self::BeginObserve => "Use the sensor".into(),
            Self::ResolveObservation {
                truth,
                seed,
                report,
                ..
            } => format!(
                "Receive sensor reading: {report:?} (survivor at {truth:?}, sensor state {seed})"
            ),
            Self::Move { from, to } => format!("Move {from:?} → {to:?}"),
            Self::Rescue { location } => format!("Reach survivor at {location:?}"),
            Self::Evacuate { location } => format!("Evacuate from {location:?} to Base"),
            Self::Finish => "Complete the rescue".into(),
        }
    }
}

impl Label for rescue::Role {
    fn label(&self) -> String {
        match self {
            Self::Actor => "Responder",
            Self::Nature => "Nature",
        }
        .into()
    }
}
