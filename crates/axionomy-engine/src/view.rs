//! Views as data (P7): how people read the runs of an ontology. A view is
//! kept per ontology and names the script functions that label its
//! identifiers, format its quantities and draw each frame; the engine draws
//! any point's history with the view of its run's ontology, or with its
//! paths when the store keeps none.

use crate::{Engine, EngineError, ScriptCall, ScriptState, definitions::kind};
use axionomy::{
    AccountPath, AssetPath, OntologyPath, RatePath, RolePath,
    path::Path,
    paths::{self, Paths},
};
use axionomy_store::Point;
use axionomy_view::{
    DocumentPath, ItemPath, LeaderboardEntryView, LeaderboardView, MeasurePath,
    ObjectiveDirectionView, QuantityFormat, Scene, SceneMetricView, ViewDocument,
    ViewDocumentMetadata, ViewId, ViewOntology, ViewSource, derive_document, derive_model,
};
use rhai::{Array, Dynamic, Map};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::cell::RefCell;

/// The name a view is kept under: the ontology it shows, such as `maze`.
pub type ViewPath = Path<kind::View>;

/// How people read an ontology's runs. Every part is a script function, and
/// every part is optional: a view without labels shows each identifier's
/// path, one without formats shows counts, and one without a scene shows
/// balances alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// What people call the ontology, such as `Key-door maze`.
    pub title: String,
    /// `fn(item)`: what people call an account, asset, rate or role, written
    /// `account/<path>`, `asset/<path>`, `rate/<path>` or `role/<path>`, or
    /// nothing for its path.
    pub labels: Option<ScriptCall>,
    /// `fn(asset)`: how people read quantities of an asset, written by its
    /// path, as `#{ scale, digits, unit }`, each optional, or nothing for a
    /// count.
    pub formats: Option<ScriptCall>,
    /// `fn(carried, before, exchange, after)`: what the view carries from
    /// each frame to the next, starting from nothing, such as the routes a
    /// trace has taken or each actor's running contribution.
    pub carry: Option<ScriptCall>,
    /// `fn(state, frame)`: the scene of a frame, as a map shaped like
    /// [`Scene`], or nothing. `frame` is `#{ index, exchanges, carried }`:
    /// the frame's step counted from the history's start, every exchange of
    /// the history drawn, and what the view carried to it.
    pub scene: Option<ScriptCall>,
    /// `fn(state, frame)`: leaderboards, an array of
    /// `#{ key, label, description, direction, entries }`, each entry
    /// `#{ participant: #{ key, label }, score, value, unit, eligible,
    /// components }`. The engine ranks eligible entries by their integer
    /// `score` in the board's direction, ties sharing a rank.
    pub leaderboards: Option<ScriptCall>,
}

impl View {
    fn calls(&self) -> impl Iterator<Item = &ScriptCall> {
        [
            &self.labels,
            &self.formats,
            &self.carry,
            &self.scene,
            &self.leaderboards,
        ]
        .into_iter()
        .flatten()
    }
}

impl Engine {
    /// Keeps `view` as how people read the runs of `ontology`, refusing one
    /// whose scripts are not kept.
    pub fn keep_view(&self, ontology: &OntologyPath, view: &View) -> Result<(), EngineError> {
        for call in view.calls() {
            self.require_script(&call.script)?;
        }
        self.define(&view_path(ontology), view)
    }

