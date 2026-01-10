use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::{SensorData, ActuatorFeedback, ActuatorType};

/// Real-time data point for dashboard visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub timestamp: u64,
    pub sensor_data: Option<SensorData>,
    pub actuator_feedback: Option<(ActuatorType, ActuatorFeedback)>,
    pub metrics: Option<MetricsSnapshot>,
}

/// Snapshot of current system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub cycle_id: u64,
    pub processing_time_ns: u64,
    pub lock_wait_ns: u64,
    pub total_latency_ns: u64,
    pub deadline_met: bool,
    pub lateness_ns: i64,
}

/// Thread-safe dashboard data buffer
#[derive(Clone)]
pub struct DashboardBuffer {
    data: Arc<Mutex<Vec<DashboardData>>>,
    max_size: usize,
}

impl DashboardBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            max_size,
        }
    }

    pub fn add(&self, item: DashboardData) {
        let mut buffer = self.data.lock().unwrap();
        buffer.push(item);
        
        // Keep only the most recent data
        if buffer.len() > self.max_size {
            buffer.remove(0);
        }
    }

    pub fn get_recent(&self, count: usize) -> Vec<DashboardData> {
        let buffer = self.data.lock().unwrap();
        let start = buffer.len().saturating_sub(count);
        buffer[start..].to_vec()
    }

    pub fn get_all(&self) -> Vec<DashboardData> {
        self.data.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.data.lock().unwrap().clear();
    }

    pub fn len(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}






