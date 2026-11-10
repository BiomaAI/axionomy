//! Every canonical problem studied through the registry: catalog entries,
//! replayable documents, readable labels, trajectories, showcase pressure,
//! and cooperative control mid-study.

use axionomy::{path, path::Path};
use axionomy_problems::{lending, registry};
use axionomy_study::{
    InstanceProfile, Progress, Study, StudyControl, StudyError, StudyObserver, StudyRequest,
};
use axionomy_view::{DocumentPath, ExchangeFrame, SceneSurfaceView, ViewDocument, ViewOntology};
use std::collections::HashSet;

#[derive(Default)]
struct FrameObserver {
    documents: Vec<DocumentPath>,
    frames: Vec<ExchangeFrame>,
}

impl StudyObserver for FrameObserver {
    fn progress(&mut self, _: Progress) {}

    fn frame(&mut self, document: &DocumentPath, frame: ExchangeFrame) {
        self.documents.push(document.clone());
        self.frames.push(frame);
    }
}

fn assert_unique<'a>(problem: &str, kind: &str, ids: impl Iterator<Item = &'a str>) {
    let mut seen = HashSet::new();
    for id in ids {
        assert!(seen.insert(id), "{problem} repeats {kind} `{id}`");
    }
}

fn has_linked_entity_change(document: &ViewDocument) -> bool {
    let states = std::iter::once(&document.initial)
        .chain(document.frames.iter().map(|frame| &frame.after))
        .collect::<Vec<_>>();
    states.windows(2).any(|pair| {
        let Some(before) = pair[0].scene.as_ref() else {
            return false;
        };
        let Some(after) = pair[1].scene.as_ref() else {
            return false;
        };
        before.entities.iter().any(|left| {
            !left.evidence.is_empty()
                && after
                    .entities
                    .iter()
                    .find(|right| right.id.key == left.id.key)
                    .is_none_or(|right| {
                        left.anchor != right.anchor
                            || left.status != right.status
                            || left.tone != right.tone
                            || left.metrics != right.metrics
                    })
        }) || after.entities.iter().any(|right| {
            !right.evidence.is_empty()
                && !before
                    .entities
                    .iter()
                    .any(|left| left.id.key == right.id.key)
        })
    })
}

#[test]
fn catalog_covers_every_canonical_problem() {
    let catalog = registry().catalog();
    assert_eq!(catalog.len(), 15);
    assert!(catalog.iter().all(|problem| !problem.strategies.is_empty()));
    assert!(catalog.iter().all(|problem| {
        problem.default_instance == InstanceProfile::Showcase
            && problem
                .instances
                .iter()
                .map(|instance| instance.profile)
                .eq([
                    InstanceProfile::Micro,
                    InstanceProfile::Showcase,
                    InstanceProfile::Stress,
                ])
    }));
    assert!(catalog.iter().all(|problem| {
        problem
            .instances
            .iter()
            .map(|instance| instance.label.as_str())
            .eq(["Micro", "Showcase", "Stress"])
            && problem.instances.iter().all(|instance| {
                !instance.description.is_empty()
                    && !instance.description.contains("Larger seeded workload")
            })
            && problem.strategies.iter().all(|strategy| {
                !strategy.algorithm.is_empty()
                    && strategy.algorithm != strategy.description.to_lowercase()
            })
    }));
}

