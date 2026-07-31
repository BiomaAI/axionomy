use axionomy_problems::mission;

fn main() {
    let model = mission::initial();
    let estimate = mission::monte_carlo(&model, 16).expect("mission policies can be evaluated");
    let rollout = mission::run_policy(&model, estimate.chosen(), 3);
    let replayed = model
        .replayed(rollout.trace())
        .expect("coordinated mission must replay");

    assert!(rollout.succeeded());
    assert!(replayed.matches(&mission::goal()));

    println!(
        "Mission: {:?} won {}/{} samples and replayed in {} encoded time",
        estimate.chosen(),
        estimate.coordinated_successes(),
        estimate.samples(),
        rollout.elapsed_time(),
    );
}
