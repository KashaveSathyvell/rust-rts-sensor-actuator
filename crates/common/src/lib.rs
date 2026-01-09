use serde::{Deserialize, Serialize};

pub mod metrics;
pub mod pid;
pub mod config;
pub mod diagnostics;
pub mod sync_strategies;
pub mod dashboard;

pub use metrics::BenchmarkRecorder;
pub use config::ExperimentConfig;
pub use diagnostics::SharedDiagnostics;
pub use dashboard::{DashboardBuffer, DashboardData, MetricsSnapshot};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ActuatorType {
    Gripper,
    Motor,
    Stabilizer,
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Feedback {
    pub sensor_id: u64,
    pub emergency: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ActuatorStatus {
    Normal,
    Correcting,
    Emergency,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ActuatorFeedback {
    pub sensor_id: u64,
    pub status: ActuatorStatus,
    pub control_output: f64,
    pub error: f64,
    pub timestamp: u64,
}