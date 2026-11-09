use super::*;
use axionomy::{Exchange, Quantity};
use axionomy_problems::work_league::{
    self, AGENTS, AccountId, AgentId, Asset, Facility, JobId, Location, Policy, Profile, RateId,
    Role, World,
};
use axionomy_view::{
    GraphEdgeView, GraphNodeView, LeaderboardEntryView, LeaderboardView, ObjectiveDirectionView,
    SceneGlyphView, SceneToneView, TelemetryKindView,
};
use std::cmp::Ordering;

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = match run.instance {
        InstanceProfile::Micro => Profile::Micro,
        InstanceProfile::Showcase => Profile::Showcase,
        InstanceProfile::Stress => Profile::Stress,
    };
    let configurations = [
        (
            "mixed-field",
            work_league::mixed_lineup(),
            "Mixed policy field",
            "A sprinter, steward, value hunter, and resilient worker compete in the same finite job market.",
        ),
        (
            "throughput-field",
            work_league::throughput_lineup(),
            "Throughput field",
            "A speed-biased field shows what is gained—and wasted—when most workers optimize for short completion times.",
        ),
        (
            "sustainable-field",
            work_league::sustainable_lineup(),
            "Sustainable field",
            "A resource-conscious field recycles aggressively and exposes the opportunity cost of minimizing residual waste.",
        ),
    ];
    let selected = run.strategy.as_str().to_owned();
    let mut documents = Vec::new();
    for (offset, (strategy, lineup, title, description)) in configurations.into_iter().enumerate() {
        let league = work_league::league(profile, lineup);
        let outcome = work_league::run(&league, run.seed)
            .map_err(|error| problem_error("work-league", error))?;
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
        let document_id: DocumentPath = slug(strategy);
        let mut view = document_with_leaderboards_observed(
            DocumentSpec {
                problem: "work-league",
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
                    key: slug("value"),
                    label: "Contract value".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: total_value.to_string(),
                },
                ObjectiveView {
                    key: slug("completed"),
                    label: "Jobs completed".into(),
                    direction: ObjectiveDirectionView::Maximize,
                    value: completed.to_string(),
                },
                ObjectiveView {
                    key: slug("waste"),
                    label: "Residual waste".into(),
                    direction: ObjectiveDirectionView::Minimize,
                    value: waste.to_string(),
                },
            ],
            (scene, leaderboards),
            |frame| {
                if strategy == selected {
                    progress.frame(&document_id, frame);
                }
            },
        )
        .map_err(|error| problem_error("work-league", error))?;
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
        view.proposals.push(proposal(
            "work-league",
            ProposalSpec {
                id: "claim-with-wrong-worker",
                label: "Atlas claim rebound to Bolt",
                description: "The rate names Atlas, but the proposed role binding points at Bolt. Identity assets must reject the mismatch.",
            },
            league.initial(),
            &wrong_worker,
        ));
        documents.push(view);
        let _ = progress.emit(
            slug("multi-agent-match"),
            offset as u64 + 1,
            configurations.len() as u64,
            format!(
                "replayed {title}: {} atomic exchanges",
                outcome.trace().exchanges().len()
            ),
        );
        progress.ensure()?;
    }
    artifact(run, descriptor, documents)
}

fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
    world.balance(&AccountId::Agent(agent), &asset).to_u64()
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
            "contract-value",
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
            "resource-efficiency",
            "Resource efficiency",
            "Contract value per unit of energy and material spent.",
            ObjectiveDirectionView::Maximize,
            Some("value / resource"),
            rows.iter()
                .map(|row| row.ratio(row.value, row.energy + row.material, row.completed > 0))
                .collect(),
        ),
        ranking(
            "least-waste",
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
            visual_metric(slug("value"), "Contract value", self.value, Some("credits")),
            visual_metric(
                slug("completed"),
                "Jobs completed",
                self.completed,
                Some("jobs"),
            ),
            visual_metric(slug("elapsed"), "Elapsed time", self.elapsed, Some("ticks")),
            visual_metric(slug("energy"), "Energy spent", self.energy, Some("units")),
            visual_metric(
                slug("material"),
                "Material spent",
                self.material,
                Some("units"),
            ),
            visual_metric(slug("waste"), "Residual waste", self.waste, Some("units")),
            visual_metric(
                slug("recycled"),
                "Waste recycled",
                self.recycled,
                Some("units"),
            ),
            visual_metric(slug("attempts"), "Attempts", self.attempts, None),
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
            let rank = (standing.eligible
                && !previous.is_some_and(|known| ratios_equal(known, score)))
            .then_some(offset as u64 + 1);
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
                    slug(format!("work-league/account/agent-{:?}", standing.agent)),
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
        key: slug(key),
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
        "pareto-standing",
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
    let continuous = world.account(&AccountId::World).is_some();
    let tick = work_league::clock(world);
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
                slug(format!("league/location/{location:?}")),
                format!("{location:?}"),
            ),
            classes: if matches!(
                location,
                Location::Workshop | Location::Charger | Location::Recycler
            ) {
                vec![slug("facility")]
            } else {
                Vec::new()
            },
            x: Some(x),
            y: Some(y),
        })
        .collect::<Vec<_>>();
    let routes: std::collections::BTreeSet<_> = world
        .rate_ids()
        .filter_map(|id| match id {
            RateId::Move { from, to, .. } if from < to => Some((*from, *to)),
            _ => None,
        })
        .collect();
    let edges = routes
        .into_iter()
        .enumerate()
        .map(|(index, (from, to))| GraphEdgeView {
            id: slug(format!("league/route/{index}")),
            source: slug(format!("league/location/{from:?}")),
            target: slug(format!("league/location/{to:?}")),
            label: Some("1 energy · 1 tick".into()),
            classes: if AGENTS.into_iter().any(|agent| {
                world.account(&AccountId::Agent(agent)).is_some()
                    && work_league::location(world, agent) == Some(to)
            }) {
                vec![slug("current")]
            } else {
                Vec::new()
            },
        })
        .collect();
    let agents = AGENTS
        .into_iter()
        .filter(|agent| world.account(&AccountId::Agent(*agent)).is_some())
        .map(|agent| {
            let location = work_league::location(world, agent).unwrap_or(Location::Depot);
            let policy = work_league::policy(world, agent).unwrap_or(Policy::Sprinter);
            let mut entity = link_balance(
                visual_entity(
                    slug(format!("league/agent/{agent:?}")),
                    format!("{agent:?} · {policy:?}"),
                    SceneGlyphView::Robot,
                    SceneAnchorView::GraphNode {
                        node: slug(format!("league/location/{location:?}")),
                    },
                    SceneEntityRoleView::Occupant,
                    if balance(world, agent, Asset::Damage) > 0 {
                        SceneToneView::Danger
                    } else {
                        SceneToneView::Active
                    },
                    Some(if balance(world, agent, Asset::Busy) > 0 {
                        format!(
                            "{} · ready at {}",
                            [Facility::Workshop, Facility::Charger, Facility::Recycler]
                                .into_iter()
                                .find(
                                    |facility| balance(world, agent, Asset::Servicing(*facility))
                                        > 0
                                )
                                .map_or_else(
                                    || "working".into(),
                                    |facility| format!("{facility:?} service")
                                ),
                            balance(world, agent, Asset::ReadyAt)
                        )
                    } else if balance(world, agent, Asset::Damage) > 0 {
                        "damaged · repair or handoff needed".into()
                    } else {
                        format!(
                            "{} jobs · {} value",
                            balance(world, agent, Asset::Completed),
                            balance(world, agent, Asset::Value)
                        )
                    }),
                ),
                slug(format!("work-league/account/agent-{agent:?}")),
                slug(format!("work-league/asset/at-{location:?}")),
            );
            entity.metrics = vec![
                visual_metric(
                    slug("value"),
                    "Value",
                    balance(world, agent, Asset::Value),
                    Some("credits"),
                ),
                visual_metric(
                    slug("jobs"),
                    "Completed",
                    balance(world, agent, Asset::Completed),
                    Some("jobs"),
                ),
                visual_metric(
                    slug("energy"),
                    "Energy left",
                    balance(world, agent, Asset::Energy),
                    Some("units"),
                ),
                visual_metric(
                    slug("waste"),
                    "Residual waste",
                    balance(world, agent, Asset::Waste),
                    Some("units"),
                ),
            ];
            if continuous {
                entity.metrics.extend([
                    visual_metric(
                        slug("credit"),
                        "Spendable credit",
                        balance(world, agent, Asset::Credit),
                        Some("credits"),
                    ),
                    visual_metric(
                        slug("material"),
                        "Material left",
                        balance(world, agent, Asset::Material),
                        Some("units"),
                    ),
                    visual_metric(
                        slug("wear"),
                        "Equipment wear",
                        balance(world, agent, Asset::Wear),
                        None,
                    ),
                    visual_metric(
                        slug("handoffs"),
                        "Paid handoffs",
                        balance(world, agent, Asset::Assists),
                        Some("contracts"),
                    ),
                ]);
            }
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
        } else if balance_job(world, job, Asset::Expired) > 0 {
            (SceneToneView::Danger, "deadline missed".into())
        } else if continuous && tick < balance_job(world, job, Asset::ReleaseAt) {
            (
                SceneToneView::Neutral,
                format!("opens at {}", balance_job(world, job, Asset::ReleaseAt)),
            )
        } else if let Some(agent) = assigned {
            (
                SceneToneView::Active,
                format!(
                    "{} by {agent:?}",
                    if balance_job(world, job, Asset::InProgress) > 0 {
                        "being worked"
                    } else {
                        "claimed"
                    }
                ),
            )
        } else {
            (
                SceneToneView::Neutral,
                format!("{} credits · risk {}", spec.value, spec.risk),
            )
        };
        let mut entity = link_account(
            visual_entity(
                slug(format!("league/job/{}", job.0)),
                format!("Job {}", job.0),
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: slug(format!("league/location/{:?}", spec.location)),
                },
                SceneEntityRoleView::Attachment,
                tone,
                Some(status),
            ),
            slug(format!("work-league/account/job-jobid-{}", job.0)),
        );
        entity.metrics = vec![
            visual_metric(slug("value"), "Value", spec.value, Some("credits")),
            visual_metric(slug("risk"), "Failure weight", spec.risk, Some("of 10")),
        ];
        if continuous {
            entity.metrics.push(visual_metric(
                slug("deadline"),
                "Deadline",
                balance_job(world, job, Asset::Deadline),
                Some("tick"),
            ));
            if assigned.is_some() && balance_job(world, job, Asset::Pending) > 0 {
                entity.metrics.push(visual_metric(
                    slug("lease"),
                    "Claim expires after",
                    balance_job(world, job, Asset::LeaseUntil),
                    Some("tick"),
                ));
            }
        }
        Some(entity)
    });
    let facilities = [Facility::Workshop, Facility::Charger, Facility::Recycler]
        .into_iter()
        .filter_map(|facility| {
            if !continuous {
                return None;
            }
            let occupant = AGENTS.into_iter().find(|agent| {
                !world
                    .balance(
                        &AccountId::Facility(facility),
                        &Asset::Occupied(facility, *agent),
                    )
                    .is_zero()
            });
            let location = match facility {
                Facility::Workshop => Location::Workshop,
                Facility::Charger => Location::Charger,
                Facility::Recycler => Location::Recycler,
            };
            let mut entity = link_account(
                visual_entity(
                    slug(format!("league/facility/{facility:?}")),
                    format!("{facility:?} bay"),
                    SceneGlyphView::Task,
                    SceneAnchorView::GraphNode {
                        node: slug(format!("league/location/{location:?}")),
                    },
                    SceneEntityRoleView::Attachment,
                    if occupant.is_some() {
                        SceneToneView::Active
                    } else {
                        SceneToneView::Neutral
                    },
                    Some(occupant.map_or_else(
                        || "free".into(),
                        |agent| {
                            format!("{agent:?} until {}", balance(world, agent, Asset::ReadyAt))
                        },
                    )),
                ),
                slug(format!("work-league/account/facility-{facility:?}")),
            );
            entity.metrics.push(visual_metric(
                slug("waiting"),
                "Workers waiting",
                AGENTS
                    .into_iter()
                    .filter(|agent| {
                        work_league::location(world, *agent) == Some(location)
                            && balance(world, *agent, Asset::Idle) > 0
                    })
                    .count(),
                Some("workers"),
            ));
            Some(entity)
        });
    let completed = (1u8..=24)
        .filter(|number| balance_job(world, work_league::JobId(*number), Asset::Completed) > 0)
        .count();
    let mut metrics = vec![
        visual_metric(slug("completed"), "Jobs completed", completed, Some("jobs")),
        visual_metric(
            slug("active-agents"),
            "Competing agents",
            AGENTS
                .into_iter()
                .filter(|agent| world.account(&AccountId::Agent(*agent)).is_some())
                .count(),
            Some("agents"),
        ),
        visual_metric(
            slug("repair-supply"),
            "Repair supply",
            world.balance(
                &AccountId::Facility(Facility::Workshop),
                &Asset::RepairSupply,
            ),
            Some("units"),
        ),
    ];
    if continuous {
        metrics.insert(
            0,
            visual_metric(slug("clock"), "World time", tick, Some("ticks")),
        );
        metrics.insert(
            2,
            visual_metric(
                slug("expired"),
                "Missed deadlines",
                (1u8..=24)
                    .filter(|number| balance_job(world, JobId(*number), Asset::Expired) > 0)
                    .count(),
                Some("jobs"),
            ),
        );
        metrics.push(visual_metric(
            slug("activities"),
            "Simultaneous activities",
            world.balance(&AccountId::World, &Asset::Activities),
            Some("workers"),
        ));
    }
    Some(
        Scene::graph("Autonomous work network", nodes, edges, None)
            .with_entities(agents.chain(jobs).chain(facilities))
            .with_metrics(metrics),
    )
}

fn balance_job(world: &World, job: work_league::JobId, asset: Asset) -> u64 {
    world.balance(&AccountId::Job(job), &asset).to_u64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReferenceService, RunRequest};

    #[test]
    fn every_snapshot_has_multiple_rankings_and_they_change() {
        let request = RunRequest::new(slug("work-league"))
            .with_instance(InstanceProfile::Showcase)
            .with_strategy(slug("mixed-field"));
        let run = ReferenceService.resolve(&request).unwrap();
        let descriptor = ReferenceService.problem(&run.problem).unwrap();
        let artifact = build(
            &run,
            &descriptor,
            &mut ProgressSink::new(&RunControl::default(), &mut |_| {}),
        )
        .unwrap();
        let document = artifact.selected().unwrap();
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
                    .find(|board| board.key.as_str() == "contract-value")?
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