#[test]
fn maze_retains_strategy_specific_search_and_route_evidence() {
    use axionomy_view::{AssessmentStatusView, ScenePathStatusView, SearchObservationKindView};

    let study = registry()
        .study(
            StudyRequest::new(path!("maze"))
                .with_instance(InstanceProfile::Showcase)
                .with_strategy(path!("a-star")),
        )
        .unwrap();
    for (document, algorithm) in [
        (path!("breadth-first"), path!("breadth-first")),
        (path!("dijkstra"), path!("dijkstra")),
        (path!("a-star"), path!("a-star")),
    ] {
        let document = study.document(&document).unwrap();
        assert!(
            document
                .thinking
                .iter()
                .all(|observation| observation.algorithm == algorithm)
        );
        let accepted = document.thinking.last().unwrap();
        assert_eq!(accepted.kind, SearchObservationKindView::Incumbent);
        assert!(accepted.label.starts_with("Solution accepted at Exit"));
        assert_eq!(accepted.subjects[0].key.as_str(), "node/exit");
    }
    for document in [path!("pareto-energy"), path!("pareto-time")] {
        let document = study.document(&document).unwrap();
        let observation = document.thinking.last().unwrap();
        assert_eq!(observation.algorithm.as_str(), "exact-pareto-search");
        assert_eq!(observation.kind, SearchObservationKindView::Frontier);
        assert!(observation.label.contains("4 non-dominated tradeoffs"));
    }

    let selected = study.selected().unwrap();
    assert!(
        selected
            .initial
            .scene
            .as_ref()
            .unwrap()
            .paths
            .iter()
            .any(|path| path.status == ScenePathStatusView::Candidate)
    );
    assert_eq!(
        selected.frames[0]
            .after
            .scene
            .as_ref()
            .unwrap()
            .paths
            .iter()
            .filter(|path| path.status == ScenePathStatusView::Current)
            .count(),
        1
    );
    assert_eq!(
        selected.proposals[0].assessment.status,
        AssessmentStatusView::Infeasible
    );
    let missing = selected.proposals[0]
        .assessment
        .shortfalls
        .iter()
        .flat_map(|shortfall| shortfall.missing.iter())
        .map(|asset| asset.asset.label.as_str())
        .collect::<Vec<_>>();
    assert!(missing.contains(&"Standing in Gate"));
    assert!(missing.contains(&"Vault gate open"));
}

#[test]
fn selected_replay_frames_stream_with_the_same_snapshot_contract() {
    let request = StudyRequest::new(path!("work-league"))
        .with_instance(InstanceProfile::Micro)
        .with_strategy(path!("mixed-field"));
    let mut observer = FrameObserver::default();
    let study = registry()
        .study_with(&request, &StudyControl::default(), &mut observer)
        .unwrap();
    let selected = study.selected().unwrap();
    assert_eq!(observer.frames.len(), selected.frames.len());
    assert!(
        observer
            .documents
            .iter()
            .all(|document| document == &selected.id)
    );
    assert!(
        observer
            .frames
            .iter()
            .all(|frame| !frame.after.leaderboards.is_empty())
    );
    assert_eq!(observer.frames.last(), selected.frames.last());
}

/// Each document of every study reports one trajectory, written as paths,
/// and it is the trajectory its frames show: replayed, it ends where the
/// document's last snapshot does.
#[test]
fn every_document_reports_the_trajectory_its_frames_replay() {
    use axionomy::paths;

    #[derive(Default)]
    struct Trajectories(Vec<(DocumentPath, paths::Economy, paths::Trace)>);

    impl StudyObserver for Trajectories {
        fn progress(&mut self, _: Progress) {}

        fn trajectory(
            &mut self,
            document: &DocumentPath,
            initial: &paths::Economy,
            trace: &paths::Trace,
        ) {
            self.0
                .push((document.clone(), initial.clone(), trace.clone()));
        }
    }

    /// The path a view key `{ontology}/{kind}/{path}` writes.
    fn written<K>(key: &axionomy_view::ItemPath, kind: &str) -> Path<K> {
        let (_, path) = key.as_str().split_once(&format!("/{kind}/")).unwrap();
        path.parse().unwrap()
    }

    for problem in registry().catalog() {
        let mut request =
            StudyRequest::new(problem.key.clone()).with_instance(InstanceProfile::Micro);
        request.budget = 8;
        let mut observed = Trajectories::default();
        let study = registry()
            .study_with(&request, &StudyControl::default(), &mut observed)
            .unwrap();
        assert_eq!(
            observed.0.iter().map(|(id, ..)| id).collect::<Vec<_>>(),
            study
                .documents
                .iter()
                .map(|document| &document.id)
                .collect::<Vec<_>>(),
            "{}",
            problem.key
        );
        for ((id, initial, trace), document) in observed.0.iter().zip(&study.documents) {
            assert_eq!(trace.exchanges().len(), document.frames.len(), "{id}");
            let last = initial.replayed(trace).unwrap();
            let shown = document
                .frames
                .last()
                .map_or(&document.initial, |frame| &frame.after);
            assert_eq!(last.accounts().count(), shown.accounts.len(), "{id}");
            for account in &shown.accounts {
                let account_path = written(&account.account.key, "account");
                for balance in &account.balances {
                    assert_eq!(
                        last.balance(&account_path, &written(&balance.asset.key, "asset"))
                            .to_string(),
                        balance.quantity.0,
                        "{} {id} {account_path}",
                        problem.key
                    );
                }
            }
        }
    }
}

