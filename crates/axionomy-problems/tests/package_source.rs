#![cfg(feature = "packages")]
use axionomy_engine::{Engine, PackagePath, source};
use axionomy_store::Store;
use std::collections::BTreeMap;

/// Each hash covers every named starting economy, its ontology and its
/// ordered exchanges, recorded before replacing the original gallery writers.
#[test]
fn standalone_gallery_sources_reproduce_the_pinned_histories() {
    let pins: BTreeMap<PackagePath, String> =
        serde_json::from_str(include_str!("fixtures/package-histories.json")).unwrap();
    let packages = axionomy_problems::packages();
    assert_eq!(pins.len(), packages.len());
    for (name, write) in packages {
        let installed = Engine::new(Store::memory().unwrap()).unwrap();
        write(&installed).unwrap();
        let files = installed.package_source(&name).unwrap();
        assert_eq!(files, axionomy_problems::source(&name));
        let rebuilt = source::build(&files).unwrap();
        assert!(
            !rebuilt.tests.is_empty(),
            "{name} needs an economic assertion"
        );
        let mut histories = BTreeMap::new();
        for reference in installed.named_under(&name).unwrap().refs {
            let history = |engine: &Engine| {
                let at = engine.referred(&reference).unwrap();
                let (economy, exchanges) = engine.history(at).unwrap();
                serde_json::json!([engine.ontology(at).unwrap(), economy, exchanges])
            };
            let original = history(&installed);
            assert_eq!(original, history(&rebuilt.engine), "{reference}");
            histories.insert(reference, original);
        }
        let digest = blake3::hash(&serde_json::to_vec(&histories).unwrap());
        assert_eq!(digest.to_string(), pins[&name], "{name}");
    }
}

/// Check the deals from buyer cash rather than reading the strategies'
/// scores. The two posted prices are 100 and 90. Only the first seller
/// offers a seven-credit concession.
#[test]
fn marketplace_strategies_reach_the_two_independent_settlement_values() {
    use axionomy::{Quantity, path};
    let built = source::build(&axionomy_problems::source(&path!("marketplace"))).unwrap();
    let results = built
        .engine
        .run_experiment(&path!("marketplace/micro"), &mut |_, _| {
            std::ops::ControlFlow::Continue(())
        })
        .unwrap();
    let expected = BTreeMap::from([
        (path!("marketplace/settlement/market-clearing"), 190),
        (path!("marketplace/settlement/pareto-sellers"), 190),
        (path!("marketplace/settlement/pareto-buyers"), 183),
    ]);
    assert_eq!(results.arms.len(), expected.len());
    for arm in results.arms {
        let paid = Quantity::new(expected[&arm.arm.strategy]);
        assert!(!arm.trials.is_empty());
        for trial in arm.trials {
            let state = &built.engine.reach(trial.end).unwrap().state;
            let remaining = [path!("buyer/a"), path!("buyer/b"), path!("buyer/c")]
                .into_iter()
                .fold(Quantity::ZERO, |total, id| {
                    total
                        .checked_add(state.economy.balance(&id, &path!("money")))
                        .unwrap()
                });
            assert_eq!(
                Quantity::new(365).checked_sub(remaining),
                Some(paid),
                "{}",
                arm.arm.strategy
            );
        }
    }
}
