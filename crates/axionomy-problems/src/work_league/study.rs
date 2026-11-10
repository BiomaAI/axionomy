//! The Autonomous Work League as a study: its catalog entry, strategies and documents.

use super::view::{leaderboards, scene};
use crate::{
    support::{ProblemCopy, StrategyCopy, problem},
    work_league::{self, AccountId, AgentId, Asset, JobId, Profile, RateId, Role, World},
};
use axionomy::{Exchange, Quantity, path};
use axionomy_service::{
    Capability, DocumentSpec, InstanceProfile, Problem, ProblemDescriptor, ProblemFamily,
    ProposalSpec, ServiceError, StudyKey, StudyProgress, Work, document_with_leaderboards_observed,
    problem_error, proposal, strategy_document, telemetry,
};
use axionomy_view::{
    ObjectiveDirectionView, ObjectiveView, SearchObservationKindView, TelemetryKindView,
    ViewDocument,
};

/// Autonomous Work League: its catalog entry and how its studies are computed.
pub struct WorkLeague {
    descriptor: ProblemDescriptor,
}

impl Default for WorkLeague {
    fn default() -> Self {
        Self {
            descriptor: problem(
                ProblemCopy {
                    key: path!("work-league"),
                    title: "Autonomous Work League",
                    summary: "Four workers share one clock: jobs overlap, repair and charging bays fill, claims expire, and deadlines can be missed. Earned credit funds recovery; recycling restores material; a worker can pay to take over another's contract. Outcomes compare the same job-attempt disruptions across policy fields.",
                    instances: [
                        "Two agents contest four jobs in a short but complete match",
                        "Four policies contest 12 arriving contracts with deadlines, dependencies, occupied facilities, and paid handoffs",
                        "24 contracts over six arrival waves with shared finite supplies and equipment wear",
                    ],
                },
                ProblemFamily::MultiAgentCompetition,
                path!("mixed-field"),
                &[
                    StrategyCopy {
                        key: path!("mixed-field"),
                        label: "Mixed policy field",
                        description: "A sprinter, steward, value hunter, and resilient worker reveal genuine cross-objective tradeoffs.",
                        algorithm: "deterministic policy league with seeded chance",
                    },
                    StrategyCopy {
                        key: path!("throughput-field"),
                        label: "Throughput field",
                        description: "Most workers prioritize short jobs and fast execution; one still chases contract value.",
                        algorithm: "throughput-biased policy league with seeded chance",
                    },
                    StrategyCopy {
                        key: path!("sustainable-field"),
                        label: "Sustainable field",
                        description: "Most workers minimize material loss and recycle, while one specializes in reliability.",
                        algorithm: "resource-biased policy league with seeded chance",
                    },
                ],
                &[
                    Capability::MultiAccountExchange,
                    Capability::AtomicSettlement,
                    Capability::Chance,
                    Capability::ReplayDerivedLeaderboards,
                    Capability::ApproximatePareto,
                    Capability::RlProjection,
                ],
            ),
        }
    }
}

impl Problem for WorkLeague {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.descriptor
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, ServiceError> {
        build(key, progress)
    }
}

