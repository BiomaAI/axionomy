use axionomy::{
    Apply, Quantity, Rate, basket,
    path::Path,
    paths::{self, Paths},
};
use axionomy_store::{
    Checkpoints, Durability, Group, Member, Point, RefPath, State, Store, StoreError, Target,
};
use std::{collections::BTreeMap, num::NonZeroU64};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

/// A bank, a few agents and a rate that pays, opens and closes.
fn economy(agents: u64) -> paths::Economy {
    let mut builder = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 1_000_000)]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .rate(path("open"), Rate::new().opens(path("new")))
        .rate(
            path("close"),
            Rate::new()
                .consume(path("gone"), basket([(path("cash"), 1)]))
                .produce(path("heir"), basket([(path("cash"), 1)]))
                .closes(path("gone")),
        );
    for agent in 0..agents {
        builder = builder.account(path(&format!("agents/{agent}")), basket([]).into());
    }
    builder.build().unwrap()
}

fn pay(from: &str, to: &str, amount: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(amount))
        .bind(path("from"), path(from))
        .bind(path("to"), path(to))
}

/// A deterministic sequence of exchanges with openings and closings.
fn exchanges(seed: u64, count: u64, agents: u64) -> Vec<paths::Exchange> {
    (0..count)
        .map(|step| {
            let n = seed * 7_919 + step;
            match step % 10 {
                3 => paths::Exchange::new(path("open"), Quantity::new(1))
                    .bind(path("new"), path(&format!("spawned/{seed}/{step}"))),
                6 if step > 3 => pay("bank", &format!("spawned/{seed}/{}", step - 3), 5),
                9 if step > 6 => paths::Exchange::new(path("close"), Quantity::new(5))
                    .bind(path("gone"), path(&format!("spawned/{seed}/{}", step - 6)))
                    .bind(path("heir"), path("bank")),
                _ => pay("bank", &format!("agents/{}", n % agents), 1 + n % 13),
            }
        })
        .collect()
}

fn every(n: u64) -> Checkpoints {
    Checkpoints::Every(NonZeroU64::new(n).unwrap())
}

#[test]
fn every_point_of_a_run_reads_back_as_the_state_it_was() {
    for checkpoints in [Checkpoints::EveryStep, every(7), Checkpoints::Ends] {
        let store = Store::memory().unwrap();
        let mut recorder = store.record::<Paths>(economy(20), checkpoints);
        let mut roots = vec![recorder.state().root()];
        for exchange in exchanges(1, 60, 20) {
            recorder.apply(exchange).unwrap();
            roots.push(recorder.state().root());
        }
        let run = recorder.run();
        let end = recorder.finish().unwrap();
        assert_eq!(end, Point { run, step: 60 });

        let reader = store.read().unwrap();
        reader.verify::<Paths>(run).unwrap();
        for (step, root) in roots.iter().enumerate() {
            let point = Point {
                run,
                step: step as u64,
            };
            assert_eq!(
                reader.state::<Paths>(point).unwrap().unwrap().root(),
                *root,
                "{checkpoints:?} {step}"
            );
        }
        assert!(
            reader
                .state::<Paths>(Point { run, step: 61 })
                .unwrap()
                .is_none()
        );
        let stored = (0..=60)
            .filter(|&step| reader.root(Point { run, step }).unwrap().is_some())
            .count();
        let expected = match checkpoints {
            Checkpoints::EveryStep => 61,
            Checkpoints::Every(_) => 10,
            Checkpoints::Ends => 2,
        };
        assert_eq!(stored, expected, "{checkpoints:?}");
    }
}

