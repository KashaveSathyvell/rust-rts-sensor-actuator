use common::config::ExperimentConfig;
use common::metrics::MetricsRecorder;

/// Runs the async (Tokio-based) sensor–actuator experiment.
///
/// This function is intentionally unimplemented for now.
/// The full logic will be added later.
pub fn run_experiment(_config: ExperimentConfig) -> MetricsRecorder {
    unimplemented!("Async experiment not implemented yet");
}
