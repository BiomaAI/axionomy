use super::*;
use axionomy_problems::work_league::{
    self, AGENTS, AccountId, AgentId, Asset, Facility, Location, Policy, Profile, World,
};
use axionomy_view::{
    GraphEdgeView, GraphNodeView, LeaderboardEntryView, LeaderboardView, ObjectiveDirectionView,
    SceneGlyphView, SceneToneView, TelemetryKindView,
};
use std::cmp::Ordering;

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = match instance_profile(request, descriptor) {
        InstanceProfile::Micro => Profile::Micro,
        InstanceProfile::Showcase => Profile::Showcase,
        InstanceProfile::Stress => Profile::Stress,
    };
    let configurations = [
        (
            "mixed_field",
            work_league::mixed_lineup(),
            "Mixed policy field",
            "A sprinter, steward, value hunter, and resilient worker compete in the same finite job market.",
        ),
        (
            "throughput_field",
            work_league::throughput_lineup(),
            "Throughput field",
            "A speed-biased field shows what is gained—and wasted—when most workers optimize for short completion times.",
        ),
        (
            "sustainable_field",
            work_league::sustainable_lineup(),
            "Sustainable field",
            "A resource-conscious field recycles aggressively and exposes the opportunity cost of minimizing residual waste.",
        ),
    ];
    let mut documents = Vec::new();
    for (offset, (strategy, lineup, title, description)) in configurations.into_iter().enumerate() {
        let league = work_league::league(profile, lineup);
        let outcome = work_league::run(&league, request.seed.wrapping_add(offset as u64 * 10_007))
            .map_err(|error| problem_error("work_league", error))?;
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
        let document_id = format!("work_league:{strategy}");
        let mut view = document_with_leaderboards_observed(
            DocumentSpec {
                problem: "work_league",
                strategy,
                title: &format!("Work League · {title}"),
                description,
                source_label: "Competitive multi-agent work allocation",
            },
            league.initial(),
            league.goal(),
            outcome.trace(),
            vec![
                ObjectiveView {
                    key: "value".into(),
                    label: "Contract value".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: total_value.to_string(),
                },
                ObjectiveView {
                    key: "completed".into(),
                    label: "Jobs completed".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: completed.to_string(),
                },
                ObjectiveView {
                    key: "waste".into(),
                    label: "Residual waste".into(),
                    direction: ObjectiveDirectionView::Minimize,
                    value: waste.to_string(),
                },
            ],
            scene,
            leaderboards,
            |frame| progress.frame(&document_id, frame),
        )
        .map_err(|error| problem_error("work_league", error))?;
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
                    "finite jobs claimed exactly once".into(),
                ),
            ],
        ));
        documents.push(view);
        let _ = progress.emit(
            "multi_agent_match",
            offset as u64 + 1,
            configurations.len() as u64,
            format!(
                "replayed {title}: {} atomic exchanges",
                outcome.trace().exchanges().len()
            ),
        );
        progress.ensure()?;
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        documents,
    )
}

fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
    world.balance(&AccountId::Agent(agent), &asset).get()
}

#[derive(Clone)]
struct Standing {
    agent: AgentId,
    numerator: u64,
    denominator: u64,
    eligible: bool,
    components: Vec<SceneMetricView>,
    display: Option<String>,
}

