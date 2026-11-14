//! Scripts through the engine's host API: exact quantities, exchanges they
//! propose, views, seeded randomness, and scripts kept in the store.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, EngineError, Random, ScriptPath, ScriptState, exchanges_of, rhai};
use axionomy_search::sampling::{
    EventTickets, SeededSampler, TicketSource, WeightedExchange, sample, sample_seed, splitmix,
};
use axionomy_store::Store;

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn economy() -> paths::Economy {
    paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 100)]).into())
        .account(path("alice"), basket([(path("cash"), 5)]).into())
        .account(path("bob"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap()
}

const SCRIPT: &str = r#"
    fn total(state) {
        state.balance("bank", "cash") + state.balance("alice", "cash")
    }

    // Pay each account the bank can afford to pay ten, in path order.
    fn candidates(state) {
        let found = [];
        for account in state.accounts("") {
            if account == "bank" { continue; }
            let paying = exchange("pay", #{ from: "bank", to: account }, 10);
            if state.applicable(paying) { found.push(paying); }
        }
        found
    }

    fn hidden(state) {
        state.view(["alice"]).balance("bank", "cash")
    }

    fn peek(state) {
        state.view(["alice"]).applicable(exchange("pay", #{ from: "bank", to: "bob" }, 1))
    }

    fn overdraw() {
        quantity(1) - quantity(2)
    }

    fn draws(random) {
        [random.next(1000), random.next(1000), random.next(1000)]
    }

    fn weighted(random, weights) {
        [random.sample(weights), random.sample(weights), random.sample(weights)]
    }

    // A weighted sample between two draws.
    fn between(random) {
        let first = random.next(1000);
        random.sample([1, 2, 3]);
        [first, random.next(1000)]
    }

    // The world's chance by named event.
    fn event(random, name, bound) { random.next(name, bound) }
    fn weighted_event(random, name, weights) { random.sample(name, weights) }

    // Each of `names`' draws, around draws in sequence.
    fn events(random, names, bound) {
        let drawn = [];
        for name in names {
            drawn.push(random.next(name, bound));
            random.next(1000);
            random.sample([1, 2]);
        }
        drawn
    }

    // The draw of `name` after unrelated ones, by event and in sequence.
    fn after_others(random, name, bound) {
        random.next("weather/elsewhere/3", bound);
        random.next(1000);
        random.sample([1, 2, 3]);
        random.sample("scenario", [1, 1]);
        random.next(name, bound)
    }

    // Draws in sequence around draws by event.
    fn around_events(random) {
        let first = random.next(1000);
        random.next("weather/direct-out/12", 1000);
        random.sample("scenario", [1, 2, 3]);
        [first, random.next(1000)]
    }

    // Tickets keyed by what they are for, around a draw and a sample.
    fn keyed(random, keys, bound) {
        let tickets = [];
        for key in keys {
            tickets.push(random.keyed(key, bound));
            random.next(1000);
            random.sample([1, 2]);
        }
        tickets
    }

    fn road(from, to) {
        let roads = [["base", "north"], ["north", "east"], ["base", "south"]];
        distances(roads, from)[to]
    }

    fn forever() {
        loop { }
    }
"#;

fn engine() -> (Engine, ScriptPath) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let script: ScriptPath = path("bank/policy");
    engine.keep_script(&script, SCRIPT).unwrap();
    (engine, script)
}

#[test]
fn scripts_read_exact_quantities_and_propose_exchanges() {
    let (engine, script) = engine();
    let state = ScriptState::new(economy());
    let total: Quantity = engine.call(&script, "total", (state.clone(),)).unwrap();
    assert_eq!(total, Quantity::new(105));

    let proposed: rhai::Array = engine.call(&script, "candidates", (state,)).unwrap();
    let proposed = exchanges_of(proposed).unwrap();
    let expected: Vec<_> = ["alice", "bob"]
        .into_iter()
        .map(|to| {
            paths::Exchange::new(path("pay"), Quantity::new(10))
                .bind(path("from"), path("bank"))
                .bind(path("to"), path(to))
        })
        .collect();
    assert_eq!(proposed, expected);

    // Arithmetic is exact: a quantity never goes below zero.
    let overdrawn = engine.call::<Quantity>(&script, "overdraw", ());
    assert!(matches!(overdrawn, Err(EngineError::Script { .. })));
}

#[test]
fn a_view_shows_only_its_accounts_and_hides_applicability() {
    let (engine, script) = engine();
    let state = ScriptState::new(economy());
    let hidden: Quantity = engine.call(&script, "hidden", (state.clone(),)).unwrap();
    assert_eq!(hidden, Quantity::ZERO);
    let peek = engine.call::<bool>(&script, "peek", (state,));
    assert!(matches!(peek, Err(EngineError::Script { message, .. }) if message.contains("view")));
}

#[test]
fn randomness_is_the_hosts_and_replays_from_its_seed() {
    let (engine, script) = engine();
    let draws = |seed| -> Vec<i64> {
        let drawn: rhai::Array = engine.call(&script, "draws", (Random::new(seed),)).unwrap();
        drawn
            .into_iter()
            .map(|value| value.as_int().unwrap())
            .collect()
    };
    assert_eq!(draws(7), draws(7));
    assert_ne!(draws(7), draws(8));
    assert!(draws(7).iter().all(|value| (0..1000).contains(value)));
}

#[test]
fn weighted_samples_are_the_rust_samplers() {
    let (engine, script) = engine();
    let ints = |drawn: rhai::Array| -> Vec<i64> {
        drawn
            .into_iter()
            .map(|value| value.as_int().unwrap())
            .collect()
    };
    let weights = [3_u64, 0, 5, 1];
    let outcomes: Vec<_> = weights
        .iter()
        .enumerate()
        .map(|(index, weight)| WeightedExchange::new(index as i64, *weight))
        .collect();
    let as_ints: rhai::Array = weights
        .iter()
        .map(|weight| rhai::Dynamic::from(*weight as i64))
        .collect();
    let as_quantities: rhai::Array = weights
        .iter()
        .map(|weight| rhai::Dynamic::from(Quantity::new(*weight)))
        .collect();
    for seed in [7, 8, sample_seed(17, 3)] {
        // What `axionomy_search` samples from a sampler of the same seed.
        let mut sampler = SeededSampler::new(seed);
        let expected: Vec<i64> = (0..3)
            .map(|_| *sample(&outcomes, &mut sampler).unwrap())
            .collect();
        for weights in [&as_ints, &as_quantities] {
            let drawn: rhai::Array = engine
                .call(&script, "weighted", (Random::new(seed), weights.clone()))
                .unwrap();
            assert_eq!(ints(drawn), expected, "seed {seed}");
        }
    }

    // Samples draw tickets of their own: `next` goes on as if none were.
    let between: rhai::Array = engine.call(&script, "between", (Random::new(7),)).unwrap();
    let drawn: rhai::Array = engine.call(&script, "draws", (Random::new(7),)).unwrap();
    assert_eq!(ints(between), ints(drawn)[..2]);

    // Nothing to weigh, or no weight, samples nothing.
    for weights in [vec![], vec![rhai::Dynamic::from(0_i64)]] {
        let refused = engine.call::<rhai::Array>(&script, "weighted", (Random::new(7), weights));
        assert!(matches!(refused, Err(EngineError::Script { .. })));
    }
}

fn ints(drawn: rhai::Array) -> Vec<i64> {
    drawn
        .into_iter()
        .map(|value| value.as_int().unwrap())
        .collect()
}

#[test]
fn an_event_draws_the_same_whatever_else_was_drawn() {
    let (engine, script) = engine();
    let event = |random: Random, name: &str| -> i64 {
        engine
            .call(&script, "event", (random, name.to_owned(), 1000_i64))
            .unwrap()
    };
    for seed in [7, 17, sample_seed(17, 3)] {
        for name in ["scenario", "weather/direct-out/12", "attempt/3/2"] {
            let drawn = event(Random::new(seed), name);
            // Every generator of the run seeded alike, whatever its own
            // sequence, meets the event alike: the ticket `axionomy_search`
            // draws for the event.
            assert_eq!(event(Random::new(seed), name), drawn);
            assert_eq!(
                event(Random::apart(seed, sample_seed(seed, 1)), name),
                drawn
            );
            let mut tickets = EventTickets::new(seed, &path(name));
            assert_eq!(drawn, tickets.ticket(1000) as i64, "{name}");
            // A branch that draws something else first meets the same.
            let after: i64 = engine
                .call(
                    &script,
                    "after_others",
                    (Random::new(seed), name.to_owned(), 1000_i64),
                )
                .unwrap();
            assert_eq!(after, drawn, "seed {seed}: {name}");
        }
    }

    // A generator that has drawn in sequence before meets the event alike.
    let random = Random::new(7);
    let _: rhai::Array = engine.call(&script, "draws", (random.clone(),)).unwrap();
    assert_eq!(event(random, "scenario"), event(Random::new(7), "scenario"));

    // Draws by event never move a sequence: it goes on as if none were.
    let around: rhai::Array = engine
        .call(&script, "around_events", (Random::new(7),))
        .unwrap();
    let drawn: rhai::Array = engine.call(&script, "draws", (Random::new(7),)).unwrap();
    assert_eq!(ints(around), ints(drawn)[..2]);
}

#[test]
fn different_events_and_seeds_draw_apart() {
    let (engine, script) = engine();
    let names: rhai::Array = (0..256)
        .map(|time| rhai::Dynamic::from(format!("weather/direct-out/{time}")))
        .collect();
    let drawn = |seed| -> Vec<i64> {
        ints(
            engine
                .call(
                    &script,
                    "events",
                    (Random::new(seed), names.clone(), 1000_i64),
                )
                .unwrap(),
        )
    };
    let seven = drawn(7);
    assert_eq!(seven, drawn(7));
    assert!(seven.iter().all(|value| (0..1000).contains(value)));
    // Each occurrence draws on its own: few of 256 numbers below a thousand
    // coincide, and another seed draws others.
    let distinct: std::collections::BTreeSet<_> = seven.iter().collect();
    assert!(distinct.len() > 200, "{} distinct", distinct.len());
    let eight = drawn(8);
    let same = seven.iter().zip(&eight).filter(|(a, b)| a == b).count();
    assert!(same < 8, "{same} alike");
    // Names that differ only past their first eight bytes, or in their
    // length, are other events.
    let event = |name: &str| -> i64 {
        engine
            .call(
                &script,
                "event",
                (Random::new(7), name.to_owned(), 1_i64 << 40),
            )
            .unwrap()
    };
    let near = [
        "weather/direct-out/1",
        "weather/direct-out/10",
        "weather/direct-ou",
    ];
    let near: std::collections::BTreeSet<_> = near.into_iter().map(event).collect();
    assert_eq!(near.len(), 3);
}

#[test]
fn an_event_is_named_by_a_path() {
    let (engine, script) = engine();
    for name in [
        "",
        "Weather",
        "weather//1",
        "weather/-1",
        "weather/direct out",
    ] {
        let refused =
            engine.call::<i64>(&script, "event", (Random::new(7), name.to_owned(), 10_i64));
        assert!(
            matches!(&refused, Err(EngineError::Script { message, .. })
                if message.contains(&format!("`{name}` is not an event"))),
            "{name}: {refused:?}"
        );
        let weights: rhai::Array = vec![rhai::Dynamic::from(1_i64)];
        let refused = engine.call::<i64>(
            &script,
            "weighted_event",
            (Random::new(7), name.to_owned(), weights),
        );
        assert!(matches!(refused, Err(EngineError::Script { .. })), "{name}");
    }
    // Bounds are positive.
    for bound in [0_i64, -1] {
        let refused = engine.call::<i64>(
            &script,
            "event",
            (Random::new(7), "scenario".to_owned(), bound),
        );
        assert!(
            matches!(refused, Err(EngineError::Script { .. })),
            "{bound}"
        );
    }
}

#[test]
fn weighted_events_sample_as_the_rust_sampler_does() {
    let (engine, script) = engine();
    let weights = [3_u64, 0, 5, 1];
    let outcomes: Vec<_> = weights
        .iter()
        .enumerate()
        .map(|(index, weight)| WeightedExchange::new(index as i64, *weight))
        .collect();
    let as_ints: rhai::Array = weights
        .iter()
        .map(|weight| rhai::Dynamic::from(*weight as i64))
        .collect();
    let as_quantities: rhai::Array = weights
        .iter()
        .map(|weight| rhai::Dynamic::from(Quantity::new(*weight)))
        .collect();
    let sampled = |seed, name: &str, weights: &rhai::Array| {
        engine.call::<i64>(
            &script,
            "weighted_event",
            (Random::new(seed), name.to_owned(), weights.clone()),
        )
    };
    let mut counts = [0_usize; 4];
    for time in 0..900 {
        let name = format!("weather/safe-out-first/{time}");
        // What `axionomy_search` samples with the event's tickets.
        let expected = *sample(&outcomes, &mut EventTickets::new(17, &path(&name))).unwrap();
        for weights in [&as_ints, &as_quantities] {
            assert_eq!(sampled(17, &name, weights).unwrap(), expected, "{name}");
        }
        counts[usize::try_from(expected).unwrap()] += 1;
    }
    // Each outcome as often as its weight makes it, and never one of none.
    assert_eq!(counts[1], 0);
    for (count, weight) in counts.into_iter().zip(weights) {
        let expected = 100 * weight as usize;
        assert!(count.abs_diff(expected) < 60, "{counts:?}");
    }

    // A number below a bound is the index the event draws from that many
    // equal weights.
    let equal: rhai::Array = (0..6).map(|_| rhai::Dynamic::from(1_i64)).collect();
    for name in ["scenario", "attempt/1/1", "attempt/1/2"] {
        let below: i64 = engine
            .call(&script, "event", (Random::new(17), name.to_owned(), 6_i64))
            .unwrap();
        assert_eq!(sampled(17, name, &equal).unwrap(), below, "{name}");
    }

    // What the Rust sampler refuses is refused: nothing to weigh, no weight,
    // a total past a u64; and so is what is no weight.
    let refusals: [(rhai::Array, &str); 4] = [
        (vec![], "weighted support is empty"),
        (vec![rhai::Dynamic::from(0_i64)], "zero total weight"),
        (
            vec![
                rhai::Dynamic::from(Quantity::new(u64::MAX)),
                rhai::Dynamic::from(Quantity::new(1)),
            ],
            "overflowed",
        ),
        (vec![rhai::Dynamic::from(-1_i64)], "is not a weight"),
    ];
    for (weights, reason) in refusals {
        let refused = sampled(7, "scenario", &weights);
        assert!(
            matches!(&refused, Err(EngineError::Script { message, .. }) if message.contains(reason)),
            "{reason}: {refused:?}"
        );
    }
}

#[test]
fn keyed_tickets_are_the_samples_common_random_numbers() {
    let (engine, script) = engine();
    let keys = [0_u64, 3, 1 << 32 | 2, 7 << 17 | 1 << 9 | 2 << 5 | 1];
    let as_ints: rhai::Array = keys
        .iter()
        .map(|key| rhai::Dynamic::from(*key as i64))
        .collect();
    let keyed = |seed, keys: rhai::Array, bound: i64| {
        engine.call::<rhai::Array>(&script, "keyed", (Random::new(seed), keys, bound))
    };
    for seed in [7, 17, sample_seed(17, 3)] {
        // `splitmix(seed ^ key, 1) % bound`, as the samples key their
        // tickets, whatever else the generator draws in between.
        let expected: Vec<i64> = keys
            .iter()
            .map(|key| (splitmix(seed ^ key, 1) % 10) as i64)
            .collect();
        let drawn: Vec<i64> = keyed(seed, as_ints.clone(), 10)
            .unwrap()
            .into_iter()
            .map(|ticket| ticket.as_int().unwrap())
            .collect();
        assert_eq!(drawn, expected, "seed {seed}");
    }

    // Keys are not negative, and bounds are positive.
    let negative: rhai::Array = vec![rhai::Dynamic::from(-1_i64)];
    assert!(matches!(
        keyed(7, negative, 10),
        Err(EngineError::Script { .. })
    ));
    assert!(matches!(
        keyed(7, as_ints, 0),
        Err(EngineError::Script { .. })
    ));
}

#[test]
fn distances_follow_the_roads_given() {
    let (engine, script) = engine();
    let far: i64 = engine.call(&script, "road", ("south", "east")).unwrap();
    assert_eq!(far, 3);
}

#[test]
fn scripts_are_kept_compiled_bounded_and_read_back() {
    let store = Store::memory().unwrap();
    let engine = Engine::new(store.clone()).unwrap();
    let script: ScriptPath = path("bank/policy");
    engine.keep_script(&script, SCRIPT).unwrap();

    // Another engine over the same store reads the script from it.
    let other = Engine::new(store).unwrap();
    assert_eq!(other.script(&script).unwrap().as_deref(), Some(SCRIPT));
    let total: Quantity = other
        .call(&script, "total", (ScriptState::new(economy()),))
        .unwrap();
    assert_eq!(total, Quantity::new(105));

    // A script that does not compile is refused and nothing is kept.
    let broken: ScriptPath = path("bank/broken");
    assert!(matches!(
        engine.keep_script(&broken, "fn total( {"),
        Err(EngineError::Script { .. })
    ));
    assert!(engine.script(&broken).unwrap().is_none());
    assert!(matches!(
        engine.call::<Quantity>(&broken, "total", ()),
        Err(EngineError::UnknownScript(_))
    ));

    // A script that never ends is stopped.
    assert!(matches!(
        engine.call::<()>(&script, "forever", ()),
        Err(EngineError::Script { .. })
    ));

    // What a script returns must be what the caller expects.
    assert!(matches!(
        engine.call::<bool>(&script, "total", (ScriptState::new(economy()),)),
        Err(EngineError::Returned { .. })
    ));
}

/// Keeps `source` as the script at `script` straight in `store`, as merging
/// a package into it would, without the checks keeping it through an engine
/// makes.
fn define(store: &Store, script: &str, source: &str) {
    let name: axionomy_store::DefinitionPath = path(&format!("script/{script}"));
    let bytes = source.as_bytes().to_vec();
    store
        .transact(axionomy_store::Durability::Immediate, move |txn| {
            txn.define(&name, Some(&bytes))
        })
        .wait()
        .unwrap();
}

#[test]
fn scripts_import_kept_scripts_with_the_host_api() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine
        .keep_script(&path("bank/units"), "fn tens(quantity) { quantity * 10 }")
        .unwrap();
    // A script imported that imports another, recurses, closes over its
    // arguments and reads the economy and draws through the host.
    let library: ScriptPath = path("bank/library");
    engine
        .keep_script(
            &library,
            r#"
            import "bank/units" as units;

            fn fib(n) { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
            fn above(values, floor) { values.filter(|value| value > floor) }
            fn wealth(state) { units::tens(state.balance("bank", "cash")) }
            fn drawn(random) { [random.next(1000), random.sample([1, 2, 3])] }
            "#,
        )
        .unwrap();
    let script: ScriptPath = path("bank/importer");
    engine
        .keep_script(
            &script,
            r#"
            import "bank/library" as library;

            fn fibonacci(n) { library::fib(n) }
            fn above_four(values) { library::above(values, 4) }
            fn bank_wealth(state) { library::wealth(state) }
            fn draws(random) { library::drawn(random) }
            fn units_of(quantity) { library::units::tens(quantity) }
            "#,
        )
        .unwrap();

    // Calls nest as deep in a debug build as in a release one, where Rhai's
    // own limits differ.
    assert_eq!(
        engine.call::<i64>(&script, "fibonacci", (15_i64,)).unwrap(),
        610
    );
    let values: rhai::Array = vec![1_i64.into(), 5_i64.into(), 9_i64.into()];
    let above: rhai::Array = engine.call(&script, "above_four", (values,)).unwrap();
    let above: Vec<i64> = above.iter().map(|value| value.as_int().unwrap()).collect();
    assert_eq!(above, [5, 9]);
    let wealth: Quantity = engine
        .call(&script, "bank_wealth", (ScriptState::new(economy()),))
        .unwrap();
    assert_eq!(wealth, Quantity::new(1000));
    let units: Quantity = engine
        .call(&script, "units_of", (Quantity::new(3),))
        .unwrap();
    assert_eq!(units, Quantity::new(30));

    // An imported script draws what it draws when called itself with the
    // same seed.
    let drawn = |script: &ScriptPath, function: &str| {
        let drawn: rhai::Array = engine.call(script, function, (Random::new(7),)).unwrap();
        drawn
            .iter()
            .map(|value| value.as_int().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(drawn(&script, "draws"), drawn(&library, "drawn"));

    // Replacing an imported script changes what its importers call.
    engine
        .keep_script(&path("bank/units"), "fn tens(quantity) { quantity * 100 }")
        .unwrap();
    let units: Quantity = engine
        .call(&script, "units_of", (Quantity::new(3),))
        .unwrap();
    assert_eq!(units, Quantity::new(300));
}

#[test]
fn a_script_and_those_it_imports_name_their_functions_apart() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine
        .keep_script(&path("bank/units"), "fn tens(quantity) { quantity * 10 }")
        .unwrap();
    engine
        .keep_script(
            &path("bank/scales"),
            "fn tens(quantity) { quantity * 10 } fn hundreds(quantity) { tens(tens(quantity)) }",
        )
        .unwrap();
    // Rhai resolves a function by its name and number of parameters once per
    // call, the script called and those it imports alike, so a function of
    // either could be called for the other's: a script defining one of the
    // same name and number of parameters as one it imports is refused, ...
    let refused = engine.keep_script(
        &path("bank/importer"),
        r#"import "bank/units" as units; fn tens(quantity) { units::tens(quantity) }"#,
    );
    assert_eq!(
        refused.as_ref().unwrap_err().to_string(),
        "scripts `bank/importer` and `bank/units` both define `tens` with 1 parameters: \
         a script and those it imports name their functions apart"
    );
    // ... and so is one importing two scripts that do, ...
    assert!(matches!(
        engine.keep_script(
            &path("bank/importer"),
            r#"import "bank/units" as units; import "bank/scales" as scales;"#,
        ),
        Err(EngineError::SameFunction { function, parameters: 1, .. })
            if function.as_str() == "tens"
    ));
    // ... while one of another number of parameters is the script's own.
    engine
        .keep_script(
            &path("bank/importer"),
            r#"import "bank/scales" as scales; fn tens() { scales::hundreds(quantity(3)) }"#,
        )
        .unwrap();
    let hundreds: Quantity = engine.call(&path("bank/importer"), "tens", ()).unwrap();
    assert_eq!(hundreds, Quantity::new(300));
}

#[test]
fn an_import_names_a_kept_script_by_its_path() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let store = engine.store().clone();
    // A script importing one the store does not keep is refused, naming it,
    // and is not kept.
    let refused = engine.keep_script(
        &path("bank/importer"),
        r#"import "bank/missing" as missing; fn f() { missing::g() }"#,
    );
    assert_eq!(
        refused.as_ref().unwrap_err().to_string(),
        "script `bank/importer` imports `bank/missing`, which the store does not keep"
    );
    assert!(matches!(refused, Err(EngineError::UnknownImport { .. })));
    assert!(engine.script(&path("bank/importer")).unwrap().is_none());

    // So is one importing a script whose own import is not kept.
    define(&store, "bank/broken", r#"import "bank/gone" as gone;"#);
    assert!(matches!(
        engine.keep_script(&path("bank/importer"), r#"import "bank/broken" as broken;"#),
        Err(EngineError::UnknownImport { script, import })
            if script.as_str() == "bank/broken" && import.as_str() == "bank/gone"
    ));

    // An import names its script literally, by a path.
    for (source, refusal) in [
        (
            r#"let name = "bank/units"; import name as units;"#,
            "an import names its script by a literal path",
        ),
        (
            r#"import "Not a path" as units;"#,
            "`Not a path` is not a ScriptPath",
        ),
    ] {
        assert!(matches!(
            engine.keep_script(&path("bank/computed"), source),
            Err(EngineError::Script { message, .. }) if message.starts_with(refusal)
        ));
    }

    // A script importing one the store does not keep, merged in without the
    // engine's checks, is refused when it is called.
    define(
        &store,
        "bank/unchecked",
        r#"import "bank/missing" as missing; fn f() { missing::g() }"#,
    );
    assert!(matches!(
        engine.call::<i64>(&path("bank/unchecked"), "f", ()),
        Err(EngineError::UnknownImport { script, import })
            if script.as_str() == "bank/unchecked" && import.as_str() == "bank/missing"
    ));
}

#[test]
fn scripts_that_import_one_another_are_refused() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let cycle = |kept: Result<(), EngineError>| match kept {
        Err(EngineError::ImportCycle(scripts)) => scripts
            .iter()
            .map(|script| script.as_str().to_owned())
            .collect::<Vec<_>>(),
        other => panic!("expected a cycle, found {other:?}"),
    };
    assert_eq!(
        cycle(engine.keep_script(&path("bank/itself"), r#"import "bank/itself" as itself;"#)),
        ["bank/itself", "bank/itself"]
    );

    engine
        .keep_script(&path("bank/a"), "fn one() { 1 }")
        .unwrap();
    engine
        .keep_script(
            &path("bank/b"),
            r#"import "bank/a" as a; fn two() { a::one() + 1 }"#,
        )
        .unwrap();
    let refused = engine.keep_script(&path("bank/a"), r#"import "bank/b" as b; fn one() { 1 }"#);
    assert_eq!(
        refused.as_ref().unwrap_err().to_string(),
        "scripts import one another in a cycle: `bank/a` → `bank/b` → `bank/a`"
    );
    assert_eq!(cycle(refused), ["bank/a", "bank/b", "bank/a"]);
    // The script refused is not kept: the one before it still is.
    assert_eq!(engine.call::<i64>(&path("bank/b"), "two", ()).unwrap(), 2);

    // Scripts importing one another, merged in without the engine's checks,
    // are refused when called, rather than importing forever.
    let store = engine.store().clone();
    define(
        &store,
        "bank/c",
        r#"import "bank/d" as d; fn f() { d::g() }"#,
    );
    define(&store, "bank/d", r#"import "bank/c" as c; fn g() { 1 }"#);
    assert_eq!(
        cycle(engine.call::<i64>(&path("bank/c"), "f", ()).map(|_| ())),
        ["bank/c", "bank/d", "bank/c"]
    );
}