    /// The view kept for `ontology`, if any.
    pub fn view(&self, ontology: &OntologyPath) -> Result<Option<View>, EngineError> {
        match self.defined(&view_path(ontology)) {
            Ok(view) => Ok(Some(view)),
            Err(EngineError::Undefined(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The ontology the run of `at` was written from.
    pub fn ontology(&self, at: Point) -> Result<OntologyPath, EngineError> {
        Ok(self
            .store
            .read()?
            .run(at.run)?
            .ok_or(EngineError::UnknownPoint(at))?
            .ontology)
    }

    /// The history of `at` drawn as a document: its first state and a frame
    /// for every exchange since, shown by the view of its run's ontology, or
    /// by its paths when there is none.
    pub fn draw(&self, at: Point) -> Result<ViewDocument, EngineError> {
        let ontology = self.ontology(at)?;
        let view = self.view(&ontology)?;
        let (initial, exchanges) = self.history(at)?;
        let trace: paths::Trace = exchanges.iter().cloned().collect();
        let drawing = Drawing::new(self, ontology.clone(), view.as_ref(), &exchanges);
        let title = view
            .as_ref()
            .map_or_else(|| ontology.to_string(), |view| view.title.clone());
        let metadata = ViewDocumentMetadata {
            id: format!("point/{}/{}", at.run, at.step)
                .parse::<DocumentPath>()
                .expect("a run and a step make a path"),
            title: title.clone(),
            description: String::new(),
            source: ViewSource {
                key: ontology.as_str().parse().expect("an ontology is a path"),
                label: title,
            },
        };
        let document = derive_document(metadata, &initial, &trace, &drawing, Vec::new());
        drawing.failed()?;
        let mut document = document.map_err(|error| {
            EngineError::Store(axionomy_store::StoreError::Malformed(match error {
                axionomy_view::PlaybackError::Replay { .. } => "a history that does not replay",
            }))
        })?;
        document.model = Some(derive_model(&initial, &paths::Goal::new(), &drawing));
        drawing.failed()?;
        Ok(document)
    }
}

pub(crate) fn view_path(ontology: &OntologyPath) -> ViewPath {
    ontology.as_str().parse().expect("an ontology is a path")
}

/// A history being drawn: the view's script calls, what it has carried so
/// far, and the first script failure, which ends the drawing.
struct Drawing<'a> {
    engine: &'a Engine,
    ontology: OntologyPath,
    view: Option<&'a View>,
    /// Every exchange of the history, shared with each frame's script call.
    exchanges: Dynamic,
    exchange_list: &'a [paths::Exchange],
    carried: RefCell<Carried>,
    failure: RefCell<Option<EngineError>>,
}

/// What the view carried to the frame at `index`, and that frame's state.
struct Carried {
    index: u64,
    economy: Option<paths::Economy>,
    value: Dynamic,
}

impl<'a> Drawing<'a> {
    fn new(
        engine: &'a Engine,
        ontology: OntologyPath,
        view: Option<&'a View>,
        exchanges: &'a [paths::Exchange],
    ) -> Self {
        let list: Array = exchanges.iter().cloned().map(Dynamic::from).collect();
        Self {
            engine,
            ontology,
            view,
            exchanges: Dynamic::from_array(list).into_shared(),
            exchange_list: exchanges,
            carried: RefCell::new(Carried {
                index: 0,
                economy: None,
                value: Dynamic::UNIT,
            }),
            failure: RefCell::new(None),
        }
    }

    fn failed(&self) -> Result<(), EngineError> {
        match self.failure.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Keeps the first failure; later calls do nothing once one failed.
    fn attempt<T>(&self, call: impl FnOnce() -> Result<T, EngineError>) -> Option<T> {
        if self.failure.borrow().is_some() {
            return None;
        }
        call()
            .map_err(|error| {
                self.failure.borrow_mut().get_or_insert(error);
            })
            .ok()
    }

    fn id(&self, category: &str, path: &impl std::fmt::Display) -> ViewId {
        let item = format!("{category}/{path}");
        let key: ItemPath = format!("{}/{item}", self.ontology)
            .parse()
            .expect("an ontology, a category and a path make a path");
        let label = self
            .view
            .and_then(|view| view.labels.as_ref())
            .and_then(|labels| {
                self.attempt(|| {
                    let label: Dynamic = self.engine.call(
                        &labels.script,
                        labels.function.as_str(),
                        (item.clone(),),
                    )?;
                    text(label, "a label")
                })
            })
            .flatten()
            .unwrap_or(item);
        ViewId::new(key, label)
    }

    /// The frame map a scene or leaderboards call reads for the state at
    /// `index`, carrying the view to it first.
    fn frame(&self, index: u64, economy: &paths::Economy) -> Option<Map> {
        let mut carried = self.carried.borrow_mut();
        if carried.economy.is_none() || carried.index != index {
            let value = match (&carried.economy, index.checked_sub(1)) {
                (Some(before), Some(previous)) if previous == carried.index => {
                    match self.view.and_then(|view| view.carry.as_ref()) {
                        Some(carry) => self.attempt(|| {
                            let exchange = self
                                .exchange_list
                                .get(usize::try_from(previous).unwrap_or(usize::MAX))
                                .cloned()
                                .ok_or(EngineError::Returned {
                                    expected: "a frame of the history",
                                    found: index.to_string(),
                                })?;
                            self.engine.call::<Dynamic>(
                                &carry.script,
                                carry.function.as_str(),
                                (
                                    carried.value.clone(),
                                    ScriptState::new(before.clone()),
                                    exchange,
                                    ScriptState::new(economy.clone()),
                                ),
                            )
                        })?,
                        None => Dynamic::UNIT,
                    }
                }
                _ => Dynamic::UNIT,
            };
            *carried = Carried {
                index,
                economy: Some(economy.clone()),
                value,
            };
        }
        let mut frame = Map::new();
        frame.insert(
            "index".into(),
            Dynamic::from(i64::try_from(index).unwrap_or(i64::MAX)),
        );
        frame.insert("exchanges".into(), self.exchanges.clone());
        frame.insert("carried".into(), carried.value.clone());
        Some(frame)
    }

    fn draw<T: DeserializeOwned>(
        &self,
        call: Option<&ScriptCall>,
        index: u64,
        economy: &paths::Economy,
    ) -> Option<T> {
        let call = call?;
        let frame = self.frame(index, economy)?;
        self.attempt(|| {
            let drawn: Dynamic = self.engine.call(
                &call.script,
                call.function.as_str(),
                (ScriptState::new(economy.clone()), frame),
            )?;
            if drawn.is_unit() {
                return Ok(None);
            }
            rhai::serde::from_dynamic(&drawn)
                .map(Some)
                .map_err(|error| EngineError::Script {
                    path: call.script.clone(),
                    message: format!("`{}` drew {error}", call.function),
                })
        })
        .flatten()
    }
}

/// A script's text, or nothing when it returned nothing.
fn text(value: Dynamic, expected: &'static str) -> Result<Option<String>, EngineError> {
    if value.is_unit() {
        return Ok(None);
    }
    let found = value.type_name();
    value
        .into_string()
        .map(Some)
        .map_err(|_| EngineError::Returned {
            expected,
            found: found.to_owned(),
        })
}

/// A leaderboard as a view's script draws it, before the engine ranks it.
#[derive(Deserialize)]
struct Board {
    key: MeasurePath,
    label: String,
    #[serde(default)]
    description: String,
    direction: ObjectiveDirectionView,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    participant: ViewId,
    score: i64,
    value: String,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default = "eligible")]
    eligible: bool,
    #[serde(default)]
    components: Vec<SceneMetricView>,
}

fn eligible() -> bool {
    true
}

impl Board {
    /// Eligible entries by score in the board's direction, in the order
    /// drawn on a tie, ranked one-based with ties sharing a rank; then the
    /// ineligible, unranked.
    fn ranked(self) -> LeaderboardView {
        let (mut ranked, unranked): (Vec<_>, Vec<_>) =
            self.entries.into_iter().partition(|entry| entry.eligible);
        ranked.sort_by_key(|entry| match self.direction {
            ObjectiveDirectionView::Maximize => -i128::from(entry.score),
            ObjectiveDirectionView::Minimize => i128::from(entry.score),
        });
        let mut entries = Vec::with_capacity(ranked.len() + unranked.len());
        let mut previous: Option<(i64, u64)> = None;
        for (index, entry) in ranked.into_iter().enumerate() {
            let rank = match previous {
                Some((score, rank)) if score == entry.score => rank,
                _ => index as u64 + 1,
            };
            previous = Some((entry.score, rank));
            entries.push(entry.view(Some(rank)));
        }
        entries.extend(unranked.into_iter().map(|entry| entry.view(None)));
        LeaderboardView {
            key: self.key,
            label: self.label,
            description: self.description,
            direction: self.direction,
            entries,
        }
    }
}

impl Entry {
    fn view(self, rank: Option<u64>) -> LeaderboardEntryView {
        LeaderboardEntryView {
            rank,
            participant: self.participant,
            value: self.value,
            unit: self.unit,
            eligible: self.eligible,
            components: self.components,
        }
    }
}

impl ViewOntology<Paths> for Drawing<'_> {
    fn account(&self, id: &AccountPath) -> ViewId {
        self.id("account", id)
    }

    fn asset(&self, id: &AssetPath) -> ViewId {
        self.id("asset", id)
    }

    fn rate(&self, id: &RatePath) -> ViewId {
        self.id("rate", id)
    }

    fn role(&self, id: &RolePath) -> ViewId {
        self.id("role", id)
    }

    fn format(&self, asset: &AssetPath) -> QuantityFormat {
        self.view
            .and_then(|view| view.formats.as_ref())
            .and_then(|formats| {
                self.attempt(|| {
                    let format: Dynamic = self.engine.call(
                        &formats.script,
                        formats.function.as_str(),
                        (asset.to_string(),),
                    )?;
                    if format.is_unit() {
                        return Ok(None);
                    }
                    rhai::serde::from_dynamic(&format)
                        .map(Some)
                        .map_err(|error| EngineError::Script {
                            path: formats.script.clone(),
                            message: format!(
                                "`{}` formatted `{asset}` as {error}",
                                formats.function
                            ),
                        })
                })
            })
            .flatten()
            .unwrap_or(QuantityFormat::COUNT)
    }

    fn scene(&self, index: u64, economy: &paths::Economy) -> Option<Scene> {
        self.draw(
            self.view.and_then(|view| view.scene.as_ref()),
            index,
            economy,
        )
    }

    fn leaderboards(&self, index: u64, economy: &paths::Economy) -> Vec<LeaderboardView> {
        self.draw::<Vec<Board>>(
            self.view.and_then(|view| view.leaderboards.as_ref()),
            index,
            economy,
        )
        .map(|boards| boards.into_iter().map(Board::ranked).collect())
        .unwrap_or_default()
    }
}

type Failure = Box<rhai::EvalAltResult>;

/// What a value a script built reads as, or why it does not.
fn read<T: DeserializeOwned>(value: Dynamic) -> Result<T, Failure> {
    rhai::serde::from_dynamic(&value)
}

/// An optional value: nothing, or what it reads as.
fn optional<T: DeserializeOwned>(value: Dynamic) -> Result<Option<T>, Failure> {
    if value.is_unit() {
        Ok(None)
    } else {
        read(value).map(Some)
    }
}

/// The `scene` module a view's script builds on, so a scripted scene infers
/// the entities, paths and legend of its surface as every view does:
/// `scene::graph(title, nodes, edges, focus)`, `scene::grid(title, width,
/// height, cells)`, `scene::matrix(title, rows, columns, cells)` and
/// `scene::timeline(title, lanes, spans, cursor)`, each returning a scene map
/// the script may change before it returns it. Only a view calls them; a
/// script that does not leaves the module alone.
pub(crate) fn register(rhai: &mut rhai::Engine) {
    let mut scene = rhai::Module::new();
    scene.set_native_fn(
        "graph",
        |title: &str, nodes: Array, edges: Array, focus: Dynamic| -> Result<Dynamic, Failure> {
            rhai::serde::to_dynamic(Scene::graph(
                title,
                read(nodes.into())?,
                read(edges.into())?,
                optional(focus)?,
            ))
        },
    );
    scene.set_native_fn(
        "grid",
        |title: &str, width: i64, height: i64, cells: Array| -> Result<Dynamic, Failure> {
            rhai::serde::to_dynamic(Scene::grid(
                title,
                read(width.into())?,
                read(height.into())?,
                read(cells.into())?,
            ))
        },
    );
    scene.set_native_fn(
        "matrix",
        |title: &str, rows: Array, columns: Array, cells: Array| -> Result<Dynamic, Failure> {
            rhai::serde::to_dynamic(Scene::matrix(
                title,
                read(rows.into())?,
                read(columns.into())?,
                read(cells.into())?,
            ))
        },
    );
    scene.set_native_fn(
        "timeline",
        |title: &str, lanes: Array, spans: Array, cursor: Dynamic| -> Result<Dynamic, Failure> {
            rhai::serde::to_dynamic(Scene::timeline(
                title,
                read(lanes.into())?,
                read(spans.into())?,
                optional(cursor)?,
            ))
        },
    );
    rhai.register_static_module("scene", scene.into());
}
