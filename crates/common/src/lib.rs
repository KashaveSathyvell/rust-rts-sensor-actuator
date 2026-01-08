use serde::{Deserialize, Serialize};

pub mod metrics;
pub mod pid;
pub mod config;
pub mod diagnostics;

pub use metrics::BenchmarkRecorder;
pub use config::ExperimentConfig;
pub use diagnostics::SharedDiagnostics;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SensorData {
    pub id: u64,
    pub timestamp: u64,
    pub force: f64,
    pub position: f64,
    pub temperature: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ActuatorCommand {
    pub sensor_id: u64,
    pub action_value: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Feedback {
    pub sensor_id: u64,
    pub emergency: bool,
    pub timestamp: u64,
}