#[test]
fn forks_continue_from_their_point_and_leave_the_source_as_it_was() {
    let store = Store::memory().unwrap();
    let mut source = store.record::<Paths>(economy(5), every(4));
    for exchange in exchanges(2, 30, 5) {
        source.apply(exchange).unwrap();
    }
    let source = source.finish().unwrap();
    let at = Point {
        run: source.run,
        step: 13,
    };
    let mut fork = store.fork::<Paths>(at, Checkpoints::EveryStep).unwrap();
    assert_eq!(fork.point().step, 13);
    let before = fork.state().clone();
    fork.apply(pay("bank", "agents/0", 500)).unwrap();
    let end = fork.finish().unwrap();
    assert_eq!(end.step, 14);

    let reader = store.read().unwrap();
    let forked = reader.state::<Paths>(end).unwrap().unwrap();
    let changes = before.tree.diff(&forked.tree);
    assert_eq!(changes.len(), 2, "the bank's and the agent's cash");
    let original = reader
        .state::<Paths>(Point {
            run: source.run,
            step: 14,
        })
        .unwrap()
        .unwrap();
    assert_ne!(original.root(), forked.root());
    assert_eq!(reader.run(end.run).unwrap().unwrap().from, Some(at));
    reader.verify::<Paths>(end.run).unwrap();
    reader.verify::<Paths>(source.run).unwrap();
}