#[test]
fn formerly_aliased_stress_models_are_structurally_distinct() {
    use axionomy_problems::{bridge, exact_cover, marketplace, maze, rescue, sokoban};

    let maze_showcase = maze::initial_showcase();
    let maze_stress = maze::initial_stress();
    assert_ne!(maze_showcase.state_key(), maze_stress.state_key());
    assert_ne!(
        maze_showcase.rate_ids().count(),
        maze_stress.rate_ids().count()
    );

    let cover_showcase = exact_cover::initial_showcase();
    let cover_stress = exact_cover::initial_stress();
    assert_ne!(cover_showcase.state_key(), cover_stress.state_key());
    assert_ne!(
        cover_showcase.rate_ids().count(),
        cover_stress.rate_ids().count()
    );

    let warehouse_showcase = sokoban::initial_showcase();
    let warehouse_stress = sokoban::initial_stress();
    assert_eq!(
        warehouse_showcase
            .rate_ids()
            .filter(|rate| matches!(rate, sokoban::RateId::Latch { .. }))
            .count(),
        1
    );
    assert_eq!(
        warehouse_stress
            .rate_ids()
            .filter(|rate| matches!(rate, sokoban::RateId::Latch { .. }))
            .count(),
        2
    );
    assert!(
        warehouse_showcase
            .accounts()
            .any(|(_, account)| account.balance(&sokoban::Asset::ExitCell).to_u64() == 1)
    );

    let bridge_showcase = bridge::initial_showcase();
    let bridge_stress = bridge::initial_stress();
    assert_ne!(bridge_showcase.state_key(), bridge_stress.state_key());

    let market_showcase = marketplace::initial_showcase();
    let market_stress = marketplace::initial_stress();
    assert_ne!(market_showcase.state_key(), market_stress.state_key());
    assert_ne!(
        market_showcase.rate_ids().count(),
        market_stress.rate_ids().count()
    );

    let rescue_showcase = rescue::uniform_uncertain_showcase();
    let rescue_stress = rescue::uniform_uncertain_stress();
    assert_ne!(rescue_showcase.state_key(), rescue_stress.state_key());
    assert_ne!(
        rescue_showcase.rate_ids().count(),
        rescue_stress.rate_ids().count()
    );
}

#[test]
fn every_micro_problem_remains_a_fast_replayable_contract_fixture() {
    let registry = registry();
    for problem in registry.catalog() {
        let mut request = StudyRequest::new(problem.key.clone())
            .with_instance(InstanceProfile::Micro)
            .with_strategy(problem.default_strategy.clone());
        request.budget = 8;
        let study = registry
            .study(request)
            .unwrap_or_else(|error| panic!("{} micro failed: {error}", problem.key));
        assert_eq!(study.instance.profile, InstanceProfile::Micro);
        let selected = study.selected().expect("selected document");
        assert!(selected.frames.iter().all(|frame| {
            !frame.exchange.rate.label.contains('{')
                && !frame.exchange.rate.label.contains("encoded")
                && frame
                    .cues
                    .first()
                    .is_some_and(|cue| cue.label == frame.exchange.rate.label)
        }));
        if matches!(
            problem.key.as_str(),
            "maze"
                | "bridge"
                | "workshop"
                | "marketplace"
                | "logistics"
                | "mission"
                | "rescue"
                | "work-league"
        ) {
            assert!(
                std::iter::once(&selected.initial)
                    .chain(selected.frames.iter().map(|frame| &frame.after))
                    .filter_map(|snapshot| snapshot.scene.as_ref())
                    .flat_map(|scene| &scene.entities)
                    .any(|entity| !entity.evidence.is_empty()),
                "{} graph has no entity linked to economic evidence",
                problem.key
            );
            assert!(
                has_linked_entity_change(selected),
                "{} graph has no replay-visible linked entity change",
                problem.key
            );
        }
    }
}

