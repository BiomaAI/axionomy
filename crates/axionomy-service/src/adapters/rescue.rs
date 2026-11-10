use super::*;
use axionomy_search::rollout::Unrecorded;
mod incident;
use axionomy_problems::rescue::{self, AccountId, Asset, Location, Policy, World};
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, FrontierCompletenessView, GraphEdgeView,
    GraphNodeView, ObjectiveAxisView, ObjectiveDirectionView, ObservationView, ParetoFrontView,
    ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    if !matches!(profile, InstanceProfile::Micro) {
        return incident::build(study, descriptor, profile, progress);
    }
    let model = match profile {
        InstanceProfile::Micro => rescue::uniform_uncertain(),
        InstanceProfile::Showcase => rescue::uniform_uncertain_showcase(),
        InstanceProfile::Stress => rescue::uniform_uncertain_stress(),
    };
    let samples = if matches!(profile, InstanceProfile::Stress) {
        study.budget.max(256)
    } else {
        study.budget.max(2)
    } as usize;
    let sample = rescue::instantiate(&model, Location::South, 1)
        .ok_or_else(|| problem_error("rescue", "scenario could not be instantiated"))?;
    let front = rescue::policy_front(&model, samples, study.seed, &mut Unrecorded)
        .ok_or_else(|| problem_error("rescue", "policy front failed"))?;
    let comparison = rescue::monte_carlo(&model, samples, study.seed, &mut Unrecorded)
        .ok_or_else(|| problem_error("rescue", "Monte Carlo failed"))?;
    let policies = [
        ("observe-then-follow", Policy::ObserveThenFollow),
        ("direct-north", Policy::NorthWithoutObserving),
    ];
    let mut documents = Vec::new();
    for (strategy, policy) in policies {
        let rollout = rescue::run_sampled_policy(&mut model.clone(), &sample, policy)
            .ok_or_else(|| problem_error("rescue", "sampled rollout failed"))?;
        let mut view = document(
            progress,
            DocumentSpec { problem: "rescue", strategy, title: if policy == Policy::ObserveThenFollow { "Rescue · sense, then move" } else { "Rescue · go north immediately" }, description: if policy == Policy::ObserveThenFollow { "The agent spends the sensor, receives a reading, and follows it." } else { "The agent commits north without looking. In this scenario the survivor is south, and the replay keeps the failure." }, source_label: "Uncertain rescue" },
            &model, &rescue::goal(), rollout.trace(), vec![
                ObjectiveView { key: path!("success"), label: "Succeeded".into(), direction: ObjectiveDirectionView::Maximize, value: u8::from(rollout.succeeded()).to_string() },
                ObjectiveView { key: path!("sensor"), label: "Sensor used".into(), direction: ObjectiveDirectionView::Minimize, value: u8::from(rollout.used_sensor()).to_string() },
                ObjectiveView { key: path!("energy"), label: "Energy spent".into(), direction: ObjectiveDirectionView::Minimize, value: rollout.spent_energy().to_string() },
            ], scene,
        ).map_err(|error| problem_error("rescue", error))?;
        view.pareto_fronts.push(policy_front(&front, policy));
        view.telemetry.push(telemetry(
            "Monte Carlo policy evaluation",
            false,
            [
                (
                    TelemetryKindView::Sample,
                    comparison.samples() as u64,
                    "scenarios sampled".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    rollout.trace().exchanges().len() as u64,
                    "rollout exchanges".into(),
                ),
            ],
        ));
        view.observations.push(observation(&model));
        if let Some(candidate) = rescue::candidates(&model).first() {
            view.proposals.push(proposal(ProposalSpec { id: "unresolved-action", label: "Act before Nature resolves", description: "Acting before Nature has decided what is true shows exactly which facts are still missing." }, &model, candidate));
        }
        documents.push(view);
    }
    assemble(study, descriptor, documents)
}

fn observation(world: &World) -> ObservationView {
    let key = rescue::agent_view(world).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(
                account_key::<rescue::Ontology>(account),
                account.studio_label(),
            ),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(asset_key::<rescue::Ontology>(asset), asset.studio_label()),
                    quantity: ExactQuantity(quantity.to_string()),
                })
                .collect(),
        })
        .collect();
    ObservationView {
        actor: ViewId::new(path!("rescue/actor/agent"), "Rescue agent"),
        label: "Agent-visible state; Nature truth and seed are omitted".into(),
        visible_accounts: accounts,
        facts: Vec::new(),
    }
}

fn policy_front(front: &rescue::PolicyFront, selected: Policy) -> ParetoFrontView {
    ParetoFrontView {
        title: "Success vs. sensor use vs. energy — sampled, not exact".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: vec![
            ObjectiveAxisView {
                key: path!("success"),
                label: "Success probability".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: path!("sensor"),
                label: "Sensor use".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: path!("energy"),
                label: "Mean energy".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        points: front
            .entries()
            .iter()
            .map(|entry| {
                let policy = *entry.payload().policy();
                let values = entry
                    .objectives()
                    .objectives()
                    .iter()
                    .map(|o| format!("{:.3}", o.value()))
                    .collect::<Vec<_>>();
                ParetoPointView {
                    label: match policy {
                        Policy::ObserveThenFollow => "Sense, then move",
                        Policy::NorthWithoutObserving => "Go north immediately",
                    }
                    .into(),
                    values,
                    selected: policy == selected,
                }
            })
            .collect(),
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
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

impl StudioFormat for rescue::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Energy | Self::SpentEnergy => QuantityFormat::COUNT.unit("units"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for rescue::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent => "Responder".into(),
            Self::Nature => "Hidden scenario".into(),
        }
    }
}

impl StudioLabel for rescue::Asset {
    fn studio_label(&self) -> String {
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

impl StudioLabel for rescue::RateId {
    fn studio_label(&self) -> String {
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

impl StudioLabel for rescue::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::Actor => "Responder",
            Self::Nature => "Nature",
        }
        .into()
    }
}
