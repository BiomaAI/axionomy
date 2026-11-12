//! A problem registered from outside `axionomy-problems`, with a family, a
//! capability and an instance of its own, is listed and studied end to end
//! over HTTP in the words it declares. Nothing in the server, or in the Studio
//! that reads it, knows the problem in advance.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, Identifier, Ontology, Quantity, Rate, Trace,
    basket, path,
};
use axionomy_studio_server::{ProblemList, StudioState, StudyStatus, StudySummary, api};
use axionomy_study::{
    Capability, DocumentSpec, InstanceDescriptor, Problem, ProblemDescriptor, ProblemFamily,
    StrategyDescriptor, Study, StudyError, StudyKey, StudyProgress, StudyRequest, Work, document,
    problem_error,
};
use axionomy_view::{Labeled, Scene, SearchObservationKindView, ViewDocument};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Bakery {}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
enum Holder {
    Baker,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
enum Good {
    Flour,
    Bread,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
enum Law {
    Bake,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
enum Part {
    Oven,
}

impl Ontology for Bakery {
    const NAME: &'static str = "bakery";
    type Account = Holder;
    type Asset = Good;
    type Rate = Law;
    type Role = Part;
}

axionomy_view::debug_labels!(Holder, Good, Law, Part);
axionomy_view::counted!(Good);

/// Bakes a tray of loaves from flour, one loaf an exchange. Its family,
/// capability and only instance are its own: no front end has heard of them.
struct Baking(ProblemDescriptor);

impl Default for Baking {
    fn default() -> Self {
        Self(ProblemDescriptor {
            key: path!("bakery"),
            title: "Corner bakery".into(),
            summary: "Turn flour into bread before the shop opens.".into(),
            family: ProblemFamily::new(path!("home-baking"), "Home baking"),
            default_instance: path!("one-tray"),
            instances: vec![InstanceDescriptor {
                key: path!("one-tray"),
                label: "One tray".into(),
                description: "Three units of flour and one oven".into(),
            }],
            default_strategy: path!("whole-tray"),
            strategies: vec![StrategyDescriptor {
                key: path!("whole-tray"),
                label: "Bake the whole tray".into(),
                description: "Bake a loaf from every unit of flour.".into(),
                algorithm: "batch baking".into(),
            }],
            capabilities: vec![Capability::new(path!("proofing"), "Proofing dough")],
        })
    }
}

fn no_scene(_: u64, _: &Economy<Bakery>) -> Option<Scene> {
    None
}

impl Problem for Baking {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.0
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, StudyError> {
        let work = Work::new(
            Labeled::new(path!("kneading"), "Kneading the dough"),
            Labeled::new(path!("by-hand"), "Kneading by hand"),
            SearchObservationKindView::Rollout,
        );
        let flour = 3;
        for loaf in 1..=flour {
            let _ = progress.emit(&work, loaf, flour, format!("Kneaded loaf {loaf}"));
            progress.checkpoint()?;
        }
        let initial = EconomyBuilder::<Bakery>::new()
            .account(Holder::Baker, Account::from(basket([(Good::Flour, flour)])))
            .rate(
                Law::Bake,
                Rate::new()
                    .consume(Part::Oven, basket([(Good::Flour, 1)]))
                    .produce(Part::Oven, basket([(Good::Bread, 1)])),
            )
            .build()
            .map_err(|error| problem_error(&key.problem, error))?;
        let mut trace = Trace::new();
        for _ in 0..flour {
            trace.push(Exchange::new(Law::Bake, Quantity::new(1)).bind(Part::Oven, Holder::Baker));
        }
        let tray = document(
            progress,
            DocumentSpec {
                problem: &key.problem,
                document: path!("whole-tray"),
                title: "Bakery · the whole tray",
                description: "Every unit of flour baked into a loaf",
                source_label: "Corner bakery",
            },
            &initial,
            &Goal::new(),
            &trace,
            Vec::new(),
            no_scene,
        )
        .map_err(|error| problem_error(&key.problem, error))?;
        Ok(vec![tray])
    }
}

async fn json<T: DeserializeOwned>(app: &Router, request: Request<Body>) -> (StatusCode, T) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn post(request: &StudyRequest) -> Request<Body> {
    Request::post("/api/studies")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(request).unwrap()))
        .unwrap()
}

fn get(uri: String) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn a_problem_of_your_own_is_listed_and_studied_in_its_own_words() {
    let mut registry = axionomy_problems::registry();
    registry.register(Baking::default()).unwrap();
    let (app, _) = api(StudioState::new(registry));

    // The catalog lists it after the canonical problems, as it declared itself.
    let (status, catalog) = json::<ProblemList>(&app, get("/api/problems".into())).await;
    assert_eq!(status, StatusCode::OK);
    let bakery = catalog.problems.last().unwrap();
    assert_eq!(bakery, Baking::default().descriptor());
    assert_eq!(bakery.family.label, "Home baking");
    assert_eq!(bakery.capabilities[0].label, "Proofing dough");
    assert_eq!(bakery.instances[0].label, "One tray");

    // An instance it does not declare is refused where it is read.
    let canonical = StudyRequest::new(path!("bakery")).with_instance(path!("showcase"));
    let (status, refusal) = json::<serde_json::Value>(&app, post(&canonical)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        refusal["error"],
        "problem `bakery` has no instance `showcase`"
    );

    // Its own instance is studied, and the study speaks in its words.
    let request = StudyRequest::new(path!("bakery")).with_instance(path!("one-tray"));
    let (status, created) = json::<StudySummary>(&app, post(&request)).await;
    assert_eq!(status, StatusCode::OK);
    let summary = loop {
        let (_, summary) =
            json::<StudySummary>(&app, get(format!("/api/studies/{}", created.id))).await;
        if summary.status != StudyStatus::Studying {
            break summary;
        }
        tokio::task::yield_now().await;
    };
    assert_eq!(
        summary.status,
        StudyStatus::Completed,
        "{:?}",
        summary.message
    );
    let (_, study) = json::<Study>(&app, get(format!("/api/studies/{}/study", created.id))).await;
    assert_eq!(study.key.instance, path!("one-tray"));
    assert_eq!(study.instance.label, "One tray");
    assert_eq!(study.problem.family.label, "Home baking");
    let tray = study.selected().unwrap();
    assert_eq!(tray.frames.len(), 3);
    assert!(tray.thinking.iter().any(|observation| {
        observation.phase.label == "Kneading the dough"
            && observation.algorithm.label == "Kneading by hand"
    }));
    assert!(tray.thinking.iter().any(|observation| {
        observation.label == "Preparing the One tray instance of Corner bakery"
    }));
}
