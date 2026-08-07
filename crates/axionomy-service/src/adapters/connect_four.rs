use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Player, World};
use axionomy_view::{GridCellView, ObjectiveDirectionView, TelemetryKindView};

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
    let mut view = document(DocumentSpec { problem: "connect_four", strategy: "mcts_game", title: "Connect Four · complete MCTS game", description: "Both adversarial players choose vector-valued MCTS actions until encoded winner or draw truth is produced.", source_label: "Connect Four" }, &initial, &axionomy::Goal::new(), &trace, vec![
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
        view.proposals.push(proposal("connect_four", ProposalSpec { id: "move-without-board", label: "Move without board bindings", description: "Gravity and line-count updates require game, column, cell, and result accounts; an unbound move is invalid." }, &initial, &malformed));
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
    let (width, height) = connect_four::board_dimensions(world);
    for column in 0..width {
        for row in 0..height {
            let account = AccountId::Cell { column, row };
            let (label, classes) = if !world
                .balance(&account, &Asset::Piece(Player::Red))
                .is_zero()
            {
                ("Red", vec!["red".into()])
            } else if !world
                .balance(&account, &Asset::Piece(Player::Yellow))
                .is_zero()
            {
                ("Yellow", vec!["yellow".into()])
            } else {
                ("Empty", Vec::new())
            };
            cells.push(GridCellView {
                x: u32::from(column),
                y: u32::from(height - row - 1),
                label: label.into(),
                classes,
            });
        }
    }
    Some(Scene::grid(
        "Encoded board, gravity, and pieces",
        u32::from(width),
        u32::from(height),
        cells,
    ))
}