fn leaderboards(_: u64, world: &World) -> Vec<LeaderboardView> {
    let active = AGENTS
        .into_iter()
        .filter(|agent| world.account(&AccountId::Agent(*agent)).is_some())
        .collect::<Vec<_>>();
    let rows = active
        .iter()
        .copied()
        .map(|agent| AgentMetrics::new(world, agent))
        .collect::<Vec<_>>();
    vec![
        ranking(
            "contract_value",
            "Contract value",
            "Total value earned from completed jobs.",
            ObjectiveDirectionView::Maximize,
            Some("credits"),
            rows.iter().map(|row| row.scalar(row.value, true)).collect(),
        ),
        ranking(
            "throughput",
            "Fastest throughput",
            "Completed jobs per elapsed tick; agents without a completion are not yet eligible.",
            ObjectiveDirectionView::Maximize,
            Some("jobs / tick"),
            rows.iter()
                .map(|row| row.ratio(row.completed, row.elapsed, row.completed > 0))
                .collect(),
        ),
        ranking(
            "resource_efficiency",
            "Resource efficiency",
            "Contract value per unit of energy and material spent.",
            ObjectiveDirectionView::Maximize,
            Some("value / resource"),
            rows.iter()
                .map(|row| row.ratio(row.value, row.energy + row.material, row.completed > 0))
                .collect(),
        ),
        ranking(
            "least_waste",
            "Least residual waste",
            "Waste still held after recycling; only agents that completed work are ranked.",
            ObjectiveDirectionView::Minimize,
            Some("units"),
            rows.iter()
                .map(|row| row.scalar(row.waste, row.completed > 0))
                .collect(),
        ),
        ranking(
            "reliability",
            "Reliability",
            "Successful jobs per attempt, retaining failures rather than hiding them.",
            ObjectiveDirectionView::Maximize,
            Some("successes / attempt"),
            rows.iter()
                .map(|row| row.ratio(row.successes, row.attempts, row.attempts > 0))
                .collect(),
        ),
        pareto_ranking(&rows),
    ]
}

struct AgentMetrics {
    agent: AgentId,
    value: u64,
    completed: u64,
    attempts: u64,
    successes: u64,
    elapsed: u64,
    energy: u64,
    material: u64,
    waste: u64,
    recycled: u64,
}

impl AgentMetrics {
    fn new(world: &World, agent: AgentId) -> Self {
        Self {
            agent,
            value: balance(world, agent, Asset::Value),
            completed: balance(world, agent, Asset::Completed),
            attempts: balance(world, agent, Asset::Attempts),
            successes: balance(world, agent, Asset::Successes),
            elapsed: balance(world, agent, Asset::ElapsedTime),
            energy: balance(world, agent, Asset::SpentEnergy),
            material: balance(world, agent, Asset::MaterialSpent),
            waste: balance(world, agent, Asset::Waste),
            recycled: balance(world, agent, Asset::RecycledWaste),
        }
    }

    fn components(&self) -> Vec<SceneMetricView> {
        vec![
            visual_metric("value", "Contract value", self.value, Some("credits")),
            visual_metric("completed", "Jobs completed", self.completed, Some("jobs")),
            visual_metric("elapsed", "Elapsed time", self.elapsed, Some("ticks")),
            visual_metric("energy", "Energy spent", self.energy, Some("units")),
            visual_metric("material", "Material spent", self.material, Some("units")),
            visual_metric("waste", "Residual waste", self.waste, Some("units")),
            visual_metric("recycled", "Waste recycled", self.recycled, Some("units")),
            visual_metric("attempts", "Attempts", self.attempts, None),
        ]
    }

    fn scalar(&self, value: u64, eligible: bool) -> Standing {
        Standing {
            agent: self.agent,
            numerator: value,
            denominator: 1,
            eligible,
            components: self.components(),
            display: None,
        }
    }

    fn ratio(&self, numerator: u64, denominator: u64, eligible: bool) -> Standing {
        let divisor = gcd(numerator, denominator.max(1));
        Standing {
            agent: self.agent,
            numerator: numerator / divisor,
            denominator: denominator.max(1) / divisor,
            eligible,
            components: self.components(),
            display: None,
        }
    }
}

fn ranking(
    key: &str,
    label: &str,
    description: &str,
    direction: ObjectiveDirectionView,
    unit: Option<&str>,
    mut standings: Vec<Standing>,
) -> LeaderboardView {
    standings.sort_by(|left, right| match (left.eligible, right.eligible) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => {
            let comparison = (left.numerator as u128 * right.denominator as u128)
                .cmp(&(right.numerator as u128 * left.denominator as u128));
            let comparison = if direction == ObjectiveDirectionView::Maximize {
                comparison.reverse()
            } else {
                comparison
            };
            comparison.then_with(|| left.agent.cmp(&right.agent))
        }
    });
    let mut previous: Option<(u64, u64)> = None;
    let entries = standings
        .into_iter()
        .enumerate()
        .map(|(offset, standing)| {
            let score = (standing.numerator, standing.denominator);
            let rank = if !standing.eligible {
                None
            } else if previous.is_some_and(|known| ratios_equal(known, score)) {
                None // replaced below from the preceding entry
            } else {
                Some(offset as u64 + 1)
            };
            previous = standing.eligible.then_some(score);
            (rank, standing)
        })
        .scan(None, |last_rank, (rank, standing)| {
            if rank.is_some() {
                *last_rank = rank;
            }
            Some(LeaderboardEntryView {
                rank: standing.eligible.then_some((*last_rank).unwrap_or(1)),
                participant: ViewId::new(
                    format!("work_league:account:agent-{:?}", standing.agent).to_ascii_lowercase(),
                    format!("{:?}", standing.agent),
                ),
                value: standing.display.unwrap_or_else(|| {
                    if standing.denominator == 1 {
                        standing.numerator.to_string()
                    } else {
                        format!("{}/{}", standing.numerator, standing.denominator)
                    }
                }),
                unit: unit.map(str::to_owned),
                eligible: standing.eligible,
                components: standing.components,
            })
        })
        .collect();
    LeaderboardView {
        key: key.into(),
        label: label.into(),
        description: description.into(),
        direction,
        entries,
    }
}