fn build(
    study: &StudyKey,
    progress: &mut StudyProgress<'_>,
) -> Result<Vec<ViewDocument>, ServiceError> {
    let profile = match study.instance {
        InstanceProfile::Micro => Profile::Micro,
        InstanceProfile::Showcase => Profile::Showcase,
        InstanceProfile::Stress => Profile::Stress,
    };
    let configurations = [
        (
            path!("mixed-field"),
            work_league::mixed_lineup(),
            "Mixed policy field",
            "A sprinter, steward, value hunter, and resilient worker compete in the same finite job market.",
        ),
        (
            path!("throughput-field"),
            work_league::throughput_lineup(),
            "Throughput field",
            "A speed-biased field shows what is gained—and wasted—when most workers optimize for short completion times.",
        ),
        (
            path!("sustainable-field"),
            work_league::sustainable_lineup(),
            "Sustainable field",
            "A resource-conscious field recycles aggressively and exposes the opportunity cost of minimizing residual waste.",
        ),
    ];
    let selected = strategy_document(&study.strategy);
    let count = configurations.len() as u64;
    let mut documents = Vec::new();
    for (offset, (strategy, lineup, title, description)) in configurations.into_iter().enumerate() {
        let league = work_league::league(profile, lineup);
        let outcome = work_league::run(&league, study.seed)
            .map_err(|error| problem_error(&study.problem, error))?;
        let final_world = outcome.final_world();
        let total_value = league
            .agents()
            .iter()
            .map(|agent| balance(final_world, *agent, Asset::Value))
            .sum::<u64>();
        let completed = league
            .agents()
            .iter()
            .map(|agent| balance(final_world, *agent, Asset::Completed))
            .sum::<u64>();
        let waste = league
            .agents()
            .iter()
            .map(|agent| balance(final_world, *agent, Asset::Waste))
            .sum::<u64>();
        let streamed = strategy == selected;
        let mut view = document_with_leaderboards_observed(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
                title: &format!("Work League · {title}"),
                description,
                source_label: "Competitive multi-agent work allocation",
            },
            league.initial(),
            league.goal(),
            outcome.trace(),
            vec![
                ObjectiveView::new(
                    path!("value"),
                    "Contract value",
                    ObjectiveDirectionView::Maximize,
                    total_value,
                ),
                ObjectiveView::new(
                    path!("completed"),
                    "Jobs completed",
                    ObjectiveDirectionView::Maximize,
                    completed,
                ),
                ObjectiveView::new(
                    path!("waste"),
                    "Residual waste",
                    ObjectiveDirectionView::Minimize,
                    waste,
                ),
            ],
            (scene, leaderboards),
            streamed,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.telemetry.push(telemetry(
            "Seeded multi-agent policy match",
            true,
            [
                (
                    TelemetryKindView::Transitions,
                    outcome.trace().exchanges().len() as u64,
                    "replay-verified atomic exchanges".into(),
                ),
                (
                    TelemetryKindView::Alternatives,
                    league.agents().len() as u64,
                    "competing agents".into(),
                ),
                (
                    TelemetryKindView::Constraints,
                    league.jobs().len() as u64,
                    "finite contracts with exclusive live claims".into(),
                ),
            ],
        ));
        let wrong_worker = Exchange::new(
            RateId::Claim {
                agent: AgentId::Atlas,
                job: JobId(1),
            },
            Quantity::new(1),
        )
        .bind(Role::Agent, AccountId::Agent(AgentId::Bolt))
        .bind(Role::Job, AccountId::Job(JobId(1)));
        view.proposals.push(proposal(ProposalSpec {
                id: path!("claim-with-wrong-worker"),
                label: "Atlas claim rebound to Bolt",
                description: "The rate names Atlas, but the proposed role binding points at Bolt. Identity assets must reject the mismatch.",
            },
            league.initial(),
            &wrong_worker,
        ));
        documents.push(view);
        let _ = progress.emit(
            &Work::new(
                path!("multi-agent-match"),
                path!("multi-agent-match"),
                SearchObservationKindView::Phase,
            ),
            offset as u64 + 1,
            count,
            format!(
                "Replayed {title}: {} atomic exchanges",
                outcome.trace().exchanges().len()
            ),
        );
        progress.checkpoint()?;
    }
    Ok(documents)
}

pub(super) fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
    world.balance(&AccountId::Agent(agent), &asset).to_u64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;
    use axionomy_service::StudyRequest;

    #[test]
    fn every_snapshot_has_multiple_rankings_and_they_change() {
        let request = StudyRequest::new(path!("work-league"))
            .with_instance(InstanceProfile::Showcase)
            .with_strategy(path!("mixed-field"));
        let study = registry().study(request).unwrap();
        let document = study.selected().unwrap();
        assert_eq!(document.initial.leaderboards.len(), 6);
        assert!(document.frames.len() >= 50);
        assert!(
            document
                .frames
                .iter()
                .any(|frame| frame.after.leaderboards != document.initial.leaderboards)
        );
        let value_leaders = document
            .frames
            .iter()
            .filter_map(|frame| {
                frame
                    .after
                    .leaderboards
                    .iter()
                    .find(|board| board.key == path!("contract-value"))?
                    .entries
                    .first()
                    .map(|entry| entry.participant.key.as_str())
            })
            .collect::<std::collections::HashSet<_>>();
        assert!(
            value_leaders.len() >= 2,
            "the contract-value lead should change during replay"
        );
        assert!(
            document
                .frames
                .last()
                .unwrap()
                .after
                .leaderboards
                .iter()
                .filter_map(|board| board.entries.first())
                .map(|entry| entry.participant.key.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len()
                >= 2
        );
    }
}