#[test]
fn a_store_on_disk_keeps_its_runs_and_mints_new_identifiers() {
    let directory = std::env::temp_dir().join(format!("axionomy-store-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("runs.redb");
    let _ = std::fs::remove_file(&file);
    let (first, root) = {
        let store = Store::open(&file).unwrap();
        let mut recorder = store.record::<Paths>(economy(3), Checkpoints::Ends);
        for exchange in exchanges(3, 25, 3) {
            recorder.apply(exchange).unwrap();
        }
        let root = recorder.state().root();
        (recorder.finish().unwrap(), root)
    };
    let store = Store::open(&file).unwrap();
    let reader = store.read().unwrap();
    assert_eq!(reader.state::<Paths>(first).unwrap().unwrap().root(), root);
    reader.verify::<Paths>(first.run).unwrap();
    assert_ne!(store.mint_run(), first.run);
    assert!(store.mint_run() > first.run);
    drop(store);
    std::fs::remove_dir_all(&directory).unwrap();
}

/// Bytes that outlive the stores opened on them, as a browser's origin
/// private file system does: any redb backend serves.
#[derive(Debug, Clone, Default)]
struct Kept(std::sync::Arc<redb::backends::InMemoryBackend>);

impl redb::StorageBackend for Kept {
    fn len(&self) -> Result<u64, std::io::Error> {
        self.0.len()
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), std::io::Error> {
        self.0.read(offset, out)
    }

    fn set_len(&self, len: u64) -> Result<(), std::io::Error> {
        self.0.set_len(len)
    }

    fn sync_data(&self) -> Result<(), std::io::Error> {
        self.0.sync_data()
    }

    fn write(&self, offset: u64, data: &[u8]) -> Result<(), std::io::Error> {
        self.0.write(offset, data)
    }
}

#[test]
fn a_store_on_any_backend_keeps_its_runs_there() {
    let kept = Kept::default();
    let (first, root) = {
        let store = Store::on(kept.clone()).unwrap();
        assert!(!store.in_memory());
        let mut recorder = store.record::<Paths>(economy(3), Checkpoints::Ends);
        for exchange in exchanges(5, 25, 3) {
            recorder.apply(exchange).unwrap();
        }
        let root = recorder.state().root();
        (recorder.finish().unwrap(), root)
    };
    let store = Store::on(kept).unwrap();
    let reader = store.read().unwrap();
    assert_eq!(reader.state::<Paths>(first).unwrap().unwrap().root(), root);
    reader.verify::<Paths>(first.run).unwrap();
    assert!(store.mint_run() > first.run);
    assert!(Store::memory().unwrap().in_memory());
}

#[test]
fn episodes_from_one_state_store_its_nodes_once() {
    let store = Store::memory().unwrap();
    let start = economy(2_000);
    let mut runs = Vec::new();
    for episode in 0..8 {
        let mut recorder = store.record::<Paths>(start.clone(), Checkpoints::Ends);
        for exchange in exchanges(episode, 20, 2_000) {
            recorder.apply(exchange).unwrap();
        }
        runs.push(recorder.finish().unwrap());
        if episode == 0 {
            let first = store.read().unwrap().nodes().unwrap();
            assert!(
                first > 20,
                "a state of two thousand accounts spans many nodes"
            );
        }
    }
    let reader = store.read().unwrap();
    let nodes = reader.nodes().unwrap();
    let one = State::<Paths>::new(start).tree.missing(|_| false).len() as u64;
    // Each later episode adds only the nodes its last state changed.
    assert!(nodes < one + 8 * 20, "{nodes} nodes for {one} per state");
    for end in runs {
        reader.verify::<Paths>(end.run).unwrap();
    }
}

#[test]
fn tags_metrics_groups_and_refs_organize_runs() {
    let store = Store::memory().unwrap();
    let group = store.mint_group();
    let returns = [3.0, 7.0, 5.0, 1.0, 9.0];
    let mut ends = Vec::new();
    for (sample, value) in returns.iter().enumerate() {
        let mut recorder = store.record::<Paths>(economy(4), Checkpoints::Ends);
        recorder.tag(
            path("split"),
            Some(if sample % 2 == 0 { "eval" } else { "train" }.into()),
        );
        recorder
            .apply(pay("bank", "agents/1", 1 + sample as u64))
            .unwrap();
        recorder.metric(path("return"), *value, false).unwrap();
        recorder.metric(path("reward"), *value / 2.0, true).unwrap();
        assert!(matches!(
            recorder.metric(path("return"), f64::NAN, false),
            Err(StoreError::NotFinite(_))
        ));
        ends.push(recorder.finish().unwrap());
    }
    let source = ends[0];
    let created = store.transact(Durability::Immediate, move |txn| {
        txn.create_group(
            group,
            Group {
                kind: path("monte-carlo"),
                source: Some(source),
                tags: BTreeMap::from([(path("rollouts"), "5".into())]),
            },
        )?;
        for (sample, (end, value)) in ends.iter().zip(returns).enumerate() {
            let member = Member {
                run: Some(end.run),
                seed: Some(sample as u64),
                metrics: BTreeMap::from([(path("return"), value)]),
            };
            assert!(txn.add_member(group, sample as u64, &member)?);
            assert!(!txn.add_member(group, sample as u64, &member)?);
        }
        txn.set_ref(&path("best/rollout"), Some(Target::Run(ends[4].run)))?;
        Ok(ends)
    });
    let ends = created.wait().unwrap();

    let reader = store.read().unwrap();
    let eval = reader.tagged(&path("split"), "eval", None, 10).unwrap();
    assert_eq!(eval, vec![ends[0].run, ends[2].run, ends[4].run]);
    assert_eq!(
        reader
            .tagged(&path("split"), "eval", Some(ends[2].run), 10)
            .unwrap(),
        vec![ends[4].run]
    );
    assert_eq!(
        reader.run_metrics(ends[1].run).unwrap()[&path("return")],
        7.0
    );
    assert_eq!(reader.step_metrics(ends[1]).unwrap()[&path("reward")], 3.5);

    let recorded = reader.group(group).unwrap().unwrap();
    assert_eq!(recorded.group.source, Some(ends[0]));
    let summary = recorded.summaries[&path("return")];
    assert_eq!(
        (summary.count, summary.mean, summary.min, summary.max),
        (5, 5.0, 1.0, 9.0)
    );
    assert!((summary.variance() - 10.0).abs() < 1e-12);
    assert_eq!(
        reader.quantile(group, &path("return"), 0.5).unwrap(),
        Some(5.0)
    );
    assert_eq!(reader.members(group, 3, 10).unwrap().len(), 2);
    assert_eq!(
        reader.reference(&path("best/rollout")).unwrap(),
        Some(Target::Run(ends[4].run))
    );
    assert_eq!(reader.refs("best/", 10).unwrap().len(), 1);
}

/// Code that steps an economy takes `&mut impl Apply`, so a recorder steps
/// it as an economy would and keeps the steps.
fn pay_round(world: &mut impl Apply<Paths>, agents: u64) {
    for agent in 0..agents {
        world
            .apply(pay("bank", &format!("agents/{}", agent % 3), 1 + agent))
            .unwrap();
    }
}

#[test]
fn a_group_recorder_adds_each_episode_as_it_ends() {
    let store = Store::memory().unwrap();
    let mut group = store.group(Group {
        kind: path("monte-carlo"),
        source: None,
        tags: BTreeMap::new(),
    });
    let mut ends = Vec::new();
    for sample in 0..4 {
        let mut recorder = group.record::<Paths>(economy(3), Checkpoints::Ends);
        pay_round(&mut recorder, 1 + sample);
        let mut unrecorded = economy(3);
        pay_round(&mut unrecorded, 1 + sample);
        assert_eq!(recorder.economy().state_key(), unrecorded.state_key());
        let metrics = BTreeMap::from([(path("steps"), (1 + sample) as f64)]);
        ends.push(group.add(sample, Some(sample), recorder, metrics).unwrap());
    }
    let again = group.record::<Paths>(economy(3), Checkpoints::Ends);
    let metrics = BTreeMap::from([(path("steps"), 100.0)]);
    group.add(0, None, again, metrics).unwrap();
    let nan = group.record::<Paths>(economy(3), Checkpoints::Ends);
    assert!(matches!(
        group.add(9, None, nan, BTreeMap::from([(path("steps"), f64::NAN)])),
        Err(StoreError::NotFinite(_))
    ));
    let id = group.finish().unwrap();

    let reader = store.read().unwrap();
    let summary = reader.group(id).unwrap().unwrap().summaries[&path("steps")];
    assert_eq!((summary.count, summary.mean), (4, 2.5));
    let members = reader.members(id, 0, 10).unwrap();
    assert_eq!(members.len(), 4);
    for (end, (_, member)) in ends.iter().zip(&members) {
        assert_eq!(member.run, Some(end.run));
        assert!(reader.run(end.run).unwrap().unwrap().finished);
        reader.verify::<Paths>(end.run).unwrap();
    }
}

#[test]
fn deleting_runs_and_collecting_keeps_what_is_pinned_or_named() {
    let store = Store::memory().unwrap();
    let mut ends = Vec::new();
    for episode in 0..3 {
        let mut recorder = store.record::<Paths>(economy(50), Checkpoints::EveryStep);
        for exchange in exchanges(episode, 15, 50) {
            recorder.apply(exchange).unwrap();
        }
        if episode == 1 {
            recorder.pin();
        }
        ends.push(recorder.finish().unwrap());
    }
    let runs: Vec<_> = ends.iter().map(|end| end.run).collect();
    let deleted = store
        .transact(Durability::Immediate, move |txn| {
            txn.set_ref(&path("baseline"), Some(Target::Run(runs[2])))?;
            Ok([
                txn.delete_run(runs[0])?,
                txn.delete_run(runs[1])?,
                txn.delete_run(runs[2])?,
            ])
        })
        .wait()
        .unwrap();
    assert_eq!(deleted, [true, false, false]);
    let before = store.read().unwrap().nodes().unwrap();
    let collected = store
        .transact(Durability::Immediate, |txn| txn.collect())
        .wait()
        .unwrap();
    assert!(collected > 0);
    let reader = store.read().unwrap();
    assert_eq!(reader.nodes().unwrap(), before - collected as u64);
    assert!(reader.run(ends[0].run).unwrap().is_none());
    for end in &ends[1..] {
        reader.verify::<Paths>(end.run).unwrap();
    }
    let again = store
        .transact(Durability::Immediate, |txn| txn.collect())
        .wait()
        .unwrap();
    assert_eq!(again, 0);
}

#[test]
fn stores_written_apart_merge_into_one() {
    let (left, right) = (Store::memory().unwrap(), Store::memory().unwrap());
    let record = |store: &Store, seed| {
        let mut recorder = store.record::<Paths>(economy(10), every(5));
        for exchange in exchanges(seed, 12, 10) {
            recorder.apply(exchange).unwrap();
        }
        recorder.finish().unwrap()
    };
    let (ours, theirs) = (record(&left, 1), record(&right, 2));
    assert_ne!(ours.run, theirs.run);
    for (store, end) in [(&left, ours), (&right, theirs)] {
        store
            .transact(Durability::Immediate, move |txn| {
                txn.set_ref(&path("latest"), Some(Target::Step(end)))
            })
            .wait()
            .unwrap();
    }
    let kept = left.merge(&right).wait().unwrap();
    assert_eq!(kept.refs, vec![path("latest")]);
    let reader = left.read().unwrap();
    reader.verify::<Paths>(ours.run).unwrap();
    reader.verify::<Paths>(theirs.run).unwrap();
    assert_eq!(
        reader.reference(&path("latest")).unwrap(),
        Some(Target::Step(ours))
    );
    assert_eq!(
        left.merge(&right).wait().unwrap().refs,
        vec![path("latest")]
    );
}

#[test]
fn parallel_recorders_share_one_writer() {
    let store = Store::memory().unwrap();
    let ends: Vec<Point> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|worker| {
                let store = store.clone();
                scope.spawn(move || {
                    let mut recorder = store.record::<Paths>(economy(30), every(16));
                    for exchange in exchanges(worker, 400, 30) {
                        recorder.apply(exchange).unwrap();
                    }
                    recorder.finish().unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    let reader = store.read().unwrap();
    for end in ends {
        assert_eq!(end.step, 400);
        reader.verify::<Paths>(end.run).unwrap();
    }
}

#[test]
fn replay_rejects_a_trace_that_does_not_reproduce_its_receipts() {
    let store = Store::memory().unwrap();
    let mut recorder = store.record::<Paths>(economy(3), Checkpoints::Ends);
    recorder.apply(pay("bank", "agents/0", 4)).unwrap();
    let end = recorder.finish().unwrap();
    // Overwrite the step with an exchange whose receipt differs from the one
    // the state it meets would give.
    let mut other = State::<Paths>::new(economy(3));
    let wrong = other.apply(pay("bank", "agents/1", 4)).unwrap();
    store
        .transact(Durability::Immediate, move |txn| {
            txn.put_step::<Paths>(end, &wrong, None)
        })
        .wait()
        .unwrap();
    let reader = store.read().unwrap();
    assert!(
        matches!(reader.verify::<Paths>(end.run), Err(StoreError::Diverged(point)) if point == end)
    );
}

/// A ledger with Rust types of its own for its identifiers.
mod ledger {
    use axionomy::{Identifier, Ontology};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
    pub enum Account {
        Bank,
        Agent(u8),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
    pub enum Asset {
        Cash,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
    pub enum Rate {
        Pay,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
    pub enum Role {
        From,
        To,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Ledger {}

    impl Ontology for Ledger {
        const NAME: &'static str = "ledger";
        type Account = Account;
        type Asset = Asset;
        type Rate = Rate;
        type Role = Role;
    }

    /// The ledger's identifiers under another name.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Other {}

    impl Ontology for Other {
        const NAME: &'static str = "other";
        type Account = Account;
        type Asset = Asset;
        type Rate = Rate;
        type Role = Role;
    }
}

#[test]
fn typed_economies_are_recorded_and_read_back_only_as_their_ontology() {
    use axionomy::{EconomyBuilder, Exchange};
    use ledger::{Account, Asset, Ledger, Role};

    let economy = EconomyBuilder::new()
        .account(Account::Bank, basket([(Asset::Cash, 10)]).into())
        .account(Account::Agent(1), basket([]).into())
        .rate(
            ledger::Rate::Pay,
            Rate::new()
                .consume(Role::From, basket([(Asset::Cash, 1)]))
                .produce(Role::To, basket([(Asset::Cash, 1)])),
        )
        .build()
        .unwrap();
    let store = Store::memory().unwrap();
    let mut recorder = store.record::<Ledger>(economy, Checkpoints::Ends);
    for _ in 0..3 {
        recorder
            .apply(
                Exchange::new(ledger::Rate::Pay, Quantity::new(2))
                    .bind(Role::From, Account::Bank)
                    .bind(Role::To, Account::Agent(1)),
            )
            .unwrap();
    }
    let end = recorder.finish().unwrap();

    let reader = store.read().unwrap();
    reader.verify::<Ledger>(end.run).unwrap();
    let middle = reader
        .state::<Ledger>(Point {
            run: end.run,
            step: 2,
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        middle.economy.balance(&Account::Agent(1), &Asset::Cash),
        Quantity::new(4)
    );
    assert_eq!(
        reader.run(end.run).unwrap().unwrap().ontology.as_str(),
        "ledger"
    );
    // Any run reads as paths, its identifiers written as their paths.
    let written = reader.state::<Paths>(end).unwrap().unwrap();
    assert_eq!(
        written
            .economy
            .balance(&"agent/1".parse().unwrap(), &"cash".parse().unwrap()),
        Quantity::new(6)
    );
    // A fork written as paths keeps the ontology it forked from, and reads
    // back as it.
    let mut paths_fork = store.fork::<Paths>(end, Checkpoints::Ends).unwrap();
    paths_fork
        .apply(
            axionomy::paths::Exchange::new("pay".parse().unwrap(), Quantity::new(1))
                .bind("from".parse().unwrap(), "bank".parse().unwrap())
                .bind("to".parse().unwrap(), "agent/1".parse().unwrap()),
        )
        .unwrap();
    let paths_forked = paths_fork.finish().unwrap();
    let reader = store.read().unwrap();
    assert_eq!(
        reader
            .run(paths_forked.run)
            .unwrap()
            .unwrap()
            .ontology
            .as_str(),
        "ledger"
    );
    reader.verify::<Ledger>(paths_forked.run).unwrap();
    assert_eq!(
        reader
            .state::<Ledger>(paths_forked)
            .unwrap()
            .unwrap()
            .economy
            .balance(&Account::Agent(1), &Asset::Cash),
        Quantity::new(7)
    );
    // Another ontology's run is refused to a typed reading.
    assert!(matches!(
        reader.state::<ledger::Other>(end),
        Err(StoreError::OtherOntology { run, .. }) if run == end.run
    ));
    let mut fork = store.fork::<Ledger>(end, Checkpoints::Ends).unwrap();
    fork.apply(
        Exchange::new(ledger::Rate::Pay, Quantity::new(1))
            .bind(Role::From, Account::Bank)
            .bind(Role::To, Account::Agent(1)),
    )
    .unwrap();
    let forked = fork.finish().unwrap();
    assert_eq!(forked.step, 4);
    store.read().unwrap().verify::<Ledger>(forked.run).unwrap();
}

#[test]
fn bisect_finds_where_a_lasting_condition_starts_under_every_checkpoint_rule() {
    let store = Store::memory().unwrap();
    for checkpoints in [
        Checkpoints::EveryStep,
        Checkpoints::Every(NonZeroU64::new(5).unwrap()),
        Checkpoints::Ends,
    ] {
        let mut recorder = store.record::<Paths>(economy(3), checkpoints);
        for _ in 0..40 {
            recorder.apply(pay("bank", "agents/1", 3)).unwrap();
        }
        let end = recorder.finish().unwrap();
        let reader = store.read().unwrap();
        let reached = |target: u64| {
            move |economy: &paths::Economy| {
                economy.balance(&path("agents/1"), &path("cash")) >= Quantity::new(target)
            }
        };
        // The agent holds 3n after step n, so it first holds at least t at
        // step ⌈t / 3⌉.
        for (target, step) in [
            (0, Some(0)),
            (1, Some(1)),
            (30, Some(10)),
            (59, Some(20)),
            (120, Some(40)),
        ] {
            assert_eq!(
                reader
                    .bisect::<Paths>(end.run, 0, 40, reached(target))
                    .unwrap(),
                step,
                "{checkpoints:?} reaching {target}"
            );
        }
        assert_eq!(
            reader
                .bisect::<Paths>(end.run, 0, 40, reached(121))
                .unwrap(),
            None
        );
        assert_eq!(
            reader
                .bisect::<Paths>(end.run, 12, 30, reached(30))
                .unwrap(),
            Some(12)
        );
        assert_eq!(
            reader.bisect::<Paths>(end.run, 0, 9, reached(30)).unwrap(),
            None
        );
        assert!(reader.bisect::<Paths>(end.run, 0, 41, reached(1)).is_err());
    }
}

#[test]
fn transitions_chain_the_roots_of_a_run() {
    let store = Store::memory().unwrap();
    let mut recorder =
        store.record::<Paths>(economy(4), Checkpoints::Every(NonZeroU64::new(4).unwrap()));
    for exchange in exchanges(3, 30, 4) {
        recorder.apply(exchange).unwrap();
    }
    let end = recorder.finish().unwrap();
    let reader = store.read().unwrap();
    let transitions = reader.transitions::<Paths>(end.run, 0, 100).unwrap();
    assert_eq!(transitions.len(), 30);
    for pair in transitions.windows(2) {
        assert_eq!(pair[0].after, pair[1].before);
        assert_eq!(pair[0].step + 1, pair[1].step);
    }
    for transition in &transitions {
        let point = Point {
            run: end.run,
            step: transition.step,
        };
        if let Some(root) = reader.root(point).unwrap() {
            assert_eq!(root, transition.after);
        }
        assert_eq!(
            reader.state::<Paths>(point).unwrap().unwrap().root(),
            transition.after
        );
    }
    let tail = reader.transitions::<Paths>(end.run, 25, 3).unwrap();
    assert_eq!(
        tail.iter()
            .map(|transition| transition.step)
            .collect::<Vec<_>>(),
        [26, 27, 28]
    );
    assert_eq!(tail[0].before, transitions[24].after);
}

#[test]
fn a_store_in_another_format_is_refused() {
    let directory =
        std::env::temp_dir().join(format!("axionomy-store-format-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    // A store from before formats were recorded has a prefix and no format.
    let old = directory.join("old.redb");
    let _ = std::fs::remove_file(&old);
    {
        let database = redb::Database::create(&old).unwrap();
        let txn = database.begin_write().unwrap();
        txn.open_table(redb::TableDefinition::<&str, u64>::new("meta"))
            .unwrap()
            .insert("store-prefix", 7)
            .unwrap();
        txn.commit().unwrap();
    }
    assert!(matches!(
        Store::open(&old),
        Err(StoreError::Format { found: None })
    ));
    // A store this version wrote opens again.
    let fresh = directory.join("fresh.redb");
    let _ = std::fs::remove_file(&fresh);
    drop(Store::open(&fresh).unwrap());
    assert!(Store::open(&fresh).is_ok());
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn definitions_are_kept_by_name_and_merged_like_refs() {
    let define = |store: &Store, name: &'static str, value: &'static [u8]| {
        store
            .transact(Durability::Immediate, move |txn| {
                txn.define(&path(name), Some(value))
            })
            .wait()
            .unwrap();
    };
    let (left, right) = (Store::memory().unwrap(), Store::memory().unwrap());
    define(&left, "script/maze/heuristic", b"ours");
    define(&left, "goal/maze/solved", b"same");
    define(&right, "script/maze/heuristic", b"theirs");
    define(&right, "goal/maze/solved", b"same");
    define(&right, "script/sokoban/candidates", b"new");

    let merged = left.merge(&right).wait().unwrap();
    assert_eq!(merged.definitions, vec![path("script/maze/heuristic")]);
    assert!(merged.refs.is_empty());
    let reader = left.read().unwrap();
    assert_eq!(
        reader.definition(&path("script/maze/heuristic")).unwrap(),
        Some(b"ours".to_vec())
    );
    let scripts: Vec<_> = reader
        .definitions("script/", 10)
        .unwrap()
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
        .collect();
    assert_eq!(
        scripts,
        vec![
            ("script/maze/heuristic".to_owned(), b"ours".to_vec()),
            ("script/sokoban/candidates".to_owned(), b"new".to_vec()),
        ]
    );
}

#[test]
fn branches_are_indexed_and_a_ref_moves_only_from_where_it_is() {
    let store = Store::memory().unwrap();
    let mut source = store.record::<Paths>(economy(3), every(4));
    for exchange in exchanges(1, 10, 3) {
        source.apply(exchange).unwrap();
    }
    let source = source.finish().unwrap();
    let fork_at = |step| {
        let mut fork = store
            .fork::<Paths>(
                Point {
                    run: source.run,
                    step,
                },
                Checkpoints::Ends,
            )
            .unwrap();
        fork.apply(pay("bank", "agents/0", 1)).unwrap();
        fork.finish().unwrap()
    };
    let late = fork_at(7);
    let early = fork_at(2);
    let reader = store.read().unwrap();
    // Every fork of a run, in the order of the steps they fork at.
    assert_eq!(
        reader.branches(source.run).unwrap(),
        [(2, early.run), (7, late.run)]
    );
    assert!(reader.branches(late.run).unwrap().is_empty());

    // A run others forked from stays; a fork with none goes, and leaves the
    // index.
    let deleted = |run| {
        store
            .transact(Durability::Immediate, move |txn| txn.delete_run(run))
            .wait()
            .unwrap()
    };
    assert!(!deleted(source.run));
    assert!(deleted(late.run));
    assert_eq!(
        store.read().unwrap().branches(source.run).unwrap(),
        [(2, early.run)]
    );

    // A ref moves only from where the mover thinks it is.
    let name: RefPath = path("live");
    let on = |point| Some(Target::Step(point));
    let set = |expected, to| {
        let name = name.clone();
        store
            .transact(Durability::Immediate, move |txn| {
                txn.compare_and_set_ref(&name, expected, to)
            })
            .wait()
    };
    set(None, on(source)).unwrap();
    // Created once: a second creation is refused, saying where it is.
    assert!(matches!(
        set(None, on(early)),
        Err(StoreError::Moved { current, .. }) if current == on(source)
    ));
    set(on(source), on(early)).unwrap();
    // A mover that still thinks it is on the source looks again.
    assert!(matches!(
        set(on(source), on(source)),
        Err(StoreError::Moved { current, .. }) if current == on(early)
    ));
    assert_eq!(store.read().unwrap().reference(&name).unwrap(), on(early));
}

#[test]
fn invariants_read_back_in_the_order_they_were_declared() {
    // Declared out of name order: the model's order is the declaration's,
    // since an exchange breaking several is refused for the first.
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), axionomy::basket([(path("cash"), 5)]).into())
        .invariant(paths::LinearInvariant::new(path("zeta")).weight(path("cash"), 1))
        .invariant(paths::LinearInvariant::new(path("alpha")).weight(path("cash"), 2))
        .invariant(paths::LinearInvariant::new(path("alpha")).weight(path("cash"), 1))
        .build()
        .unwrap();
    let store = Store::memory().unwrap();
    let point = store
        .record::<Paths>(economy.clone(), Checkpoints::Ends)
        .finish()
        .unwrap();
    let read = store
        .read()
        .unwrap()
        .state::<Paths>(point)
        .unwrap()
        .unwrap();
    let names = |economy: &paths::Economy| {
        economy
            .invariants()
            .map(|invariant| invariant.name().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&read.economy), ["zeta", "alpha", "alpha"]);
    // Each in its place, the same weights.
    let weights = |economy: &paths::Economy| {
        economy
            .invariants()
            .map(|invariant| format!("{invariant:?}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(weights(&read.economy), weights(&economy));
}
