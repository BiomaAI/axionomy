//! Browser Worker adapter over the same interface-neutral study runtime.
//!
//! This crate does not introduce a JavaScript world model. Requests, solver
//! observations, replay validation, and studies all cross the boundary as
//! Rust-owned contracts. The JavaScript Worker only schedules and transports.
//!
//! The Worker's exports study every sample problem. [`catalog_of`] and
//! [`study_in`] serve any [`Registry`], for a Worker built over other
//! problems.
//!
//! Beside the studies, which retire once Studio no longer plays them, the
//! Worker holds the engine itself: [`WorkerEngine`], exported as `Engine`,
//! over the Worker's store on the origin private file system or in memory
//! (P11, D-047), offering the operations the Studio server offers, the
//! same functions of `axionomy-studio-api` giving the same JSON (P12).

#[cfg(target_arch = "wasm32")]
mod opfs;
mod worker;

pub use worker::{Source, Validators, WorkerEngine};

use axionomy_study::{
    Progress, Registry, StudioEvent, StudioStudyId, StudyControl, StudyObserver, StudyRequest,
};
use axionomy_view::{DocumentPath, ExchangeFrame, SearchObservationView};
use js_sys::Function;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn protocol() -> String {
    "axionomy-studio-worker/1".into()
}

/// The catalog of every sample problem, as JSON.
#[wasm_bindgen]
pub fn catalog() -> Result<String, JsValue> {
    catalog_of(&axionomy_problems::registry())
}

/// Studies a request for a sample problem, emitting its events as JSON,
/// and gives the study as JSON.
#[wasm_bindgen]
pub fn study(request_json: &str, study_id: &str, emit: &Function) -> Result<String, JsValue> {
    study_in(&axionomy_problems::registry(), request_json, study_id, emit)
}

/// The catalog of `registry`, as JSON.
pub fn catalog_of(registry: &Registry) -> Result<String, JsValue> {
    serde_json::to_string(&registry.catalog()).map_err(js_error)
}

/// Studies a request for a problem of `registry`, emitting its events as
/// JSON, and gives the study as JSON.
pub fn study_in(
    registry: &Registry,
    request_json: &str,
    study_id: &str,
    emit: &Function,
) -> Result<String, JsValue> {
    let request: StudyRequest = serde_json::from_str(request_json).map_err(js_error)?;
    let study_id: StudioStudyId = study_id.parse().map_err(js_error)?;
    let key = registry.resolve(&request).map_err(js_error)?;
    let mut observer = WorkerObserver {
        emit,
        study_id,
        next_sequence: 1,
    };
    observer.event(StudioEvent::StudyStarted {
        study_id,
        sequence: 0,
        problem: key.problem,
        strategy: key.strategy,
    })?;
    let study = registry
        .study_with(&request, &StudyControl::default(), &mut observer)
        .map_err(js_error)?;
    serde_json::to_string(&study).map_err(js_error)
}

struct WorkerObserver<'a> {
    emit: &'a Function,
    study_id: StudioStudyId,
    next_sequence: u64,
}

impl WorkerObserver<'_> {
    fn event(&mut self, event: StudioEvent) -> Result<(), JsValue> {
        let event = serde_json::to_string(&event).map_err(js_error)?;
        self.emit
            .call1(&JsValue::NULL, &JsValue::from_str(&event))?;
        Ok(())
    }

    fn sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }
}

impl StudyObserver for WorkerObserver<'_> {
    fn progress(&mut self, progress: Progress) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::Progress {
            study_id: self.study_id,
            sequence,
            phase: progress.phase,
            completed: progress.completed,
            total: Some(progress.total),
            message: progress.message,
        });
    }

    fn observation(&mut self, observation: SearchObservationView) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::Thinking {
            study_id: self.study_id,
            sequence,
            observation,
        });
    }

    fn frame(&mut self, document: &DocumentPath, frame: ExchangeFrame) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::FrameAppended {
            study_id: self.study_id,
            sequence,
            document: document.clone(),
            frame_index: frame.index,
            frame: Box::new(frame),
        });
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