#[test]
fn scaled_assets_are_read_in_their_units() {
    let study = registry()
        .study(StudyRequest::new(path!("lending")))
        .expect("lending is studied");
    let document = study.selected().expect("a selected document");
    let key = |asset| {
        axionomy_view::PathOntology::<lending::Ontology>::default()
            .asset(&asset)
            .key
    };
    let base_rate = key(lending::Asset::BaseRate);
    let format = &document.formats[&base_rate];
    let quantity = document
        .initial
        .accounts
        .iter()
        .flat_map(|account| &account.balances)
        .find(|balance| balance.asset.key == base_rate)
        .map(|balance| balance.quantity.0.parse().expect("an exact quantity"))
        .expect("the market holds its base rate");
    assert_eq!(format.write(quantity), "0.2 %");
    assert!(
        document
            .formats
            .contains_key(&key(lending::Asset::BorrowIndex))
    );
    // Counted assets need no entry.
    assert!(!document.formats.contains_key(&key(lending::Asset::Share)));
}

/// Studio labels read as words, not code, and each names one thing: no two
/// different accounts, assets, rates or roles of a document share a label.
fn assert_readable_labels(problem: &str, study: &Study) {
    use axionomy_view::{ItemPath, ViewId};
    use std::collections::{BTreeMap, BTreeSet};
    type Labels<'a> = BTreeMap<(&'static str, &'a str), BTreeSet<&'a ItemPath>>;
    fn name<'a>(labels: &mut Labels<'a>, kind: &'static str, id: &'a ViewId) {
        labels
            .entry((kind, id.label.as_str()))
            .or_default()
            .insert(&id.key);
    }
    let reads_as_code = |label: &str| {
        label.contains("::")
            || label.contains('_')
            || label
                .as_bytes()
                .windows(2)
                .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_uppercase())
    };
    for document in &study.documents {
        let mut labels = Labels::new();
        let snapshots = std::iter::once(&document.initial)
            .chain(document.frames.iter().map(|frame| &frame.after));
        for snapshot in snapshots {
            for account in &snapshot.accounts {
                name(&mut labels, "account", &account.account);
                for balance in &account.balances {
                    name(&mut labels, "asset", &balance.asset);
                }
            }
        }
        for frame in &document.frames {
            for binding in &frame.exchange.bindings {
                name(&mut labels, "role", &binding.role);
            }
        }
        for rate in document.model.iter().flat_map(|model| &model.rates) {
            name(&mut labels, "rate", &rate.rate);
        }
        // Thinking is read as a sequence of sentences, bookkeeping included.
        for observation in &document.thinking {
            let label = observation.label.as_str();
            assert!(
                !reads_as_code(label)
                    && label
                        .chars()
                        .next()
                        .is_some_and(|first| !first.is_lowercase()),
                "{problem}: thinking label `{label}` reads as code"
            );
            assert_eq!(
                observation.phase == path!("documents"),
                observation.kind == axionomy_view::SearchObservationKindView::Document,
                "{problem}: only recorded documents are document observations"
            );
        }
        for ((kind, label), keys) in &labels {
            assert!(
                !reads_as_code(label),
                "{problem}: {kind} label `{label}` reads as code"
            );
            assert!(
                keys.len() == 1,
                "{problem}: {} different {kind}s are all labeled `{label}`: {keys:?}",
                keys.len()
            );
        }
    }
}

