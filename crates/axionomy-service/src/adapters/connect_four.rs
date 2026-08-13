use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Player, World};
use axionomy_view::{
    GridCellView, ObjectiveDirectionView, SceneAnchorView, SceneEntityRoleView, SceneGlyphView,
    SceneToneView, TelemetryKindView,
};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    progress: &mut ProgressSink<'_>,
) -> Result<RunArtifact, ServiceError> {
    let profile = instance_profile(request, descriptor);
    let standard = !matches!(profile, InstanceProfile::Micro);
    let initial = if standard {
        connect_four::initial_standard()
    } else {
        connect_four::initial()
    };
    let iterations = match profile {
        InstanceProfile::Micro => request.budget.clamp(8, 64),
        InstanceProfile::Showcase => (request.budget / 8).clamp(8, 32),
        InstanceProfile::Stress => (request.budget / 4).clamp(32, 128),
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
            "mcts_game",
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
            request.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    } else {
        connect_four::play_game_with_progress(
            iterations,
            request.seed,
            iterations.clamp(1, 8),
            &mut observe,
        )
    };
    progress.ensure()?;
    let trace = trace.ok_or_else(|| problem_error("connect_four", "MCTS game was interrupted"))?;
    let final_world = initial
        .replayed(&trace)
        .map_err(|error| problem_error("connect_four", error))?;
    let terminal = connect_four::terminal_values(&final_world)
        .ok_or_else(|| problem_error("connect_four", "game did not terminate"))?;
    let mut view = document(DocumentSpec { problem: "connect_four", strategy: "mcts_game", title: "Connect Four · MCTS self-play", description: "Both players use MCTS, each maximising its own score, until the rules declare a winner or a draw.", source_label: "Connect Four" }, &initial, &axionomy::Goal::new(), &trace, vec![
        ObjectiveView { key: "red".into(), label: "Red terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[0]) },
        ObjectiveView { key: "yellow".into(), label: "Yellow terminal value".into(), direction: ObjectiveDirectionView::Maximize, value: format!("{:.1}", terminal[1]) },
    ], scene).map_err(|error| problem_error("connect_four", error))?;
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
        let malformed = Exchange::new(*first.rate(), *first.units());
        view.proposals.push(proposal("connect_four", ProposalSpec { id: "move-without-board", label: "Move without board bindings", description: "A drop must name the game, column, landing cell, and result. Without them, gravity has nothing to act on." }, &initial, &malformed));
    }
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        vec![view],
    )
}

fn scene(_: u64, world: &World) -> Option<Scene> {
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
            let account_key = format!("connect_four:account:cell-column-{column}-row-{row}");
            cells.push(GridCellView {
                x: u32::from(column),
                y: u32::from(y),
                label: player
                    .map_or_else(|| "Open slot".into(), |player| format!("{player:?} piece")),
                classes: vec!["connect-slot".into()],
                account: Some(account_key.clone()),
            });
            if let Some(player) = player {
                pieces.push(link_balance(
                    visual_entity(
                        format!("piece:{player:?}:{column}:{row}").to_ascii_lowercase(),
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
                    format!("connect_four:asset:piece-{player:?}").to_ascii_lowercase(),
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
        .with_entities(pieces),
    )
}
