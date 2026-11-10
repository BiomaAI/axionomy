//! Browser Worker adapter over the same interface-neutral reference service.
//!
//! This crate does not introduce a JavaScript world model. Requests, solver
//! observations, replay validation, and studies all cross the boundary as
//! Rust-owned contracts. The JavaScript Worker only schedules and transports.

use axionomy_service::{
    ReferenceService, ServiceProgress, StudioEvent, StudioStudyId, StudyControl, StudyObserver,
    StudyRequest,
};
use axionomy_view::{DocumentPath, ExchangeFrame, SearchObservationView};
use js_sys::Function;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn protocol() -> String {
    "axionomy-studio-worker/1".into()
}

#[wasm_bindgen]
pub fn catalog() -> Result<String, JsValue> {
    serde_json::to_string(&ReferenceService.catalog()).map_err(js_error)
}

#[wasm_bindgen]
pub fn study(request_json: &str, study_id: &str, emit: &Function) -> Result<String, JsValue> {
    let request: StudyRequest = serde_json::from_str(request_json).map_err(js_error)?;
    let study_id: StudioStudyId = study_id.parse().map_err(js_error)?;
    let key = ReferenceService.resolve(&request).map_err(js_error)?;
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
    let study = ReferenceService
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
    fn progress(&mut self, progress: ServiceProgress) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::Progress {
            study_id: self.study_id,
            sequence,
            completed: progress.completed,
            total: Some(progress.total),
            message: format!("{}: {}", progress.phase, progress.message),
        });
    }

    fn observation(&mut self, observation: SearchObservationView) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::SearchObservation {
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
