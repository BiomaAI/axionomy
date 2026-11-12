//! The uncertain rescue as a study: its catalog entry, strategies and documents.

use super::view::scene;
use crate::{
    rescue::{self, Location, Policy, World},
    support::{Capability, Family, ProblemCopy, Size, StrategyCopy, problem, size},
};
use axionomy::path;
use axionomy_search::rollout::Unrecorded;
use axionomy_study::{
    DocumentSpec, Problem, ProblemDescriptor, ProposalSpec, StudyError, StudyKey, StudyProgress,
    document, problem_error, proposal, telemetry,
};
use axionomy_view::{
    AccountView, AssetQuantityView, ExactQuantity, Label, ObjectiveAxisView,
    ObjectiveDirectionView, ObjectiveView, ObservationView, ParetoFrontView, TelemetryKindView,
    ViewDocument, ViewId, account_key, asset_key, sampled_policy_front,
};

/// Uncertain rescue: its catalog entry and how its studies are computed.
pub struct Rescue {
    descriptor: ProblemDescriptor,
}

impl Default for Rescue {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("rescue"),
                    title: "Uncertain rescue",
                    summary: "Search for several hidden casualties while their condition worsens. Trade treatment time, limited medicine, noisy hints and transport fuel against one evacuation seat.",
                    instances: [
                        "Two possible sites and eight hidden location/sensor outcomes",
                        "Three casualties across four sites, continuing deterioration, two sensor charges, stabilization and finite resupply",
                        "Four casualties across five sites, tighter fuel and crew energy, one evacuation seat and varied deadlines",
                    ],
                },
                Family::PartialObservation,
                path!("observe-then-follow"),
                &[
                    StrategyCopy {
                        key: path!("observe-then-follow"),
                        label: "Observe and triage",
                        description: "Micro evaluates the single search gamble; larger incidents repeatedly scan, search, triage, stabilize and evacuate from public observations.",
                        algorithm: "Responsive triage; Micro Monte Carlo",
                    },
                    StrategyCopy {
                        key: path!("direct-north"),
                        label: "Rapid sector sweep",
                        description: "Start north without sensing, then revisit search and evacuation decisions as the incident evolves.",
                        algorithm: "Responsive rapid sweep; Micro Monte Carlo",
                    },
                ],
                &[
                    Capability::PartialObservation,
                    Capability::Chance,
                    Capability::MonteCarlo,
                    Capability::ApproximatePareto,
                ],
            ),
        }
    }
}

impl Problem for Rescue {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, StudyError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, StudyError> {
    let profile = size(study)?;
    if !matches!(profile, Size::Micro) {
        return super::incident::study::build(study, profile, progress);
    }
    let model = match profile {
        Size::Micro => rescue::uniform_uncertain(),
        Size::Showcase => rescue::uniform_uncertain_showcase(),
        Size::Stress => rescue::uniform_uncertain_stress(),
    };
    let samples = if matches!(profile, Size::Stress) {
        study.budget.max(256)
    } else {
        study.budget.max(2)
    } as usize;
    let sample = rescue::instantiate(&model, Location::South, 1)
        .ok_or_else(|| problem_error(&study.problem, "scenario could not be instantiated"))?;
    let front = rescue::policy_front(&model, samples, study.seed, &mut Unrecorded)
        .ok_or_else(|| problem_error(&study.problem, "policy front failed"))?;
    let comparison = rescue::monte_carlo(&model, samples, study.seed, &mut Unrecorded)
        .ok_or_else(|| problem_error(&study.problem, "Monte Carlo failed"))?;
    let policies = [
        (path!("observe-then-follow"), Policy::ObserveThenFollow),
        (path!("direct-north"), Policy::NorthWithoutObserving),
    ];
    let mut documents = Vec::new();
    for (strategy, policy) in policies {
        let rollout = rescue::run_sampled_policy(&mut model.clone(), &sample, policy)
            .ok_or_else(|| problem_error(&study.problem, "sampled rollout failed"))?;
        let mut view = document(
            progress,
            DocumentSpec { problem: &study.problem, document: strategy, title: if policy == Policy::ObserveThenFollow { "Rescue · sense, then move" } else { "Rescue · go north immediately" }, description: if policy == Policy::ObserveThenFollow { "The agent spends the sensor, receives a reading, and follows it." } else { "The agent commits north without looking. In this scenario the survivor is south, and the replay keeps the failure." }, source_label: "Uncertain rescue" },
            &model, &rescue::goal(), rollout.trace(), vec![
                ObjectiveView::new(path!("success"), "Succeeded", ObjectiveDirectionView::Maximize, u64::from(rollout.succeeded())),
                ObjectiveView::new(path!("sensor"), "Sensor used", ObjectiveDirectionView::Minimize, u64::from(rollout.used_sensor())),
                ObjectiveView::new(path!("energy"), "Energy spent", ObjectiveDirectionView::Minimize, rollout.spent_energy()),
            ], scene,
        ).map_err(|error| problem_error(&study.problem, error))?;
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
            view.proposals.push(proposal(ProposalSpec { id: path!("unresolved-action"), label: "Act before Nature resolves", description: "Acting before Nature has decided what is true shows exactly which facts are still missing." }, &model, candidate));
        }
        documents.push(view);
    }
    Ok(documents)
}

fn observation(world: &World) -> ObservationView {
    let key = rescue::agent_view(world).observation_key();
    let accounts = key
        .visible_accounts()
        .iter()
        .map(|account| AccountView {
            account: ViewId::new(account_key::<rescue::Ontology>(account), account.label()),
            balances: key
                .balances()
                .iter()
                .filter(|(owner, _, _)| owner == account)
                .map(|(_, asset, quantity)| AssetQuantityView {
                    asset: ViewId::new(asset_key::<rescue::Ontology>(asset), asset.label()),
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
    sampled_policy_front(
        "Success vs. sensor use vs. energy — sampled, not exact",
        vec![
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
        front,
        |policy| {
            match policy {
                Policy::ObserveThenFollow => "Sense, then move",
                Policy::NorthWithoutObserving => "Go north immediately",
            }
            .into()
        },
        |policy| policy == selected,
    )
}
