use axionomy_problems::logistics::{self, Policy};

fn main() {
    let model = logistics::initial();
    let estimate = logistics::monte_carlo(&model, 64).expect("policies can be evaluated");
    let chosen = estimate.chosen();
    let rollout = logistics::run_policy(&model, chosen, 7);
    let replayed = model
        .replayed(rollout.trace())
        .expect("selected mission must replay");

    assert!(rollout.completed());
    assert!(replayed.matches(&logistics::goal()));
    assert_eq!(chosen, Policy::Reliable);

    println!(
        "Logistics: {chosen:?} delivered {} orders in {} encoded time over {} exchanges",
        rollout.delivered(),
        rollout.elapsed_time(),
        rollout.steps(),
    );
}
