//! Browser Worker adapter over the same interface-neutral reference service.
//!
//! This crate does not introduce a JavaScript world model. Requests, solver
//! observations, replay validation, and artifacts all cross the boundary as
//! Rust-owned contracts. The JavaScript Worker only schedules and transports.

use axionomy_service::{ReferenceService, RunControl, RunObserver, RunRequest, ServiceProgress};
use axionomy_view::{SearchObservationView, StudioEvent};
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
pub fn run(request_json: &str, run_id: &str, emit: &Function) -> Result<String, JsValue> {
    let request: RunRequest = serde_json::from_str(request_json).map_err(js_error)?;
    let problem = ReferenceService
        .problem(&request.problem)
        .ok_or_else(|| JsValue::from_str("unknown problem"))?;
    let strategy = request
        .strategy
        .clone()
        .unwrap_or_else(|| problem.default_strategy.clone());
    let mut observer = WorkerObserver {
        emit,
        run_id,
        next_sequence: 1,
    };
    observer.event(StudioEvent::RunStarted {
        run_id: run_id.into(),
        sequence: 0,
        problem: request.problem.clone(),
        strategy,
    })?;
    let artifact = ReferenceService
        .run_with(&request, &RunControl::default(), &mut observer)
        .map_err(js_error)?;
    serde_json::to_string(&artifact).map_err(js_error)
}

struct WorkerObserver<'a> {
    emit: &'a Function,
    run_id: &'a str,
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

impl RunObserver for WorkerObserver<'_> {
    fn progress(&mut self, progress: ServiceProgress) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::Progress {
            run_id: self.run_id.into(),
            sequence,
            completed: progress.completed,
            total: Some(progress.total),
            message: format!("{}: {}", progress.phase, progress.message),
        });
    }

    fn observation(&mut self, observation: SearchObservationView) {
        let sequence = self.sequence();
        let _ = self.event(StudioEvent::SearchObservation {
            run_id: self.run_id.into(),
            sequence,
            observation,
        });
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
