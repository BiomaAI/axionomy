use super::*;
use axionomy_problems::rescue::{self, AccountId, Asset, Location, Policy, World};
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, FrontierCompletenessView, GraphEdgeView,
    GraphNodeView, ObjectiveAxisView, ObjectiveDirectionView, ObservationView, ParetoFrontView,
    ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let profile = instance_profile(request, descriptor);
    let model = match profile {
        InstanceProfile::Micro => rescue::uniform_uncertain(),
        InstanceProfile::Showcase => rescue::uniform_uncertain_showcase(),
        InstanceProfile::Stress => rescue::uniform_uncertain_stress(),
    };
    let samples = if matches!(profile, InstanceProfile::Stress) {
        request.budget.max(256)
    } else {
        request.budget.max(2)
    } as usize;
    let sample = rescue::instantiate(&model, Location::South, 1)
        .ok_or_else(|| problem_error("rescue", "scenario could not be instantiated"))?;
    let front = rescue::policy_front(&model, samples, request.seed)
        .ok_or_else(|| problem_error("rescue", "policy front failed"))?;
    let comparison = rescue::monte_carlo(&model, samples, request.seed)
        .ok_or_else(|| problem_error("rescue", "Monte Carlo failed"))?;
    let policies = [
        ("observe_then_follow", Policy::ObserveThenFollow),
        ("direct_north", Policy::NorthWithoutObserving),
    ];
    let mut documents = Vec::new();
    for (strategy, policy) in policies {
        let rollout = rescue::run_sampled_policy(&model, &sample, policy)
            .ok_or_else(|| problem_error("rescue", "sampled rollout failed"))?;
        let mut view = document(
            DocumentSpec { problem: "rescue", strategy, title: if policy == Policy::ObserveThenFollow { "Rescue · sense, then move" } else { "Rescue · go north immediately" }, description: if policy == Policy::ObserveThenFollow { "The agent spends the sensor, receives a reading, and follows it." } else { "The agent commits north without looking. In this scenario the survivor is south, and the replay keeps the failure." }, source_label: "Uncertain rescue" },
            &model, &rescue::goal(), rollout.trace(), vec![
                ObjectiveView { key: "success".into(), label: "Succeeded".into(), direction: ObjectiveDirectionView::Maximize, value: u8::from(rollout.succeeded()).to_string() },
                ObjectiveView { key: "sensor".into(), label: "Sensor used".into(), direction: ObjectiveDirectionView::Minimize, value: u8::from(rollout.used_sensor()).to_string() },
                ObjectiveView { key: "energy".into(), label: "Energy spent".into(), direction: ObjectiveDirectionView::Minimize, value: rollout.spent_energy().to_string() },
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
            view.proposals.push(proposal("rescue", ProposalSpec { id: "unresolved-action", label: "Act before Nature resolves", description: "Acting before Nature has decided what is true shows exactly which facts are still missing." }, &model, candidate));
        }
        documents.push(view);
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn observation(world: &World) -> ObservationView {
    let key = rescue::agent_view(world).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(
                format!("rescue:account:{account:?}"),
                account.studio_label(),
            ),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(format!("rescue:asset:{asset:?}"), asset.studio_label()),
                    quantity: ExactQuantity(quantity.to_string()),
                })
                .collect(),
        })
        .collect();
    ObservationView {
        actor: ViewId::new("rescue:actor:agent", "Rescue agent"),
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
                key: "success".into(),
                label: "Success probability".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: "sensor".into(),
                label: "Sensor use".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "energy".into(),
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
                    label: format!("{policy:?}"),
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
            id: ViewId::new(format!("location:{location:?}"), format!("{location:?}")),
            classes: if focus == Some(location) {
                vec!["current".into()]
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
        id: format!("route:{location:?}"),
        source: "location:Base".into(),
        target: format!("location:{location:?}"),
        label: Some("hidden victim prior".into()),
        classes: vec!["uncertain".into()],
    })
    .collect();
    let mut agent = visual_entity(
        "agent:responder",
        "Responder",
        SceneGlyphView::Agent,
        focus.map_or(SceneAnchorView::Unanchored, |location| {
            SceneAnchorView::GraphNode {
                node: format!("location:{location:?}"),
            }
        }),
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
    );
    agent.account = Some("rescue:account:agent".into());
    agent.metrics = vec![visual_metric(
        "energy",
        "Energy",
        world.balance(&AccountId::Agent, &Asset::Energy),
        Some("units"),
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
        visual_entity(
            format!("belief:{location:?}"),
            format!("Belief: {location:?}"),
            SceneGlyphView::Information,
            SceneAnchorView::GraphNode {
                node: format!("location:{location:?}"),
            },
            SceneToneView::Uncertain,
            Some("actor belief".into()),
        )
    });
    Some(
        Scene::graph(
            "What the agent can see (the truth is hidden)",
            nodes,
            edges,
            focus.map(|location| format!("location:{location:?}")),
        )
        .with_entities(std::iter::once(agent).chain(belief_entities))
        .with_metrics([
            visual_metric(
                "energy",
                "Energy remaining",
                world.balance(&AccountId::Agent, &Asset::Energy),
                Some("units"),
            ),
            visual_metric(
                "sensor",
                "Sensor charges",
                world.balance(&AccountId::Agent, &Asset::Sensor),
                Some("uses"),
            ),
        ]),
    )
}
