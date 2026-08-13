use super::*;
use axionomy_problems::sokoban::{self, AccountId, Asset, RateId, Solution, World};
use axionomy_search::{
    AStarSession, BfsSession, GraphSearchProgress,
    session::{Continue, WorkBudget},
};
use axionomy_view::{
    GridCellView, SceneAnchorView, SceneEntityRoleView, SceneGlyphView, SceneToneView,
    TelemetryKindView,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = instance_profile(request, descriptor);
    let initial = match profile {
        InstanceProfile::Micro => sokoban::initial(),
        InstanceProfile::Showcase => sokoban::initial_showcase(),
        InstanceProfile::Stress => sokoban::initial_stress(),
    };
    let strategy = selected_strategy(request, descriptor);
    let chunk = usize::try_from(request.budget.clamp(8, 256)).unwrap_or(64);
    let (solution, search) = search(&initial, strategy, chunk, progress)?;
    let strategy_label = if strategy == "a_star" {
        "A* with crate-to-goal lower bound"
    } else {
        "breadth-first search"
    };
    let mut solved = document(
        DocumentSpec {
            problem: "sokoban",
            strategy,
            title: "Sokoban · warehouse solved",
            description: "Stable crates travel through a walled warehouse. Every walk and every three-cell push is replayed as an indivisible economic exchange.",
            source_label: "Sokoban",
        },
        &initial,
        &sokoban::goal(),
        solution.trace(),
        Vec::new(),
        scene,
    )
    .map_err(|error| problem_error("sokoban", error))?;
    solved.telemetry.push(telemetry(
        strategy_label,
        true,
        [
            (
                TelemetryKindView::Expanded,
                search.expanded() as u64,
                "verified economic states expanded".into(),
            ),
            (
                TelemetryKindView::Generated,
                search.generated() as u64,
                "distinct successors generated".into(),
            ),
            (
                TelemetryKindView::Message,
                solution.trace().exchanges().len() as u64,
                "replayable exchanges in solution".into(),
            ),
        ],
    ));

    if let Some(rate) = initial.rate_ids().find(|rate| {
        matches!(rate, RateId::Push { crate_at, crate_id, .. }
            if initial.balance(&AccountId::Cell(*crate_at), &Asset::Crate(*crate_id)).is_zero())
    }) {
        let missing_crate = sokoban::exchange(rate.clone());
        solved.proposals.push(proposal(
            "sokoban",
            ProposalSpec {
                id: "push-without-crate",
                label: "Push an empty floor tile",
                description: "The geometry names three real floor cells, but the middle account does not own the requested crate identity.",
            },
            &initial,
            &missing_crate,
        ));
    }

    let mut documents = vec![solved];
    if let Some(deadlock_trace) = sokoban::losing_trace(&initial) {
        let deadlocked = initial
            .replayed(&deadlock_trace)
            .map_err(|error| problem_error("sokoban", error))?;
        let mut deadlock = document(
            DocumentSpec {
                problem: "sokoban",
                strategy: "deadlock",
                title: "Sokoban · legal plan, losing outcome",
                description: "Starting from the same warehouse, replay a sequence that is valid under every rule yet strands a stable crate in a non-goal corner. Valid is not the same as wise.",
                source_label: "Sokoban",
            },
            &initial,
            &sokoban::goal(),
            &deadlock_trace,
            Vec::new(),
            scene,
        )
        .map_err(|error| problem_error("sokoban", error))?;
        let trapped = sokoban::crate_cells(&deadlocked)
            .into_iter()
            .filter(|(_, cell)| sokoban::is_dead_square(&deadlocked, *cell))
            .count();
        deadlock.telemetry.push(telemetry(
            "structural corner-deadlock proof",
            true,
            [(
                TelemetryKindView::Message,
                trapped as u64,
                "crates trapped on non-goal dead squares after the replay".into(),
            )],
        ));
        documents.push(deadlock);
    }

    artifact(request, descriptor, strategy, documents)
}

