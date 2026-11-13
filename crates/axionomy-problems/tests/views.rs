//! A sample's view as data (P7): maze's view written as a Rhai script draws
//! every document of every maze study exactly as its Rust view does, from a
//! run recorded as maze's and read as paths.

use axionomy::{OntologyPath, path, paths};
use axionomy_engine::{Engine, ScriptCall, View};
use axionomy_problems::{maze, registry};
use axionomy_store::{Checkpoints, Store};
use axionomy_study::{Progress, StudyControl, StudyObserver, StudyRequest};
use axionomy_view::DocumentPath;
use serde::{Serialize, de::DeserializeOwned};

/// `value` written in binary and read back as `T`: an economy or exchange
/// written as paths read as maze's, which D-044 makes the same bytes.
fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

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

fn maze_view(engine: &Engine) {
    let script = path!("maze/view");
    engine
        .keep_script(&script, include_str!("../scripts/maze-view.rhai"))
        .unwrap();
    let call = |function: &str| Some(ScriptCall::new(script.clone(), function.parse().unwrap()));
    engine
        .keep_view(
            &OntologyPath::of::<maze::Ontology>(),
            &View {
                title: "Key-door maze".into(),
                labels: call("label"),
                formats: call("format"),
                carry: None,
                scene: call("scene"),
                leaderboards: None,
            },
        )
        .unwrap();
}

#[test]
fn maze_drawn_by_its_script_is_maze_drawn_by_rust() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    maze_view(&engine);
    let problem = registry()
        .catalog()
        .into_iter()
        .find(|problem| problem.key.as_str() == "maze")
        .unwrap();
    let mut drawn = 0;
    for instance in &problem.instances {
        let request = StudyRequest::new(path!("maze")).with_instance(instance.key.clone());
        let mut observed = Trajectories::default();
        let study = registry()
            .study_with(&request, &StudyControl::default(), &mut observed)
            .unwrap();
        for ((id, initial, trace), document) in observed.0.iter().zip(&study.documents) {
            assert_eq!(id, &document.id);
            let mut recorder = engine
                .store()
                .record::<maze::Ontology>(written(initial), Checkpoints::Ends);
            for exchange in trace.exchanges() {
                recorder.apply(written(exchange)).unwrap();
            }
            let end = recorder.finish().unwrap();
            let scripted = engine.draw(end).unwrap();
            assert_eq!(scripted.formats, document.formats, "{id}");
            assert_eq!(scripted.initial, document.initial, "{id}");
            assert_eq!(scripted.frames.len(), document.frames.len(), "{id}");
            for (scripted, rust) in scripted.frames.iter().zip(&document.frames) {
                assert_eq!(scripted, rust, "{id} frame {}", rust.index);
            }
            drawn += 1;
        }
    }
    assert!(drawn >= 3, "{drawn} documents");
}

#[test]
fn a_run_without_a_view_is_drawn_by_its_paths() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let initial = maze::initial();
    let first: paths::Exchange = written(&maze::candidates(&initial)[0]);
    let start = engine
        .store()
        .record::<maze::Ontology>(initial, Checkpoints::Ends)
        .finish()
        .unwrap();
    let document = engine.draw(start).unwrap();
    assert_eq!(document.title, "maze");
    assert!(document.initial.scene.is_none());
    let agent = &document.initial.accounts[0].account;
    assert_eq!(agent.key.as_str(), "maze/account/agent");
    assert_eq!(agent.label, "account/agent");

    // A fork the engine applies keeps maze's ontology, so maze's view draws
    // it, its history and all.
    maze_view(&engine);
    let forked = engine.apply(start, &[first]).unwrap();
    assert_eq!(engine.ontology(forked).unwrap().as_str(), "maze");
    let document = engine.draw(forked).unwrap();
    assert_eq!(document.title, "Key-door maze");
    assert_eq!(document.frames.len(), 1);
    assert_eq!(document.initial.accounts[0].account.label, "Explorer");
    assert!(document.frames[0].after.scene.is_some());
}
