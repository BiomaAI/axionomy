//! A renderer for Burn's training that shows nothing: Burn's command-line
//! renderer prints a line for every step, and the caller of [`crate::train`]
//! reports what it learned instead.

use burn::train::logger::{EvaluationProgressLogger, TrainingProgressLogger};
use burn::train::metric::MetricDefinition;
use burn::train::renderer::{
    EvaluationName, MetricState, MetricsRenderer, MetricsRendererEvaluation,
    MetricsRendererTraining,
};

pub(crate) struct Silent;

impl MetricsRendererTraining for Silent {
    fn update_train(&mut self, _state: MetricState) {}
    fn update_valid(&mut self, _state: MetricState) {}
    fn on_train_end(
        &mut self,
        _summary: Option<burn::train::LearnerSummary>,
    ) -> Result<(), Box<dyn core::error::Error>> {
        Ok(())
    }
}

impl MetricsRendererEvaluation for Silent {
    fn update_test(&mut self, _name: EvaluationName, _state: MetricState) {}
    fn on_test_end(
        &mut self,
        _summary: Option<burn::train::LearnerSummary>,
    ) -> Result<(), Box<dyn core::error::Error>> {
        Ok(())
    }
}

impl TrainingProgressLogger for Silent {
    fn start(&mut self, _epochs: usize, _from: usize, _items: Option<usize>, _label: Option<&str>) {
    }
    fn update_epoch(&mut self, _epoch: usize) {}
    fn start_split(&mut self, _split: &str, _items: usize) {}
    fn update_split(&mut self, _items: usize) {}
    fn end_split(&mut self) {}
    fn end(&mut self) {}
    fn log_event_training(&mut self, _event: String) {}
}

impl EvaluationProgressLogger for Silent {
    fn start_global_progress(&mut self, _tests: usize) {}
    fn start_test(&mut self, _name: &str, _items: usize) {}
    fn update_test_progress(&mut self, _items: usize) {}
    fn end_test(&mut self) {}
    fn end_global_progress(&mut self) {}
    fn log_event_evaluation(&mut self, _event: String) {}
}

impl MetricsRenderer for Silent {
    fn manual_close(&mut self) {}
    fn register_metric(&mut self, _definition: MetricDefinition) {}
}