fn search(
    initial: &World,
    strategy: &str,
    chunk: usize,
    progress: &mut ProgressSink<'_>,
) -> Result<(Solution, GraphSearchProgress), ServiceError> {
    if strategy == "a_star" {
        let mut session = AStarSession::new(
            initial,
            sokoban::goal(),
            sokoban::candidates,
            |_, _, _| 1,
            sokoban::crate_goal_distance,
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(chunk), &mut observer);
            let state = session.progress();
            let _ = progress.graph(
                "a_star_frontier",
                state,
                format!(
                    "{} expanded · {} frontier · {} distinct states",
                    state.expanded(),
                    state.frontier(),
                    state.visited()
                ),
            );
            progress.ensure()?;
        }
        let state = session.progress();
        let solution = session
            .into_solution()
            .ok_or_else(|| problem_error("sokoban", "puzzle has no solution"))?;
        Ok((solution, state))
    } else {
        let mut session = BfsSession::new(initial, sokoban::goal(), sokoban::candidates);
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(chunk), &mut observer);
            let state = session.progress();
            let _ = progress.graph(
                "breadth_first_frontier",
                state,
                format!(
                    "{} expanded · {} frontier · {} distinct states",
                    state.expanded(),
                    state.frontier(),
                    state.visited()
                ),
            );
            progress.ensure()?;
        }
        let state = session.progress();
        let solution = session
            .into_solution()
            .ok_or_else(|| problem_error("sokoban", "puzzle has no solution"))?;
        Ok((solution, state))
    }
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let (width, height) = sokoban::dimensions(world);
    let crates = sokoban::crate_cells(world);
    let player = sokoban::player_cell(world);
    let mut cells = Vec::with_capacity(usize::from(width * height));
    let mut entities = Vec::with_capacity(crates.len() + usize::from(player.is_some()));

    for cell in 0..width * height {
        let account = AccountId::Cell(cell);
        let account_key = format!("sokoban:account:cell-{cell}");
        let wall = sokoban::is_wall(world, cell);
        let goal = !world.balance(&account, &Asset::GoalCell).is_zero();
        let dead = sokoban::is_dead_square(world, cell);
        let mut classes = vec![if wall { "wall".into() } else { "floor".into() }];
        if goal {
            classes.push("goal".into());
        }
        if dead {
            classes.push("dead-square".into());
        }
        cells.push(GridCellView {
            x: u32::from(cell % width),
            y: u32::from(cell / width),
            label: if wall {
                "Wall".into()
            } else if goal {
                "Goal floor".into()
            } else if dead {
                "Non-goal corner".into()
            } else {
                "Warehouse floor".into()
            },
            classes,
            account: Some(account_key.clone()),
        });

        if player == Some(cell) {
            entities.push(link_balance(
                visual_entity(
                    "player",
                    "Player",
                    SceneGlyphView::Agent,
                    SceneAnchorView::GridCell {
                        x: u32::from(cell % width),
                        y: u32::from(cell / width),
                    },
                    SceneEntityRoleView::Occupant,
                    SceneToneView::Active,
                    Some("player".into()),
                ),
                account_key.clone(),
                "sokoban:asset:player",
            ));
        }
        if let Some((crate_id, _)) = crates.iter().find(|(_, at)| *at == cell) {
            let on_goal = goal;
            let trapped = dead && !on_goal;
            entities.push(link_balance(
                visual_entity(
                    format!("crate:{crate_id}"),
                    format!("Crate {}", crate_id + 1),
                    SceneGlyphView::Package,
                    SceneAnchorView::GridCell {
                        x: u32::from(cell % width),
                        y: u32::from(cell / width),
                    },
                    SceneEntityRoleView::Occupant,
                    if on_goal {
                        SceneToneView::Success
                    } else if trapped {
                        SceneToneView::Danger
                    } else {
                        SceneToneView::Warning
                    },
                    Some(if on_goal {
                        "on-goal".into()
                    } else if trapped {
                        "deadlocked".into()
                    } else {
                        "crate".into()
                    }),
                ),
                account_key,
                format!("sokoban:asset:crate-{crate_id}"),
            ));
        }
    }

    Some(
        Scene::grid(
            "Warehouse floor, stable crates, goals, and dead squares",
            u32::from(width),
            u32::from(height),
            cells,
        )
        .with_entities(entities),
    )
}
