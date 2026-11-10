use super::*;
use axionomy::{Exchange, Quantity, Trace};
use axionomy_problems::scheduling::{
    self, AccountId, Asset, Job, Machine, ObjectiveKey, Operation, World,
};
use axionomy_view::{
    ObjectiveAxisView, ObjectiveDirectionView, ParetoFrontView, TelemetryKindView,
    TimelineLaneView, TimelineSpanView, ViewId,
};

pub(super) fn build(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<Study, ServiceError> {
    let profile = study.instance;
    let initial = match profile {
        InstanceProfile::Micro => scheduling::initial(),
        InstanceProfile::Showcase => scheduling::initial_showcase(),
        InstanceProfile::Stress => scheduling::initial_stress(),
    };
    let best = scheduling::solve_best_first(&initial)
        .ok_or_else(|| problem_error(&study.problem, "best-first found no schedule"))?;
    let bounded = scheduling::branch_optimize(&initial)
        .ok_or_else(|| problem_error(&study.problem, "optimizer found no schedule"))?;
    let pareto =
        scheduling::pareto_front(&initial).map_err(|error| problem_error(&study.problem, error))?;
    let one = frontier_trace(
        &pareto,
        (
            ObjectiveKey::JobOneCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        (
            ObjectiveKey::JobTwoCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        &study.problem,
    )?;
    let two = frontier_trace(
        &pareto,
        (
            ObjectiveKey::JobTwoCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        (
            ObjectiveKey::JobOneCompletion,
            ObjectiveDirectionView::Minimize,
        ),
        &study.problem,
    )?;
    let traces = [
        (
            path!("best-first"),
            "Scheduling · shortest makespan",
            "A generic search for the earliest overall finish time.",
            best.trace().clone(),
            "best-first search",
            Some(best.expanded() as u64),
        ),
        (
            path!("bounded-optimizer"),
            "Scheduling · branch optimizer",
            "Branch-and-bound proposes a schedule; the economy replays every step before accepting it.",
            bounded.trace().clone(),
            "bounded branch optimization",
            None,
        ),
        (
            path!("pareto-job-one"),
            "Scheduling Pareto · Job One first",
            "The frontier schedule that gets Job One done soonest — at Job Two's expense.",
            one,
            "exact Pareto search",
            Some(pareto.progress().expanded() as u64),
        ),
        (
            path!("pareto-job-two"),
            "Scheduling Pareto · Job Two first",
            "The frontier schedule that gets Job Two done soonest — at Job One's expense.",
            two,
            "exact Pareto search",
            Some(pareto.progress().expanded() as u64),
        ),
    ];
    let mut documents = Vec::new();
    for (strategy, title, description, trace, algorithm, expanded) in traces {
        let final_world = initial
            .replayed(&trace)
            .map_err(|error| problem_error(&study.problem, error))?;
        let mut view = document(
            progress,
            DocumentSpec {
                problem: &study.problem,
                document: strategy,
                title,
                description,
                source_label: "Job-shop scheduling",
            },
            &initial,
            &scheduling::goal(),
            &trace,
            objectives(&final_world),
            scene,
        )
        .map_err(|error| problem_error(&study.problem, error))?;
        view.pareto_fronts.push(front_view(&pareto, &view));
        view.telemetry.push(telemetry(
            algorithm,
            true,
            expanded
                .into_iter()
                .map(|value| {
                    (
                        TelemetryKindView::Expanded,
                        value,
                        "branches and states explored".into(),
                    )
                })
                .chain([(
                    TelemetryKindView::Generated,
                    trace.exchanges().len() as u64,
                    "scheduled transitions".into(),
                )]),
        ));
        if let Some(candidate) = scheduling::candidates(&initial).first() {
            let malformed = Exchange::new(*candidate.rate(), Quantity::new(1));
            view.proposals.push(proposal(ProposalSpec { id: path!("unbound-operation"), label: "Schedule without slots", description: "The operation exists, but no job, slot, or schedule was named for it." }, &initial, &malformed));
        }
        documents.push(view);
    }
    let impossible = if matches!(profile, InstanceProfile::Showcase | InstanceProfile::Stress) {
        scheduling::impossible_showcase()
    } else {
        scheduling::impossible()
    };
    let mut impossible_view = document(progress, DocumentSpec { problem: &study.problem, document: path!("infeasible-horizon"), title: "Scheduling · not enough time slots", description: "With only two slots the work cannot fit. That is an answer you can inspect, not an error the optimizer swallowed.", source_label: "Job-shop scheduling" }, &impossible, &scheduling::goal(), &Trace::new(), Vec::new(), scene).map_err(|error| problem_error(&study.problem, error))?;
    impossible_view.telemetry.push(telemetry(
        "exhaustive feasibility",
        true,
        [(TelemetryKindView::Message, 0, "no complete schedule".into())],
    ));
    documents.push(impossible_view);
    assemble(study, descriptor, documents)
}

fn objectives(world: &World) -> Vec<ObjectiveView> {
    vec![
        ObjectiveView::new(
            path!("job-one"),
            "Job One completion",
            ObjectiveDirectionView::Minimize,
            scheduling::completion_time(world, Job::One),
        ),
        ObjectiveView::new(
            path!("job-two"),
            "Job Two completion",
            ObjectiveDirectionView::Minimize,
            scheduling::completion_time(world, Job::Two),
        ),
    ]
}
fn front_view(result: &scheduling::ParetoResult, selected: &ViewDocument) -> ParetoFrontView {
    exact_front(
        "Job One finish vs. Job Two finish",
        vec![
            ObjectiveAxisView {
                key: path!("job-one"),
                label: "Job One".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
            ObjectiveAxisView {
                key: path!("job-two"),
                label: "Job Two".into(),
                direction: ObjectiveDirectionView::Minimize,
            },
        ],
        selected,
        result.front().entries().iter().map(exact_values),
        |values| format!("Job One {} · Job Two {}", values[0], values[1]),
    )
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    if scheduling::execution::enabled(world) {
        return Some(execution_scene(world));
    }
    let lanes = [Machine::One, Machine::Two, Machine::Three]
        .into_iter()
        .map(|machine| TimelineLaneView {
            id: ViewId::new(path!("machine").then(&machine), machine_name(machine)),
            classes: Vec::new(),
        })
        .collect();
    let mut spans = Vec::new();
    for machine in [Machine::One, Machine::Two, Machine::Three] {
        for operation in [
            Operation::OneA,
            Operation::OneB,
            Operation::OneC,
            Operation::TwoA,
            Operation::TwoB,
            Operation::TwoC,
        ] {
            let occupied = (0..12)
                .filter(|time| {
                    !world
                        .balance(
                            &AccountId::Slot(machine, *time),
                            &Asset::Reserved(operation),
                        )
                        .is_zero()
                })
                .collect::<Vec<_>>();
            if let (Some(start), Some(end)) = (occupied.first(), occupied.last()) {
                let job = if matches!(
                    operation,
                    Operation::OneA | Operation::OneB | Operation::OneC
                ) {
                    Job::One
                } else {
                    Job::Two
                };
                spans.push(TimelineSpanView {
                    id: path::write(&machine).then(&operation),
                    lane: path!("machine").then(&machine),
                    start: u64::from(*start),
                    end: u64::from(*end + 1),
                    label: operation_name(operation),
                    classes: vec![path!("job").then(&job)],
                });
            }
        }
    }
    let operations = [
        Operation::OneA,
        Operation::OneB,
        Operation::OneC,
        Operation::TwoA,
        Operation::TwoB,
        Operation::TwoC,
    ];
    let pending = operations.into_iter().filter(|operation| {
        ![Machine::One, Machine::Two, Machine::Three]
            .into_iter()
            .flat_map(|machine| (0..12).map(move |time| AccountId::Slot(machine, time)))
            .any(|account| {
                !world
                    .balance(&account, &Asset::Reserved(*operation))
                    .is_zero()
            })
    });
    let pending_entities = pending.clone().map(|operation| {
        visual_entity(
            path!("pending").then(&operation),
            operation_name(operation),
            SceneGlyphView::Task,
            SceneAnchorView::Unanchored,
            SceneEntityRoleView::Attachment,
            SceneToneView::Warning,
            Some("ready queue".into()),
        )
    });
    let pending_count = pending.count();
    Some(
        Scene::timeline(
            "Machine timeline",
            lanes,
            spans,
            Some(scheduling::encoded_makespan(world)),
        )
        .with_entities(pending_entities)
        .with_metrics([
            visual_metric(path!("pending"), "Pending operations", pending_count, None),
            visual_metric(
                path!("scheduled"),
                "Scheduled operations",
                operations.len() - pending_count,
                None,
            ),
            visual_metric(
                path!("makespan"),
                "Current makespan",
                scheduling::encoded_makespan(world),
                Some("ticks"),
            ),
        ]),
    )
}

fn execution_scene(world: &World) -> Scene {
    use scheduling::execution::{MACHINES, OPERATIONS};
    let now = scheduling::execution::now(world);
    let value = |account, asset| world.balance(&account, &asset).to_u64();
    let lanes = MACHINES
        .into_iter()
        .map(|machine| TimelineLaneView {
            id: ViewId::new(path!("machine").then(&machine), machine_name(machine)),
            classes: if value(AccountId::Machine(machine), Asset::Broken) > 0 {
                vec![path!("blocked")]
            } else {
                vec![]
            },
        })
        .collect();
    let mut spans = Vec::new();
    let mut entities = Vec::new();
    for operation in OPERATIONS {
        let job = if matches!(
            operation,
            Operation::OneA | Operation::OneB | Operation::OneC
        ) {
            Job::One
        } else {
            Job::Two
        };
        let account = AccountId::Job(job);
        let started = value(AccountId::Success, Asset::Started(operation)) > 0;
        let completed_at = value(AccountId::Success, Asset::EndTime(operation));
        let queued = MACHINES
            .into_iter()
            .find(|machine| value(account, Asset::Queued(operation, *machine)) > 0);
        let running = MACHINES
            .into_iter()
            .find(|machine| value(account, Asset::Running(operation, *machine)) > 0);
        let paused = MACHINES
            .into_iter()
            .find(|machine| value(account, Asset::Paused(operation, *machine)) > 0);
        let executed = MACHINES
            .into_iter()
            .find(|machine| value(AccountId::Success, Asset::ExecutedOn(operation, *machine)) > 0);
        let status = if completed_at > 0 {
            format!("completed at {completed_at}")
        } else if paused.is_some() {
            format!(
                "paused · {} ticks remain",
                value(account, Asset::WorkRemaining)
            )
        } else if running.is_some() {
            format!("running until {}", value(account, Asset::Due))
        } else if queued.is_some() {
            "reserved in machine queue".into()
        } else if now < value(account, Asset::Arrival) {
            format!("job arrives at {}", value(account, Asset::Arrival))
        } else if value(account, Asset::Ready(operation)) > 0 {
            "ready for machine choice".into()
        } else {
            "predecessor unfinished".into()
        };
        if let Some(machine) = executed.filter(|_| started) {
            let start = value(AccountId::Success, Asset::StartTime(operation));
            let end = if completed_at > 0 {
                completed_at
            } else if running.is_some() {
                value(account, Asset::Due)
            } else {
                now.max(start + 1)
            };
            let interrupted = value(AccountId::Success, Asset::Interrupted(operation)) > 0;
            spans.push(TimelineSpanView {
                id: path!("operation").then(&operation),
                lane: path!("machine").then(&machine),
                start,
                end,
                label: format!(
                    "{}{}",
                    operation_name(operation),
                    if interrupted { " · interrupted" } else { "" }
                ),
                classes: vec![
                    path!("job").then(&job),
                    if completed_at > 0 {
                        path!("completed")
                    } else {
                        path!("active")
                    },
                ],
            });
        }
        entities.push(link_account::<scheduling::Ontology>(
            visual_entity(
                path!("operation").then(&operation),
                operation_name(operation),
                SceneGlyphView::Task,
                queued.or(running).or(paused).or(executed).map_or(
                    SceneAnchorView::Unanchored,
                    |machine| SceneAnchorView::Timeline {
                        lane: path!("machine").then(&machine),
                        at: now,
                    },
                ),
                SceneEntityRoleView::Attachment,
                if completed_at > 0 {
                    SceneToneView::Success
                } else if paused.is_some() {
                    SceneToneView::Warning
                } else {
                    SceneToneView::Active
                },
                Some(status),
            ),
            &account,
        ));
    }
    let machine_two = AccountId::Machine(Machine::Two);
    let condition = if value(machine_two, Asset::Broken) > 0 {
        "broken"
    } else if value(machine_two, Asset::Repairing) > 0 {
        "repair underway"
    } else {
        "online"
    };
    let mut timeline = Scene::timeline(
        "Executed work, setup and machine occupancy",
        lanes,
        spans,
        Some(now),
    );
    timeline.entities.clear();
    timeline.with_entities(entities).with_metrics([
        asset_metric(path!("world-time"), "World time", &Asset::Now, now),
        asset_metric(
            path!("labor"),
            "Available crew",
            &Asset::Labor,
            value(AccountId::Success, Asset::Labor),
        ),
        asset_metric(
            path!("busy-labor"),
            "Occupied crew",
            &Asset::BusyLabor,
            value(AccountId::Success, Asset::BusyLabor),
        ),
        visual_metric(path!("machine-two"), "Machine Two", condition, None),
        asset_metric(
            path!("repair-kits"),
            "Repair kits left",
            &Asset::RepairKit,
            value(AccountId::Success, Asset::RepairKit),
        ),
        asset_metric(
            path!("one-done"),
            "Job One completion",
            &Asset::CompletionTime,
            scheduling::completion_time(world, Job::One),
        ),
        asset_metric(
            path!("two-done"),
            "Job Two completion",
            &Asset::CompletionTime,
            scheduling::completion_time(world, Job::Two),
        ),
    ])
}

// How Studio names and writes this problem's values.

impl StudioFormat for scheduling::Asset {
    fn studio_format(&self) -> QuantityFormat {
        match self {
            Self::Arrival
            | Self::Due
            | Self::WorkRemaining
            | Self::CompletionTime
            | Self::Now
            | Self::TimeRemaining
            | Self::FaultAt
            | Self::RepairDuration
            | Self::StartTime(_)
            | Self::EndTime(_) => QuantityFormat::COUNT.unit("ticks"),
            _ => QuantityFormat::COUNT,
        }
    }
}

fn job_name(job: Job) -> &'static str {
    match job {
        Job::One => "Job One",
        Job::Two => "Job Two",
    }
}

fn job_of(operation: Operation) -> Job {
    match operation {
        Operation::OneA | Operation::OneB | Operation::OneC => Job::One,
        Operation::TwoA | Operation::TwoB | Operation::TwoC => Job::Two,
    }
}

/// An operation as its job's step: `Job One step A`.
fn operation_name(operation: Operation) -> String {
    let step = match operation {
        Operation::OneA | Operation::TwoA => "A",
        Operation::OneB | Operation::TwoB => "B",
        Operation::OneC | Operation::TwoC => "C",
    };
    format!("{} step {step}", job_name(job_of(operation)))
}

fn machine_name(machine: Machine) -> &'static str {
    match machine {
        Machine::One => "Machine One",
        Machine::Two => "Machine Two",
        Machine::Three => "Machine Three",
    }
}

impl StudioLabel for scheduling::AccountId {
    fn studio_label(&self) -> String {
        match *self {
            Self::Job(value) => job_name(value).into(),
            Self::Slot(value, slot) => format!("{} at slot {slot}", machine_name(value)),
            Self::Machine(value) => machine_name(value).into(),
            Self::Success => "Schedule".into(),
        }
    }
}

impl StudioLabel for scheduling::Asset {
    fn studio_label(&self) -> String {
        match *self {
            Self::JobIdentity(value) => format!("This is {}", job_name(value)),
            Self::SlotIdentity(value, slot) => {
                format!("This is {} at slot {slot}", machine_name(value))
            }
            Self::ReadyAt(op, slot) => format!("{} ready at slot {slot}", operation_name(op)),
            Self::CompletedAt(op, slot) => {
                format!("{} completed at slot {slot}", operation_name(op))
            }
            Self::Available => "Machine slot available".into(),
            Self::Reserved(op) => format!("Reserved for {}", operation_name(op)),
            Self::Makespan(slot) => format!("Overall finish at slot {slot}"),
            Self::Active => "Schedule in progress".into(),
            Self::Solved => "Schedule complete".into(),
            Self::MachineIdentity(value) => format!("This is {}", machine_name(value)),
            Self::Ready(op) => format!("{} ready for routing", operation_name(op)),
            Self::Queued(op, value) => {
                format!("{} queued on {}", operation_name(op), machine_name(value))
            }
            Self::Running(op, value) => {
                format!("{} running on {}", operation_name(op), machine_name(value))
            }
            Self::Paused(op, value) => {
                format!("{} paused on {}", operation_name(op), machine_name(value))
            }
            Self::QueueFree => "Queue space".into(),
            Self::QueueTicket(op) => format!("Queue reservation for {}", operation_name(op)),
            Self::Idle => "Machine idle".into(),
            Self::Busy => "Machine occupied".into(),
            Self::Online => "Machine online".into(),
            Self::Broken => "Machine broken".into(),
            Self::Repairing => "Repair underway".into(),
            Self::Configured(value) => format!("Set up for {}", job_name(value)),
            Self::Labor => "Available crew".into(),
            Self::BusyLabor => "Occupied crew".into(),
            Self::RepairKit => "Repair kit".into(),
            Self::UsedRepairKit => "Repair kit used".into(),
            Self::Arrival => "Job release time".into(),
            Self::Due => "Activity completes at".into(),
            Self::WorkRemaining => "Interrupted work remaining".into(),
            Self::Done => "Job completed".into(),
            Self::CompletionTime => "Actual completion time".into(),
            Self::Now => "World time".into(),
            Self::TimeRemaining => "Workday remaining".into(),
            Self::FaultAt => "Scheduled disruption time".into(),
            Self::FaultPending => "Disruption pending".into(),
            Self::FaultOccurred => "Disruption occurred".into(),
            Self::RepairDuration => "Repair duration".into(),
            Self::NotStarted(op) => format!("{} has not started", operation_name(op)),
            Self::Started(op) => format!("{} started", operation_name(op)),
            Self::StartTime(op) => format!("{} start time", operation_name(op)),
            Self::EndTime(op) => format!("{} completion time", operation_name(op)),
            Self::ExecutedOn(op, value) => {
                format!("{} uses {}", operation_name(op), machine_name(value))
            }
            Self::Interrupted(op) => format!("{} was interrupted", operation_name(op)),
        }
    }
}

impl StudioLabel for scheduling::RateId {
    fn studio_label(&self) -> String {
        match *self {
            Self::Schedule {
                operation: op,
                ready,
                start,
            } => format!(
                "Run {} from slot {start}, ready at slot {ready}",
                operation_name(op)
            ),
            Self::Finish {
                job_one_end,
                job_two_end,
                makespan,
            } => format!(
                "Finish with Job One done at slot {job_one_end}, Job Two at slot {job_two_end}, \
                 makespan {makespan}"
            ),
            Self::Queue {
                operation: op,
                machine: value,
            } => format!("Queue {} on {}", operation_name(op), machine_name(value)),
            Self::CancelQueue {
                operation: op,
                machine: value,
            } => format!(
                "Withdraw {} from the {} queue",
                operation_name(op),
                machine_name(value)
            ),
            Self::Start {
                operation: op,
                machine: value,
                configured,
            } => {
                if configured == job_of(op) {
                    format!(
                        "Start {} on {}, already set up",
                        operation_name(op),
                        machine_name(value)
                    )
                } else {
                    format!(
                        "Start {} on {}, switching setup from {}",
                        operation_name(op),
                        machine_name(value),
                        job_name(configured)
                    )
                }
            }
            Self::Complete {
                operation: op,
                machine: value,
            } => format!("Complete {} on {}", operation_name(op), machine_name(value)),
            Self::Resume {
                operation: op,
                machine: value,
            } => format!("Resume {} on {}", operation_name(op), machine_name(value)),
            Self::Fault { operation: op } => match op {
                Some(op) => format!("Machine Two fails during {}", operation_name(op)),
                None => "Machine Two fails".into(),
            },
            Self::Repair => "Begin Machine Two repair".into(),
            Self::CompleteRepair => "Complete Machine Two repair".into(),
            Self::Advance { to } => format!("Advance the workday to tick {to}"),
            Self::Close => "Close the completed workday".into(),
        }
    }
}

impl StudioLabel for scheduling::Role {
    fn studio_label(&self) -> String {
        match self {
            Self::PrimaryJob => "Job".into(),
            Self::SecondaryJob => "Second job".into(),
            Self::Slot0 => "First time slot".into(),
            Self::Slot1 => "Second time slot".into(),
            Self::Schedule => "Schedule".into(),
            Self::Machine => "Machine".into(),
        }
    }
}
