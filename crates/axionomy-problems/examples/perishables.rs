mod support;

use axionomy_problems::perishables::{self, ClaimIndex, Cohort, DEFAULT_TRANSFER, EffectAgenda};
use tracing::{debug, info};

fn main() {
    support::init(
        "Perishables",
        "Apply event-driven decay to fungible cohort claims under explicit storage and time facts.",
    );
    let source = perishables::initial();
    let initial_index = ClaimIndex::build(&source);
    let initial_agenda = EffectAgenda::build(&source);
    info!(
        accounts = source.accounts().count(),
        rates = source.rate_ids().count(),
        fungible_claims = initial_index.total_claims(),
        claim_balance_entries = initial_index.balance_entries(),
        unique_condition_facts = 2,
        scheduled_cohort_events = initial_agenda.len(),
        next_due = ?initial_agenda.next_due(),
        "encoded economy aggregates units by shared fate"
    );

    let run = perishables::run_outage_scenario(&source, DEFAULT_TRANSFER)
        .expect("the outage scenario is valid");
    info!(
        moved_claims = DEFAULT_TRANSFER,
        ambient_claims = run.claim_index().total(Cohort::Ambient),
        refrigerated_claims = run.claim_index().total(Cohort::Refrigerated),
        claim_balance_entries = run.claim_index().balance_entries(),
        "one scaled exchange moved fungible inventory into cold storage"
    );

    for report in run.effects() {
        info!(
            at = ?report.at(),
            applied = report.applied().len(),
            stale = report.stale().len(),
            applied_rates = ?report.applied(),
            stale_rates = ?report.stale(),
            "due cohort effects assessed by the authoritative economy"
        );
    }

    let replayed = source
        .replayed(run.trace())
        .expect("the accepted effect trace must replay");
    assert!(replayed.matches(&perishables::goal()));
    assert_eq!(replayed.state_key(), run.world().state_key());
    assert_eq!(run.claim_index().total_claims(), 10_000);
    assert_eq!(run.claim_index().usable_total(run.world()), 0);

    info!(
        exchanges = run.trace().exchanges().len(),
        fungible_claims = run.claim_index().total_claims(),
        usable_claims = run.claim_index().usable_total(run.world()),
        stale_cold_events = run
            .effects()
            .iter()
            .map(|report| report.stale().len())
            .sum::<usize>(),
        goal_verified = true,
        replay_verified = true,
        "power loss changed one shared cohort fact without rewriting its four thousand claims"
    );
    debug!(trace = ?run.trace().exchanges(), "accepted temporal exchange trace");

    let pareto = perishables::storage_plan_front(&source).expect("objective schema is valid");
    info!(
        completeness = ?pareto.front().completeness(),
        plans = pareto.front().len(),
        batch_size = DEFAULT_TRANSFER,
        "exact bounded policy search exposed preservation versus cooling energy"
    );
    for entry in pareto.front().entries() {
        let outcome = source
            .replayed(entry.payload())
            .expect("Pareto storage plan must replay");
        info!(
            usable_inventory = perishables::usable_inventory(&outcome),
            cooling_energy = perishables::spent_cooling_energy(&outcome),
            replay_verified = true,
            "non-dominated storage commitment"
        );
    }
}
