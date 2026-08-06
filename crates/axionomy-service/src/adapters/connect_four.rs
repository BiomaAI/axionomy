use super::*;
use axionomy::Exchange;
use axionomy_problems::connect_four::{self, AccountId, Asset, Player, World};
use axionomy_view::{GridCellView, ObjectiveDirectionView, TelemetryKindView};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let initial = connect_four::initial();
    let iterations = request.budget.max(8) as usize;
    let trace = connect_four::play_game(iterations, request.seed);
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
        let malformed = Exchange::new(*first.rate(), first.units().clone());
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
    for column in 0..connect_four::WIDTH {
        for row in 0..connect_four::HEIGHT {
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
                y: u32::from(connect_four::HEIGHT - row - 1),
                label: label.into(),
                classes,
            });
        }
    }
    Some(Scene::Grid {
        title: "Encoded board, gravity, and pieces".into(),
        width: u32::from(connect_four::WIDTH),
        height: u32::from(connect_four::HEIGHT),
        cells,
    })
}