fn pareto_ranking(rows: &[AgentMetrics]) -> LeaderboardView {
    let standings = rows
        .iter()
        .map(|row| {
            let dominated = row.completed > 0
                && rows.iter().any(|other| {
                    other.agent != row.agent
                        && other.completed > 0
                        && other.value >= row.value
                        && other.completed >= row.completed
                        && other.waste <= row.waste
                        && other.elapsed <= row.elapsed
                        && (other.value > row.value
                            || other.completed > row.completed
                            || other.waste < row.waste
                            || other.elapsed < row.elapsed)
                });
            Standing {
                agent: row.agent,
                numerator: u64::from(!dominated),
                denominator: 1,
                eligible: row.completed > 0,
                components: row.components(),
                display: Some(if dominated {
                    "dominated".into()
                } else {
                    "non-dominated".into()
                }),
            }
        })
        .collect();
    ranking(
        "pareto_standing",
        "Pareto standing",
        "Non-dominated agents are not worse on value, completions, residual waste, and elapsed time all at once.",
        ObjectiveDirectionView::Maximize,
        None,
        standings,
    )
}

fn ratios_equal(left: (u64, u64), right: (u64, u64)) -> bool {
    left.0 as u128 * right.1 as u128 == right.0 as u128 * left.1 as u128
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let locations = [
        (Location::Depot, 360.0, 210.0),
        (Location::North, 360.0, 40.0),
        (Location::East, 650.0, 170.0),
        (Location::South, 360.0, 380.0),
        (Location::West, 70.0, 170.0),
        (Location::Workshop, 80.0, 340.0),
        (Location::Charger, 640.0, 340.0),
        (Location::Recycler, 650.0, 40.0),
    ];
    let nodes = locations
        .into_iter()
        .map(|(location, x, y)| GraphNodeView {
            id: ViewId::new(
                format!("league:location:{location:?}").to_ascii_lowercase(),
                format!("{location:?}"),
            ),
            classes: if matches!(
                location,
                Location::Workshop | Location::Charger | Location::Recycler
            ) {
                vec!["facility".into()]
            } else {
                Vec::new()
            },
            x: Some(x),
            y: Some(y),
        })
        .collect::<Vec<_>>();
    let edges = [
        (Location::Depot, Location::North),
        (Location::Depot, Location::East),
        (Location::Depot, Location::South),
        (Location::Depot, Location::West),
        (Location::West, Location::Workshop),
        (Location::East, Location::Charger),
        (Location::North, Location::Recycler),
        (Location::South, Location::Workshop),
        (Location::South, Location::Charger),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (from, to))| GraphEdgeView {
        id: format!("league:route:{index}"),
        source: format!("league:location:{from:?}").to_ascii_lowercase(),
        target: format!("league:location:{to:?}").to_ascii_lowercase(),
        label: Some("1 energy · 1 tick".into()),
        classes: Vec::new(),
    })
    .collect();
    let agents = AGENTS
        .into_iter()
        .filter(|agent| world.account(&AccountId::Agent(*agent)).is_some())
        .map(|agent| {
            let location = work_league::location(world, agent).unwrap_or(Location::Depot);
            let policy = work_league::policy(world, agent).unwrap_or(Policy::Sprinter);
            let mut entity = visual_entity(
                format!("league:agent:{agent:?}").to_ascii_lowercase(),
                format!("{agent:?} · {policy:?}"),
                SceneGlyphView::Robot,
                SceneAnchorView::GraphNode {
                    node: format!("league:location:{location:?}").to_ascii_lowercase(),
                },
                if balance(world, agent, Asset::Damage) > 0 {
                    SceneToneView::Danger
                } else {
                    SceneToneView::Active
                },
                Some(format!(
                    "{} jobs · {} value",
                    balance(world, agent, Asset::Completed),
                    balance(world, agent, Asset::Value)
                )),
            );
            entity.account =
                Some(format!("work_league:account:agent-{agent:?}").to_ascii_lowercase());
            entity.metrics = vec![
                visual_metric(
                    "value",
                    "Value",
                    balance(world, agent, Asset::Value),
                    Some("credits"),
                ),
                visual_metric(
                    "jobs",
                    "Completed",
                    balance(world, agent, Asset::Completed),
                    Some("jobs"),
                ),
                visual_metric(
                    "energy",
                    "Energy left",
                    balance(world, agent, Asset::Energy),
                    Some("units"),
                ),
                visual_metric(
                    "waste",
                    "Residual waste",
                    balance(world, agent, Asset::Waste),
                    Some("units"),
                ),
            ];
            entity
        });
    let jobs = (1u8..=24).filter_map(|number| {
        let job = work_league::JobId(number);
        let account = AccountId::Job(job);
        world.account(&account)?;
        let spec = work_league::job_spec(job);
        let completed = balance_job(world, job, Asset::Completed) > 0;
        let assigned = AGENTS
            .into_iter()
            .find(|agent| balance_job(world, job, Asset::Assigned(*agent)) > 0);
        let (tone, status) = if completed {
            (SceneToneView::Success, "completed".into())
        } else if let Some(agent) = assigned {
            (SceneToneView::Active, format!("claimed by {agent:?}"))
        } else {
            (
                SceneToneView::Neutral,
                format!("{} credits · risk {}", spec.value, spec.risk),
            )
        };
        let mut entity = visual_entity(
            format!("league:job:{}", job.0),
            format!("Job {}", job.0),
            SceneGlyphView::Task,
            SceneAnchorView::GraphNode {
                node: format!("league:location:{:?}", spec.location).to_ascii_lowercase(),
            },
            tone,
            Some(status),
        );
        entity.account = Some(format!("work_league:account:job-jobid-{}", job.0));
        entity.metrics = vec![
            visual_metric("value", "Value", spec.value, Some("credits")),
            visual_metric("risk", "Failure weight", spec.risk, Some("of 10")),
        ];
        Some(entity)
    });
    let completed = (1u8..=24)
        .filter(|number| balance_job(world, work_league::JobId(*number), Asset::Completed) > 0)
        .count();
    Some(
        Scene::graph("Autonomous work network", nodes, edges, None)
            .with_entities(agents.chain(jobs))
            .with_metrics([
                visual_metric("completed", "Jobs completed", completed, Some("jobs")),
                visual_metric(
                    "active_agents",
                    "Competing agents",
                    AGENTS
                        .into_iter()
                        .filter(|agent| world.account(&AccountId::Agent(*agent)).is_some())
                        .count(),
                    Some("agents"),
                ),
                visual_metric(
                    "repair_supply",
                    "Repair supply",
                    world.balance(
                        &AccountId::Facility(Facility::Workshop),
                        &Asset::RepairSupply,
                    ),
                    Some("units"),
                ),
            ]),
    )
}

fn balance_job(world: &World, job: work_league::JobId, asset: Asset) -> u64 {
    world.balance(&AccountId::Job(job), &asset).get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_snapshot_has_multiple_rankings_and_they_change() {
        let request = RunRequest::new("work_league")
            .with_instance("showcase")
            .with_strategy("mixed_field");
        let descriptor = ReferenceService.problem("work_league").unwrap();
        let artifact = build(
            &request,
            &descriptor,
            &mut ProgressSink::new(&RunControl::default(), &mut |_| {}),
        )
        .unwrap();
        let document = artifact.selected_document().unwrap();
        assert_eq!(document.initial.leaderboards.len(), 6);
        assert!(document.frames.len() >= 50);
        assert!(
            document
                .frames
                .iter()
                .any(|frame| frame.after.leaderboards != document.initial.leaderboards)
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
