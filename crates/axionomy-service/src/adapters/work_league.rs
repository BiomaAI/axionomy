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

/// Autonomous Work League: its catalog entry and how its studies are computed.
pub(crate) struct WorkLeague {
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

pub(super) fn build(
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
            path!("contract-value"),
            "Contract value",
            "Total value earned from completed jobs.",
            ObjectiveDirectionView::Maximize,
            Some("credits"),
            rows.iter().map(|row| row.scalar(row.value, true)).collect(),
        ),
        ranking(
            path!("throughput"),
            "Fastest throughput",
            "Completed jobs per elapsed tick; agents without a completion are not yet eligible.",
            ObjectiveDirectionView::Maximize,
            Some("jobs / tick"),
            rows.iter()
                .map(|row| row.ratio(row.completed, row.elapsed, row.completed > 0))
                .collect(),
        ),
        ranking(
            path!("resource-efficiency"),
            "Resource efficiency",
            "Contract value per unit of energy and material spent.",
            ObjectiveDirectionView::Maximize,
            Some("value / resource"),
            rows.iter()
                .map(|row| row.ratio(row.value, row.energy + row.material, row.completed > 0))
                .collect(),
        ),
        ranking(
            path!("least-waste"),
            "Least residual waste",
            "Waste still held after recycling; only agents that completed work are ranked.",
            ObjectiveDirectionView::Minimize,
            Some("units"),
            rows.iter()
                .map(|row| row.scalar(row.waste, row.completed > 0))
                .collect(),
        ),
        ranking(
            path!("reliability"),
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
            asset_metric(path!("value"), "Contract value", &Asset::Value, self.value),
            visual_metric(
                path!("completed"),
                "Jobs completed",
                self.completed,
                Some("jobs"),
            ),
            asset_metric(
                path!("elapsed"),
                "Elapsed time",
                &Asset::ElapsedTime,
                self.elapsed,
            ),
            asset_metric(
                path!("energy"),
                "Energy spent",
                &Asset::SpentEnergy,
                self.energy,
            ),
            asset_metric(
                path!("material"),
                "Material spent",
                &Asset::MaterialSpent,
                self.material,
            ),
            asset_metric(path!("waste"), "Residual waste", &Asset::Waste, self.waste),
            asset_metric(
                path!("recycled"),
                "Waste recycled",
                &Asset::RecycledWaste,
                self.recycled,
            ),
            asset_metric(
                path!("attempts"),
                "Attempts",
                &Asset::Attempts,
                self.attempts,
            ),
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
    key: MeasurePath,
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
                    account_key::<work_league::Ontology>(&AccountId::Agent(standing.agent)),
                    AccountId::Agent(standing.agent).studio_label(),
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
        key,
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
        path!("pareto-standing"),
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
                path!("league/location").then(&location),
                format!("{location:?}"),
            ),
            classes: if matches!(
                location,
                Location::Workshop | Location::Charger | Location::Recycler
            ) {
                vec![path!("facility")]
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
            id: path!("league/route").then(&index),
            source: path!("league/location").then(&from),
            target: path!("league/location").then(&to),
            label: Some("1 energy · 1 tick".into()),
            classes: if AGENTS.into_iter().any(|agent| {
                world.account(&AccountId::Agent(agent)).is_some()
                    && work_league::location(world, agent) == Some(to)
            }) {
                vec![path!("current")]
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
            let mut entity = link_balance::<work_league::Ontology>(
                visual_entity(
                    path!("league/agent").then(&agent),
                    format!("{agent:?}"),
                    SceneGlyphView::Robot,
                    SceneAnchorView::GraphNode {
                        node: path!("league/location").then(&location),
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
                &AccountId::Agent(agent),
                &Asset::At(location),
            );
            entity.metrics = vec![
                visual_metric(path!("policy"), "Policy", policy.studio_label(), None),
                asset_metric(
                    path!("value"),
                    "Value",
                    &Asset::Value,
                    balance(world, agent, Asset::Value),
                ),
                visual_metric(
                    path!("jobs"),
                    "Completed",
                    balance(world, agent, Asset::Completed),
                    Some("jobs"),
                ),
                asset_metric(
                    path!("energy"),
                    "Energy left",
                    &Asset::Energy,
                    balance(world, agent, Asset::Energy),
                ),
                asset_metric(
                    path!("waste"),
                    "Residual waste",
                    &Asset::Waste,
                    balance(world, agent, Asset::Waste),
                ),
            ];
            if continuous {
                entity.metrics.extend([
                    asset_metric(
                        path!("credit"),
                        "Spendable credit",
                        &Asset::Credit,
                        balance(world, agent, Asset::Credit),
                    ),
                    asset_metric(
                        path!("material"),
                        "Material left",
                        &Asset::Material,
                        balance(world, agent, Asset::Material),
                    ),
                    asset_metric(
                        path!("wear"),
                        "Equipment wear",
                        &Asset::Wear,
                        balance(world, agent, Asset::Wear),
                    ),
                    visual_metric(
                        path!("handoffs"),
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
        let mut entity = link_account::<work_league::Ontology>(
            visual_entity(
                path!("league/job").then(&job.0),
                format!("Job {}", job.0),
                SceneGlyphView::Task,
                SceneAnchorView::GraphNode {
                    node: path!("league/location").then(&spec.location),
                },
                SceneEntityRoleView::Attachment,
                tone,
                Some(status),
            ),
            &account,
        );
        entity.metrics = vec![
            asset_metric(path!("value"), "Value", &Asset::Value, spec.value),
            visual_metric(path!("risk"), "Failure weight", spec.risk, Some("of 10")),
        ];
        if continuous {
            entity.metrics.push(asset_metric(
                path!("deadline"),
                "Deadline",
                &Asset::Deadline,
                balance_job(world, job, Asset::Deadline),
            ));
            if assigned.is_some() && balance_job(world, job, Asset::Pending) > 0 {
                entity.metrics.push(asset_metric(
                    path!("lease"),
                    "Claim expires after",
                    &Asset::LeaseUntil,
                    balance_job(world, job, Asset::LeaseUntil),
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
            let mut entity = link_account::<work_league::Ontology>(
                visual_entity(
                    path!("league/facility").then(&facility),
                    format!("{facility:?} bay"),
                    SceneGlyphView::Task,
                    SceneAnchorView::GraphNode {
                        node: path!("league/location").then(&location),
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
                &AccountId::Facility(facility),
            );
            entity.metrics.push(visual_metric(
                path!("waiting"),
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
        visual_metric(
            path!("completed"),
            "Jobs completed",
            completed,
            Some("jobs"),
        ),
        visual_metric(
            path!("active-agents"),
            "Competing agents",
            AGENTS
                .into_iter()
                .filter(|agent| world.account(&AccountId::Agent(*agent)).is_some())
                .count(),
            Some("agents"),
        ),
        asset_metric(
            path!("repair-supply"),
            "Repair supply",
            &Asset::RepairSupply,
            world.balance(
                &AccountId::Facility(Facility::Workshop),
                &Asset::RepairSupply,
            ),
        ),
    ];
    if continuous {
        metrics.insert(
            0,
            asset_metric(path!("clock"), "World time", &Asset::Tick, tick),
        );
        metrics.insert(
            2,
            visual_metric(
                path!("expired"),
                "Missed deadlines",
                (1u8..=24)
                    .filter(|number| balance_job(world, JobId(*number), Asset::Expired) > 0)
                    .count(),
                Some("jobs"),
            ),
        );
        metrics.push(visual_metric(
            path!("activities"),
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
    use crate::StudyRequest;

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

// How Studio names and writes this problem's values.

/// Time reads in ticks of the shared work clock, earnings in credits, and
/// energy, material, waste and supplies in units, as the league's scene
/// reads them.
impl StudioFormat for work_league::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Tick
            | Self::Horizon
            | Self::ReadyAt
            | Self::ReleaseAt
            | Self::Deadline
            | Self::LeaseUntil
            | Self::TimeRemaining
            | Self::ElapsedTime => QuantityFormat::COUNT.unit("ticks"),
            Self::Credit | Self::Value => QuantityFormat::COUNT.unit("credits"),
            Self::Energy
            | Self::SpentEnergy
            | Self::ChargingEnergy
            | Self::ChargeSupply
            | Self::Material
            | Self::MaterialSpent
            | Self::ReclaimingMaterial
            | Self::EmbodiedMaterial
            | Self::RecyclingLoss
            | Self::Waste
            | Self::RecycledWaste
            | Self::RepairSupply
            | Self::SpentRepairSupply => QuantityFormat::COUNT.unit("units"),
            _ => QuantityFormat::COUNT,
        }
    }
}

impl StudioLabel for Policy {
    fn studio_label(&self) -> String {
        match self {
            Self::Sprinter => "Sprinter",
            Self::Steward => "Steward",
            Self::ValueHunter => "Value hunter",
            Self::Resilient => "Resilient",
        }
        .into()
    }
}

/// A facility named inside a sentence.
fn facility(facility: Facility) -> &'static str {
    match facility {
        Facility::Workshop => "workshop",
        Facility::Charger => "charger",
        Facility::Recycler => "recycler",
    }
}

/// A work mode named inside a sentence.
fn mode(mode: work_league::WorkMode) -> &'static str {
    match mode {
        work_league::WorkMode::Rush => "rush",
        work_league::WorkMode::Lean => "lean",
        work_league::WorkMode::Safe => "safe",
    }
}

/// A job attempt's outcome named inside a sentence.
fn outcome(outcome: work_league::Outcome) -> &'static str {
    match outcome {
        work_league::Outcome::Success => "success",
        work_league::Outcome::Failure => "failure",
    }
}

impl StudioLabel for work_league::AccountId {
    fn studio_label(&self) -> String {
        match self {
            Self::Agent(agent) => format!("Worker {agent:?}"),
            Self::Job(job) => format!("Job {}", job.0),
            Self::Facility(facility) => format!("{facility:?} facility"),
            Self::Nature => "Disruption model".into(),
            Self::World => "Shared work clock and contracts".into(),
        }
    }
}

impl StudioLabel for work_league::Asset {
    fn studio_label(&self) -> String {
        match self {
            Self::WorldIdentity => "Identity: work economy".into(),
            Self::SeasonActive => "Season active".into(),
            Self::SeasonEnded => "Season ended".into(),
            Self::Tick => "World time".into(),
            Self::Horizon => "Season horizon".into(),
            Self::ContractsOpen => "Unresolved contracts".into(),
            Self::ContractsClosed => "Resolved contracts".into(),
            Self::Activities => "Activities in progress".into(),
            Self::Permit(agent) => format!("{agent:?} may decide this tick"),
            Self::Acted(agent) => format!("{agent:?} has decided this tick"),
            Self::Idle => "Ready for work".into(),
            Self::Busy => "Activity in progress".into(),
            Self::FreeContract => "Can accept a contract".into(),
            Self::ReadyAt => "Activity finishes at".into(),
            Self::ReleaseAt => "Contract opens at".into(),
            Self::Deadline => "Delivery deadline".into(),
            Self::LeaseUntil => "Claim reserved through".into(),
            Self::Expired => "Expired without delivery".into(),
            Self::Transferable => "Handoff available".into(),
            Self::HandedOff => "Contract handed off".into(),
            Self::Assists => "Paid handoffs".into(),
            Self::CompletedJob(job) => format!("Dependency completed: job {}", job.0),
            Self::Credit => "Spendable credit".into(),
            Self::Wear => "Equipment wear".into(),
            Self::Repairing => "Under repair".into(),
            Self::FacilityIdentity(facility) => format!("Identity: {facility:?}"),
            Self::Vacant(facility) => format!("{facility:?} service slot free"),
            Self::Occupied(facility, agent) => format!("{facility:?} occupied by {agent:?}"),
            Self::Servicing(place) => format!("Receiving {} service", facility(*place)),
            Self::ChargingEnergy => "Energy reserved for charging".into(),
            Self::ReclaimingMaterial => "Material being reclaimed".into(),
            Self::EmbodiedMaterial => "Material incorporated into work".into(),
            Self::RecyclingLoss => "Material lost during recycling".into(),
            Self::AgentIdentity(agent) => format!("Identity: {agent:?}"),
            Self::JobIdentity(job) => format!("Identity: job {}", job.0),
            Self::Policy(policy) => format!("{} policy", policy.studio_label()),
            Self::At(location) => format!("At {location:?}"),
            Self::Operational => "Operational".into(),
            Self::Energy => "Energy left".into(),
            Self::TimeRemaining => "Time left".into(),
            Self::Material => "Material left".into(),
            Self::Available => "Available for claim".into(),
            Self::Assigned(agent) => format!("Assigned to {agent:?}"),
            Self::Claimed(job) => format!("Claim on job {}", job.0),
            Self::Pending => "Waiting for work".into(),
            Self::InProgress => "Work in progress".into(),
            Self::Awaiting(job, work) => {
                format!("Job {} {} work awaiting its outcome", job.0, mode(*work))
            }
            Self::Resolved(job, work, work_league::Outcome::Success) => {
                format!("Job {} {} work succeeded", job.0, mode(*work))
            }
            Self::Resolved(job, work, work_league::Outcome::Failure) => {
                format!("Job {} {} work failed", job.0, mode(*work))
            }
            Self::Completed => "Jobs completed".into(),
            Self::Value => "Contract value earned".into(),
            Self::Attempts => "Work attempts".into(),
            Self::Successes => "Successful attempts".into(),
            Self::Failures => "Failed attempts".into(),
            Self::SpentEnergy => "Energy spent".into(),
            Self::ElapsedTime => "Elapsed time".into(),
            Self::MaterialSpent => "Material spent".into(),
            Self::Waste => "Residual waste".into(),
            Self::RecycledWaste => "Waste recycled".into(),
            Self::Damage => "Damage requiring repair".into(),
            Self::RepairSupply => "Repair supply".into(),
            Self::SpentRepairSupply => "Repair supply used".into(),
            Self::ChargeSupply => "Charging energy supply".into(),
            Self::RecyclerCapacity => "Recycler available".into(),
            Self::OutcomeWeight(job, work, result) => {
                format!("Job {} {} {} weight", job.0, mode(*work), outcome(*result))
            }
        }
    }
}

impl StudioLabel for work_league::RateId {
    fn studio_label(&self) -> String {
        match self {
            Self::Tick => "Advance the shared work clock".into(),
            Self::Wait { agent } => format!("{agent:?} waits this tick"),
            Self::BeginService {
                agent,
                facility: place,
                maintenance,
            } => format!(
                "{agent:?} starts {} at the {}",
                if *maintenance {
                    "preventive maintenance"
                } else {
                    "service"
                },
                facility(*place)
            ),
            Self::EndService {
                agent,
                facility: place,
            } => {
                format!(
                    "{agent:?} finishes service and frees the {}",
                    facility(*place)
                )
            }
            Self::Handoff {
                agent,
                recipient,
                job,
                ..
            } => format!(
                "{agent:?} hands job {} to {recipient:?} for 2 credits",
                job.0
            ),
            Self::ReleaseClaim {
                agent,
                job,
                expired: true,
            } => format!("{agent:?}'s claim on job {} lapses", job.0),
            Self::ReleaseClaim {
                agent,
                job,
                expired: false,
            } => format!("{agent:?} releases job {} for another worker", job.0),
            Self::Expire {
                job, agent: None, ..
            } => format!("Job {} misses its deadline unclaimed", job.0),
            Self::Expire {
                job,
                agent: Some(agent),
                mode: None,
            } => format!("Job {} misses its deadline while {agent:?} holds it", job.0),
            Self::Expire {
                job,
                agent: Some(agent),
                mode: Some(work),
            } => format!(
                "Job {} misses its deadline during {agent:?}'s {} work",
                job.0,
                mode(*work)
            ),
            Self::EndSeason => "Close the resolved work season".into(),
            Self::Claim { agent, job } => format!("{agent:?} claims job {}", job.0),
            Self::Move { agent, from, to } => {
                format!("{agent:?} moves from {from:?} to {to:?}")
            }
            Self::Begin {
                agent,
                job,
                mode: work,
            } => format!("{agent:?} begins job {} in {} mode", job.0, mode(*work)),
            Self::Resolve {
                agent,
                job,
                mode: work,
                outcome: result,
            } => format!(
                "{agent:?}'s {} work on job {} {}",
                mode(*work),
                job.0,
                match result {
                    work_league::Outcome::Success => "succeeds",
                    work_league::Outcome::Failure => "fails",
                }
            ),
            Self::Finish {
                agent,
                job,
                mode: work,
                outcome: result,
            } => format!(
                "{agent:?} records a {} {} on job {}",
                mode(*work),
                outcome(*result),
                job.0
            ),
            Self::Repair { agent } => format!("Repair {agent:?}"),
            Self::Recharge { agent } => format!("Recharge {agent:?}"),
            Self::Recycle { agent } => format!("{agent:?} recycles one waste unit"),
        }
    }
}

debug_labels!(work_league::Role);
