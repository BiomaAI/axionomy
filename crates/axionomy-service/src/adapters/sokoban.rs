use super::*;
use axionomy::{Exchange, Quantity, Trace};
use axionomy_problems::sokoban::{self, AccountId, Asset, RateId, Role, World};
use axionomy_view::{GridCellView, TelemetryKindView};

pub(super) fn build(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> Result<RunArtifact, ServiceError> {
    let initial = sokoban::initial();
    let solution = sokoban::solve(&initial)
        .ok_or_else(|| problem_error("sokoban", "puzzle has no solution"))?;
    let mut solved = document(
        DocumentSpec { problem: "sokoban", strategy: "breadth_first", title: "Sokoban · solved pushes", description: "Every move and three-cell push is an atomic exchange across encoded cell accounts.", source_label: "Sokoban" },
        &initial, &sokoban::goal(), solution.trace(), Vec::new(), scene,
    ).map_err(|error| problem_error("sokoban", error))?;
    solved.telemetry.push(telemetry(
        "breadth-first search",
        true,
        [
            (
                TelemetryKindView::Expanded,
                solution.expanded() as u64,
                "states expanded".into(),
            ),
            (
                TelemetryKindView::Generated,
                solution.trace().exchanges().len() as u64,
                "solution exchanges".into(),
            ),
        ],
    ));
    let blocked_push = Exchange::new(
        RateId::Push {
            behind: 1,
            crate_at: 2,
            to: 3,
        },
        Quantity::new(1),
    )
    .bind(Role::Puzzle, AccountId::Success)
    .bind(Role::From, AccountId::Cell(1))
    .bind(Role::Middle, AccountId::Cell(2))
    .bind(Role::To, AccountId::Cell(3));
    solved.proposals.push(proposal("sokoban", ProposalSpec { id: "push-without-crate", label: "Push empty cell", description: "The roles are structurally valid, but the bound middle cell does not hold a crate." }, &initial, &blocked_push));

    let deadlocked = sokoban::deadlocked();
    let mut deadlock = document(
        DocumentSpec { problem: "sokoban", strategy: "deadlock", title: "Sokoban · deadlocked state", description: "A replayable zero-step counterexample: no sequence can move the crate from the corner to the goal.", source_label: "Sokoban" },
        &deadlocked, &sokoban::goal(), &Trace::new(), Vec::new(), scene,
    ).map_err(|error| problem_error("sokoban", error))?;
    deadlock.telemetry.push(telemetry(
        "breadth-first exhaustion",
        true,
        [(
            TelemetryKindView::Message,
            0,
            "no solution reachable".into(),
        )],
    ));
    artifact(
        request,
        descriptor,
        selected_strategy(request, descriptor),
        vec![solved, deadlock],
    )
}

fn scene(_: u64, world: &World) -> Option<Scene> {
    let cells = (0..5)
        .map(|cell| {
            let account = AccountId::Cell(cell);
            let mut classes = Vec::new();
            let label = if !world.balance(&account, &Asset::Player).is_zero() {
                classes.push("player".into());
                "Player"
            } else if !world.balance(&account, &Asset::Crate).is_zero() {
                classes.push("crate".into());
                "Crate"
            } else {
                "Empty"
            };
            if !world.balance(&account, &Asset::GoalCell).is_zero() {
                classes.push("goal".into());
            }
            GridCellView {
                x: u32::from(cell),
                y: 0,
                label: label.into(),
                classes,
            }
        })
        .collect();
    Some(Scene::Grid {
        title: "Encoded cells, player, crate, and goal".into(),
        width: 5,
        height: 1,
        cells,
    })
}
