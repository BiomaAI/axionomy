mod support;

use axionomy_problems::work_league::{self, AGENTS, AccountId, Asset, Profile};
use tracing::{debug, info};

fn main() {
    support::init(
        "Work League",
        "Run four competing worker policies and compare several honest definitions of winning.",
    );
    let league = work_league::league(Profile::Showcase, work_league::mixed_lineup());
    info!(
        agents = league.agents().len(),
        jobs = league.jobs().len(),
        accounts = league.initial().accounts().count(),
        rates = league.initial().rate_ids().count(),
        "closed multi-agent economy ready"
    );

    let outcome = work_league::run(&league, 17).expect("the seeded league must complete");
    let world = outcome.final_world();
    assert!(world.matches(league.goal()));
    for agent in AGENTS {
        if world.account(&AccountId::Agent(agent)).is_none() {
            continue;
        }
        let read = |asset| world.balance(&AccountId::Agent(agent), &asset).get();
        info!(
            agent = ?agent,
            policy = ?work_league::policy(world, agent),
            jobs = read(Asset::Completed),
            value = read(Asset::Value),
            elapsed = read(Asset::ElapsedTime),
            energy = read(Asset::SpentEnergy),
            material = read(Asset::MaterialSpent),
            residual_waste = read(Asset::Waste),
            recycled = read(Asset::RecycledWaste),
            successes = read(Asset::Successes),
            attempts = read(Asset::Attempts),
            "final replay-derived agent outcome"
        );
    }
    info!(
        exchanges = outcome.trace().exchanges().len(),
        goal_verified = true,
        "competitive match completed and replay verified"
    );
    debug!(trace = ?outcome.trace().exchanges(), "accepted multi-agent exchange trace");
}
