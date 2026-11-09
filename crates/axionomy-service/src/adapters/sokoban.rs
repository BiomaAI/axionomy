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
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = run.instance;
    let initial = match profile {
        InstanceProfile::Micro => sokoban::initial(),
        InstanceProfile::Showcase => sokoban::initial_showcase(),
        InstanceProfile::Stress => sokoban::initial_stress(),
    };
    let strategy = run.strategy.as_str();
    let chunk = usize::try_from(run.budget.clamp(8, 256)).unwrap_or(64);
    let (solution, search) = search(&initial, strategy, chunk, progress)?;
    let strategy_label = if strategy == "a-star" {
        "A* with crate-to-goal lower bound"
    } else {
        "breadth-first search"
    };
    let mut solved = document(
        DocumentSpec {
            problem: "sokoban",
            strategy,
            title: "Sokoban · warehouse solved",
            description: "Crates hold pressure plates to open doors. Cross into the next chamber, latch the door, recover the staged crate, and reach the far exit after completing every goal. Each move and mechanism is an atomic economic exchange.",
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
        solved.proposals.push(proposal(ProposalSpec {
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
                description: "Starting from the same warehouse, replay a sequence that is valid under every rule yet strands a stable crate in a square from which no goal is reachable. Valid is not the same as wise.",
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
            "reverse-push reachability proof",
            true,
            [(
                TelemetryKindView::Message,
                trapped as u64,
                "crates trapped on non-goal dead squares after the replay".into(),
            )],
        ));
        documents.push(deadlock);
    }

    artifact(run, descriptor, documents)
}

fn search(
    initial: &World,
    strategy: &str,
    chunk: usize,
    progress: &mut ProgressSink<'_>,
) -> Result<(Solution, GraphSearchProgress), ServiceError> {
    if strategy == "a-star" {
        let mut session = AStarSession::new(
            initial,
            sokoban::goal(),
            sokoban::search_candidates,
            |_, _, _| 1,
            sokoban::crate_goal_distance,
        );
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(chunk), &mut observer);
            let state = session.progress();
            let _ = progress.graph(
                slug("a-star-frontier"),
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
        let mut session = BfsSession::new(initial, sokoban::goal(), sokoban::search_candidates);
        let mut observer = Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(chunk), &mut observer);
            let state = session.progress();
            let _ = progress.graph(
                slug("breadth-first-frontier"),
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
        let wall = sokoban::is_wall(world, cell);
        let goal = !world.balance(&account, &Asset::GoalCell).is_zero();
        let dead = sokoban::is_dead_square(world, cell);
        let mut classes = vec![if wall { slug("wall") } else { slug("floor") }];
        if goal {
            classes.push(slug("goal"));
        }
        if dead {
            classes.push(slug("dead-square"));
        }
        cells.push(GridCellView {
            x: u32::from(cell % width),
            y: u32::from(cell / width),
            label: if wall {
                "Wall".into()
            } else if goal {
                "Goal floor".into()
            } else if dead {
                "No crate route to any goal".into()
            } else {
                "Warehouse floor".into()
            },
            classes,
            account: Some(account_key::<sokoban::Ontology>(&account)),
        });

        if !world.balance(&account, &Asset::ExitCell).is_zero() {
            entities.push(visual_entity(
                slug("exit"),
                "Exit",
                SceneGlyphView::Goal,
                SceneAnchorView::GridCell {
                    x: u32::from(cell % width),
                    y: u32::from(cell / width),
                },
                SceneEntityRoleView::Structure,
                SceneToneView::Goal,
                Some("leave after all deliveries".into()),
            ));
        }
        for circuit in 0..2 {
            for (asset, name, glyph) in [
                (Asset::Plate(circuit), "Plate", SceneGlyphView::Constraint),
                (Asset::Door(circuit), "Door", SceneGlyphView::Door),
                (
                    Asset::Panel(circuit),
                    "Latch panel",
                    SceneGlyphView::Constraint,
                ),
            ] {
                if world.balance(&account, &asset).is_zero() {
                    continue;
                }
                let pressed = !world
                    .balance(&AccountId::Success, &Asset::Pressed(circuit))
                    .is_zero();
                let latched = !world
                    .balance(&AccountId::Success, &Asset::Latched(circuit))
                    .is_zero();
                let state = match asset {
                    Asset::Plate(_) => {
                        if pressed {
                            "holding door open"
                        } else {
                            "needs a crate"
                        }
                    }
                    Asset::Door(_) => {
                        if latched {
                            "latched open"
                        } else if pressed {
                            "open under pressure"
                        } else {
                            "closed"
                        }
                    }
                    _ => {
                        if latched {
                            "latched"
                        } else {
                            "ready to latch"
                        }
                    }
                };
                entities.push(visual_entity(
                    slug(format!("mechanism/{name}/{circuit}")),
                    format!("{name} {}", circuit + 1),
                    glyph,
                    SceneAnchorView::GridCell {
                        x: u32::from(cell % width),
                        y: u32::from(cell / width),
                    },
                    SceneEntityRoleView::Structure,
                    if latched || pressed {
                        SceneToneView::Success
                    } else {
                        SceneToneView::Warning
                    },
                    Some(state.into()),
                ));
            }
        }
        if player == Some(cell) {
            entities.push(link_balance::<sokoban::Ontology>(
                visual_entity(
                    slug("player"),
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
                &account,
                &Asset::Player,
            ));
        }
        if let Some((crate_id, _)) = crates.iter().find(|(_, at)| *at == cell) {
            let on_goal = goal;
            let trapped = dead && !on_goal;
            entities.push(link_balance::<sokoban::Ontology>(
                visual_entity(
                    slug(format!("crate/{crate_id}")),
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
                &account,
                &Asset::Crate(*crate_id),
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
        .with_entities(entities)
        .with_metrics([
            visual_metric(
                slug("held-plates"),
                "Crates holding plates",
                (0..2)
                    .filter(|circuit| {
                        !world
                            .balance(&AccountId::Success, &Asset::Pressed(*circuit))
                            .is_zero()
                    })
                    .count(),
                None,
            ),
            visual_metric(
                slug("latched"),
                "Doors latched open",
                (0..2)
                    .filter(|circuit| {
                        !world
                            .balance(&AccountId::Success, &Asset::Latched(*circuit))
                            .is_zero()
                    })
                    .count(),
                None,
            ),
            visual_metric(
                slug("goals"),
                "Crates on goals",
                crates
                    .iter()
                    .filter(|(_, at)| {
                        !world
                            .balance(&AccountId::Cell(*at), &Asset::GoalCell)
                            .is_zero()
                    })
                    .count(),
                None,
            ),
        ]),
    )
}
