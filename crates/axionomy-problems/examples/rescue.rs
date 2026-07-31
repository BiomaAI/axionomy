use axionomy_problems::rescue::{self, Location, Policy};

fn main() {
    let model = rescue::uniform_uncertain();
    let estimate = rescue::monte_carlo(&model, 8).expect("prior has positive weight");
    let sample = rescue::instantiate(&model, Location::South, 1).expect("scenario is encoded");
    let rollout = rescue::run_sampled_policy(&model, &sample, estimate.chosen())
        .expect("scenario can be instantiated");
    let final_world = model
        .replayed(rollout.trace())
        .expect("the sampled policy must pass the core");
    assert!(final_world.matches(&rescue::goal()));
    assert_eq!(estimate.chosen(), Policy::ObserveThenFollow);

    println!(
        "Rescue: Monte Carlo chose {:?} ({} informed wins vs {} direct wins)",
        estimate.chosen(),
        estimate.observe_successes(),
        estimate.direct_successes(),
    );
}
