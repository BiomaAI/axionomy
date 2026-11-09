use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Player, World};
use axionomy_view::{
    GridCellView, ObjectiveDirectionView, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, TelemetryKindView,
};

pub(super) fn build(
    run: &ArtifactKey,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = run.instance;
    let standard = !matches!(profile, InstanceProfile::Micro);
    let initial = if standard {
        connect_four::initial_standard()
    } else {
        connect_four::initial()
    };
    let iterations = match profile {
        InstanceProfile::Micro => run.budget.clamp(8, 64),
        InstanceProfile::Showcase => (run.budget / 8).clamp(8, 32),
        InstanceProfile::Stress => (run.budget / 4).clamp(32, 128),
    } as usize;
    let mut observe = |state: connect_four::GameProgress| {
        let completed = state
            .moves_completed()
            .saturating_mul(state.iterations_per_move())
            .saturating_add(state.iterations_completed());
        let total = state
            .maximum_moves()
            .saturating_mul(state.iterations_per_move());
        progress.emit(
            slug("mcts-game"),
            completed as u64,
            total as u64,
            format!(
                "action {}/{} · {}/{} MCTS iterations · {} nodes",
                state.moves_completed() + 1,
                state.maximum_moves(),
                state.iterations_completed(),
                state.iterations_per_move(),
                state.nodes()
            ),
        )
    };
    let trace = if standard {
        connect_four::play_standard_game_with_progress(
            iterations,
            run.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    } else {
        connect_four::play_game_with_progress(
            iterations,
            run.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    };
    progress.ensure()?;
    let trace = trace.ok_or_else(|| problem_error("connect-four", "MCTS game was interrupted"))?;
    let final_world = initial
        .replayed(&trace)
        .map_err(|error| problem_error("connect-four", error))?;
    let terminal = connect_four::terminal_values(&final_world)
        .ok_or_else(|| problem_error("connect-four", "game did not terminate"))?;
    let mut view = document(DocumentSpec { problem: "connect-four", strategy: "mcts-game", title: "Connect Four · MCTS self-play", description: "Both players use tactical MCTS: immediate wins, forced blocks and double threats guide search. The economy declares the winner.", source_label: "Connect Four" }, &initial, &axionomy::Goal::new(), &trace, vec![
        ObjectiveView { key: slug("red"), label: "Red terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[0]) },
        ObjectiveView { key: slug("yellow"), label: "Yellow terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[1]) },
    ], scene).map_err(|error| problem_error("connect-four", error))?;
    view.telemetry.push(telemetry(
        "vector-valued MCTS",
        false,
        [
            (
                TelemetryKindView::Iteration,
                (iterations * trace.exchanges().len()) as u64,
                "maximum tree iterations".into(),
            ),
            (
                TelemetryKindView::Generated,
                trace.exchanges().len() as u64,
                "played moves".into(),
            ),
        ],
    ));
    if let Some(first) = trace.exchanges().first() {
        let malformed = Exchange::new(*first.rate(), first.units());
        view.proposals.push(proposal(ProposalSpec { id: "move-without-board", label: "Move without board bindings", description: "A drop must name the game, column, landing cell, and result. Without them, gravity has nothing to act on." }, &initial, &malformed));
    }
    let mut documents = vec![view];
    {
        for (strategy, title, opponent) in [
            (
                "center-opponent",
                "Connect Four · against a center-first opponent",
                connect_four::Opponent::CenterFirst,
            ),
            (
                "search-opponent",
                "Connect Four · against a depth-five opponent",
                connect_four::Opponent::AlphaBeta,
            ),
        ] {
            let trace = connect_four::play_opponent_game_from_with_progress(
                initial.clone(),
                iterations,
                run.seed,
                opponent,
                |turn| {
                    progress.emit(
                        slug(strategy),
                        turn as u64,
                        85,
                        format!("{opponent:?} comparison · action {turn}"),
                    )
                },
            );
            progress.ensure()?;
            let trace = trace
                .ok_or_else(|| problem_error("connect-four", "opponent comparison interrupted"))?;
            let final_world = initial
                .replayed(&trace)
                .map_err(|error| problem_error("connect-four", error))?;
            let terminal = connect_four::terminal_values(&final_world)
                .ok_or_else(|| problem_error("connect-four", "comparison did not terminate"))?;
            documents.push(document(DocumentSpec {
                problem: "connect-four", strategy, title,
                description: "Red uses tactical MCTS. Yellow uses the named independent policy on the same empty board. One game is an inspectable matchup, not a win-rate estimate.", source_label: "Connect Four",
            }, &initial, &axionomy::Goal::new(), &trace, vec![
                ObjectiveView { key: slug("red"), label: "Red terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[0]) },
                ObjectiveView { key: slug("yellow"), label: "Yellow terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[1]) },
            ], scene).map_err(|error| problem_error("connect-four", error))?);
        }
    }
    artifact(run, descriptor, documents)
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let evidence = connect_four::tactical_evidence(world);
    let columns = |values: &[u8]| {
        if values.is_empty() {
            "none".to_string()
        } else {
            values
                .iter()
                .map(|column| (column + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let mut cells = Vec::new();
    let mut pieces = Vec::new();
    let (width, height) = connect_four::board_dimensions(world);
    for column in 0..width {
        for row in 0..height {
            let account = AccountId::Cell { column, row };
            let player = if !world
                .balance(&account, &Asset::Piece(Player::Red))
                .is_zero()
            {
                Some(Player::Red)
            } else if !world
                .balance(&account, &Asset::Piece(Player::Yellow))
                .is_zero()
            {
                Some(Player::Yellow)
            } else {
                None
            };
            let y = height - row - 1;
            let account_key = slug(format!(
                "connect-four/account/cell-column-{column}-row-{row}"
            ));
            cells.push(GridCellView {
                x: u32::from(column),
                y: u32::from(y),
                label: player
                    .map_or_else(|| "Open slot".into(), |player| format!("{player:?} piece")),
                classes: vec![slug("connect-slot")],
                account: Some(account_key.clone()),
            });
            if let Some(player) = player {
                pieces.push(link_balance(
                    visual_entity(
                        slug(format!("piece/{player:?}/{column}/{row}")),
                        format!("{player:?}"),
                        SceneGlyphView::Token,
                        SceneAnchorView::GridCell {
                            x: u32::from(column),
                            y: u32::from(y),
                        },
                        SceneEntityRoleView::Occupant,
                        match player {
                            Player::Red => SceneToneView::Danger,
                            Player::Yellow => SceneToneView::Warning,
                        },
                        Some(format!("{player:?}").to_ascii_lowercase()),
                    ),
                    account_key,
                    slug(format!("connect-four/asset/piece-{player:?}")),
                ));
            }
        }
    }
    Some(
        Scene::grid(
            "Board and pieces",
            u32::from(width),
            u32::from(height),
            cells,
        )
        .with_entities(pieces)
        .with_metrics([
            visual_metric(
                slug("turn"),
                "Next player",
                evidence.player.map_or_else(
                    || {
                        if connect_four::terminal_values(world).is_some() {
                            "finished".into()
                        } else {
                            "adjudicating".into()
                        }
                    },
                    |player| format!("{player:?}"),
                ),
                None,
            ),
            visual_metric(
                slug("wins"),
                "Winning columns",
                columns(&evidence.winning_columns),
                None,
            ),
            visual_metric(
                slug("opponent-wins"),
                "Opponent threats",
                columns(&evidence.opponent_winning_columns),
                None,
            ),
            visual_metric(
                slug("forks"),
                "Double-threat moves",
                columns(&evidence.fork_columns),
                None,
            ),
            visual_metric(
                slug("safe"),
                "Safe next columns",
                columns(&evidence.safe_columns),
                None,
            ),
        ]),
    )
}