#[test]
fn every_default_problem_studies_to_a_selected_replay() {
    let registry = registry();
    for problem in registry.catalog() {
        let study = registry
            .study(
                StudyRequest::new(problem.key.clone())
                    .with_strategy(problem.default_strategy.clone()),
            )
            .unwrap_or_else(|error| panic!("{} failed: {error}", problem.key));
        assert!(study.selected().is_some(), "{}", problem.key);
        assert!(!study.documents.is_empty(), "{}", problem.key);
        assert_eq!(study.instance.profile, InstanceProfile::Showcase);
        assert!(
            study
                .documents
                .iter()
                .all(|document| document.model.is_some()),
            "{}",
            problem.key
        );
        assert_readable_labels(problem.key.as_str(), &study);
        let maximum_frames = study
            .documents
            .iter()
            .map(|document| document.frames.len())
            .max()
            .unwrap_or(0);
        let maximum_accounts = study
            .documents
            .iter()
            .map(|document| document.initial.accounts.len())
            .max()
            .unwrap_or(0);
        let maximum_rates = study
            .documents
            .iter()
            .filter_map(|document| document.model.as_ref())
            .map(|model| model.rates.len())
            .max()
            .unwrap_or(0);
        let (minimum_frames, minimum_accounts, minimum_rates) = match problem.key.as_str() {
            "maze" => (8, 2, 19),
            "sokoban" => (20, 36, 100),
            "exact-cover" => (5, 1, 40),
            "workshop" => (7, 1, 4),
            "scheduling" => (20, 6, 80),
            "rescue" => (7, 2, 70),
            "bridge" => (10, 3, 18),
            "marketplace" => (4, 14, 4),
            "logistics" => (40, 7, 50),
            "connect-four" => (15, 50, 200),
            "mission" => (6, 5, 39),
            "perishables" => (10, 5, 13),
            "work-league" => (60, 20, 900),
            "amm" => (14, 11, 10),
            "lending" => (14, 10, 14),
            other => panic!("missing showcase pressure threshold for {other}"),
        };
        assert!(
            maximum_frames >= minimum_frames,
            "{} trace regressed",
            problem.key
        );
        assert!(
            maximum_accounts >= minimum_accounts,
            "{} account surface regressed",
            problem.key
        );
        assert!(
            maximum_rates >= minimum_rates,
            "{} rate surface regressed",
            problem.key
        );
        assert!(study.documents.iter().all(|document| {
            document
                .telemetry
                .iter()
                .any(|series| series.subject == axionomy_view::TelemetrySubjectView::Model)
        }));
        assert!(study.documents.iter().all(|document| {
            document.initial.scene.is_some()
                && document
                    .frames
                    .iter()
                    .all(|frame| frame.after.scene.is_some() && !frame.cues.is_empty())
                && std::iter::once(&document.initial)
                    .chain(document.frames.iter().map(|frame| &frame.after))
                    .filter_map(|snapshot| snapshot.scene.as_ref())
                    .all(|scene| !scene.metrics.is_empty())
        }));
        assert!(study.documents.iter().all(|document| {
            std::iter::once(&document.initial)
                .chain(document.frames.iter().map(|frame| &frame.after))
                .filter_map(|snapshot| snapshot.scene.as_ref())
                .any(|scene| !scene.entities.is_empty())
        }));
        for document in &study.documents {
            let model = document.model.as_ref().expect("model definition");
            assert_unique(
                problem.key.as_str(),
                "invariant name",
                model
                    .invariants
                    .iter()
                    .map(|invariant| invariant.name.as_str()),
            );
            for scene in std::iter::once(&document.initial)
                .chain(document.frames.iter().map(|frame| &frame.after))
                .filter_map(|snapshot| snapshot.scene.as_ref())
            {
                assert_unique(
                    problem.key.as_str(),
                    "scene entity ID",
                    scene.entities.iter().map(|entity| entity.id.key.as_str()),
                );
                assert_unique(
                    problem.key.as_str(),
                    "scene path ID",
                    scene.paths.iter().map(|path| path.id.as_str()),
                );
                assert_unique(
                    problem.key.as_str(),
                    "scene annotation ID",
                    scene
                        .annotations
                        .iter()
                        .map(|annotation| annotation.id.as_str()),
                );
                if let SceneSurfaceView::Graph { nodes, edges, .. } = &scene.surface {
                    assert_unique(
                        problem.key.as_str(),
                        "graph node ID",
                        nodes.iter().map(|node| node.id.key.as_str()),
                    );
                    assert_unique(
                        problem.key.as_str(),
                        "graph edge ID",
                        edges.iter().map(|edge| edge.id.as_str()),
                    );
                }
            }
        }
        assert!(
            study
                .documents
                .iter()
                .all(|document| !document.thinking.is_empty())
        );
        if problem.key == path!("connect-four") {
            assert!(maximum_rates < 300, "compact standard board regressed");
        }
    }
}

