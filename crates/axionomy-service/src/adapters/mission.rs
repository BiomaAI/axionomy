use super::*;
use axionomy::Exchange;
use axionomy_problems::mission::{self, AccountId, AgentId, Asset, Location, Policy, World};
use axionomy_search::mcts::MctsConfig;
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, FrontierCompletenessView, GraphEdgeView,
    GraphNodeView, ObjectiveAxisView, ObjectiveDirectionView, ObservationView, ParetoFrontView,
    ParetoPointView, TelemetryKindView, ViewId,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let model = mission::initial();
    let sample_index = (request.seed as usize) % 16;
    let actual = mission::instantiate(&model, sample_index)
        .ok_or_else(|| problem_error("mission", "scenario could not be instantiated"))?;
    let samples = request.budget.max(2) as usize;
    let comparison = mission::monte_carlo(&model, samples, request.seed)
        .ok_or_else(|| problem_error("mission", "Monte Carlo failed"))?;
    let front = mission::policy_front(&model, samples, request.seed)
        .ok_or_else(|| problem_error("mission", "policy front failed"))?;
    let beliefs = mission::initial_beliefs(&model);
    let decision = mission::plan(
        &actual,
        &beliefs,
        MctsConfig::new(samples, 12).with_seed(request.seed),
    )
    .map_err(|error| problem_error("mission", format!("{error:?}")))?;
    let policies = [
        ("coordinated", Policy::ShareAndCoordinate),
        ("direct_north", Policy::NorthTogether),
    ];
    let mut documents = Vec::new();
    for (strategy, policy) in policies {
        let rollout = mission::run_policy(&model, policy, sample_index);
        let mut view = document(DocumentSpec { problem: "mission", strategy, title: if policy == Policy::ShareAndCoordinate { "Mission · share and coordinate" } else { "Mission · direct north" }, description: if policy == Policy::ShareAndCoordinate { "Scout observation, belief filtering, information sharing, coordinated movement, hazards, and treatment remain economic transitions." } else { "The direct policy commits both agents without information; the replay retains failures and resource consequences." }, source_label: "Hidden-information mission" }, &model, &mission::goal(), rollout.trace(), vec![
            ObjectiveView { key: "success".into(), label: "Succeeded".into(), direction: ObjectiveDirectionView::Maximize, value: u8::from(rollout.succeeded()).to_string() },
            ObjectiveView { key: "time".into(), label: "Elapsed time".into(), direction: ObjectiveDirectionView::Minimize, value: rollout.elapsed_time().to_string() },
            ObjectiveView { key: "medical".into(), label: "Medical kit used".into(), direction: ObjectiveDirectionView::Minimize, value: u8::from(rollout.used_medical_kit()).to_string() },
        ], scene).map_err(|error| problem_error("mission", error))?;
        view.pareto_fronts.push(policy_front(&front, policy));
        view.telemetry.push(telemetry(
            "information-set MCTS + Monte Carlo",
            false,
            [
                (
                    TelemetryKindView::Iteration,
                    decision.iterations() as u64,
                    "ISMCTS iterations".into(),
                ),
                (
                    TelemetryKindView::InformationSet,
                    decision.information_sets() as u64,
                    "information sets".into(),
                ),
                (
                    TelemetryKindView::Sample,
                    comparison.samples() as u64,
                    "policy rollouts".into(),
                ),
                (
                    TelemetryKindView::Generated,
                    rollout.trace().exchanges().len() as u64,
                    "selected rollout exchanges".into(),
                ),
            ],
        ));
        view.observations = vec![
            observation(&actual, AgentId::Scout),
            observation(&actual, AgentId::Medic),
        ];
        if let Some(candidate) = mission::candidates(&model).first() {
            let malformed = Exchange::new(*candidate.rate(), candidate.units().clone());
            view.proposals.push(proposal("mission", ProposalSpec { id: "action-without-actors", label: "Mission action without roles", description: "Actor, Nature, mission, and goal roles are explicit; omitting them yields a structured invalid assessment." }, &model, &malformed));
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

fn observation(world: &World, agent: AgentId) -> ObservationView {
    let key = mission::agent_view(world, agent).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(
                format!("mission:account:{account:?}"),
                format!("{account:?}"),
            ),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(format!("mission:asset:{asset:?}"), format!("{asset:?}")),
                    quantity: ExactQuantity(quantity.to_string()),
                })
                .collect(),
        })
        .collect();
    ObservationView {
        actor: ViewId::new(format!("mission:actor:{agent:?}"), format!("{agent:?}")),
        label: format!("{agent:?}-visible economic state; Nature remains hidden"),
        visible_accounts: accounts,
        facts: Vec::new(),
    }
}

fn policy_front(front: &mission::PolicyFront, selected: Policy) -> ParetoFrontView {
    ParetoFrontView {
        title: "Sampled reliability / time / medical-use frontier".into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: vec![
            ObjectiveAxisView {
                key: "success".into(),
                label: "Success probability".into(),
                direction: ObjectiveDirectionView::Maximize,
            },
            ObjectiveAxisView {
                key: "time".into(),
                label: "Mean time".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: "medical".into(),
                label: "Medical-kit use".into(),
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
                    .collect();
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
        (Location::Base, 310.0, 250.0),
        (Location::North, 110.0, 60.0),
        (Location::South, 510.0, 60.0),
    ];
    let nodes = locations
        .into_iter()
        .map(|(location, x, y)| {
            let agents = [AgentId::Scout, AgentId::Medic]
                .into_iter()
                .filter(|agent| {
                    !world
                        .balance(&AccountId::Agent(*agent), &Asset::At(location))
                        .is_zero()
                })
                .map(|agent| format!("{agent:?}"))
                .collect::<Vec<_>>();
            GraphNodeView {
                id: ViewId::new(
                    format!("location:{location:?}"),
                    if agents.is_empty() {
                        format!("{location:?}")
                    } else {
                        format!("{location:?} · {}", agents.join(" + "))
                    },
                ),
                classes: if agents.is_empty() {
                    Vec::new()
                } else {
                    vec!["current".into()]
                },
                x: Some(x),
                y: Some(y),
            }
        })
        .collect();
    let edges = [Location::North, Location::South]
        .into_iter()
        .map(|location| GraphEdgeView {
            id: format!("mission:{location:?}"),
            source: "location:Base".into(),
            target: format!("location:{location:?}"),
            label: Some("hidden truth + hazard".into()),
            classes: vec!["uncertain".into()],
        })
        .collect();
    Some(Scene::Graph {
        title: "Actor positions, hidden Nature, and information flow".into(),
        nodes,
        edges,
        focus: None,
    })
}