#[test]
fn a_proposal_names_the_snapshot_it_was_assessed_against() {
    let study = registry()
        .study(StudyRequest::new(path!("mission")).with_instance(InstanceProfile::Micro))
        .unwrap();
    let (document, proposal) = study
        .documents
        .iter()
        .flat_map(|document| {
            document
                .proposals
                .iter()
                .map(move |proposal| (document, proposal))
        })
        .find(|(_, proposal)| proposal.id == path!("move-before-sharing"))
        .expect("mission proposes moving before sharing");
    // Assessed after the scan resolved: the snapshot its frame left.
    let index = usize::try_from(proposal.snapshot_index).unwrap();
    assert!(index > 0);
    let scanned = &document.frames[index - 1];
    assert!(
        scanned
            .exchange
            .rate
            .key
            .as_str()
            .starts_with("mission/rate/resolve"),
        "{}",
        scanned.exchange.rate.key
    );
    assert_ne!(
        proposal.assessment.status,
        axionomy_view::AssessmentStatusView::Applicable
    );
}

#[test]
fn a_paused_study_waits_and_resumes_without_restarting_its_command() {
    let control = std::sync::Arc::new(StudyControl::default());
    control.pause();
    let worker_control = std::sync::Arc::clone(&control);
    let worker = std::thread::spawn(move || {
        registry().study_with(
            &StudyRequest::new(path!("maze")).with_strategy(path!("a-star")),
            &worker_control,
            &mut |_| {},
        )
    });
    std::thread::sleep(std::time::Duration::from_millis(25));
    assert!(!worker.is_finished());
    control.resume();
    assert!(worker.join().unwrap().is_ok());
}

#[test]
fn logistics_reports_algorithm_progress_and_observes_cancellation_mid_study() {
    let control = StudyControl::default();
    let mut observed = Vec::new();
    let result = registry().study_with(
        &StudyRequest {
            problem: path!("logistics"),
            instance: Some(InstanceProfile::Micro),
            strategy: Some(path!("reliable")),
            seed: 42,
            budget: 64,
        },
        &control,
        &mut |progress: Progress| {
            if progress.phase == path!("monte-carlo") && progress.completed >= 16 {
                control.cancel();
            }
            observed.push(progress);
        },
    );

    assert_eq!(result, Err(StudyError::Cancelled));
    let monte_carlo = observed
        .iter()
        .filter(|progress| progress.phase == path!("monte-carlo"))
        .collect::<Vec<_>>();
    assert!(monte_carlo.len() >= 2);
    assert!(
        monte_carlo
            .windows(2)
            .all(|pair| pair[0].completed < pair[1].completed)
    );
    assert!(monte_carlo.iter().all(|progress| progress.total == 32));
}

#[test]
fn a_pause_mid_study_blocks_the_next_chunk_until_resume() {
    let control = std::sync::Arc::new(StudyControl::default());
    let resume_control = std::sync::Arc::clone(&control);
    let (paused_tx, paused_rx) = std::sync::mpsc::sync_channel(1);
    let resumer = std::thread::spawn(move || {
        paused_rx.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(resume_control.is_paused());
        resume_control.resume();
    });
    let mut paused_once = false;
    let result = registry().study_with(
        &StudyRequest {
            problem: path!("logistics"),
            instance: Some(InstanceProfile::Micro),
            strategy: Some(path!("reliable")),
            seed: 42,
            budget: 16,
        },
        &control,
        &mut |progress: Progress| {
            if !paused_once && progress.phase == path!("monte-carlo") {
                paused_once = true;
                control.pause();
                paused_tx.send(()).unwrap();
            }
        },
    );
    resumer.join().unwrap();

    assert!(paused_once);
    assert!(result.is_ok());
}
