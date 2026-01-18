use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::{SensorData, ActuatorType, ActuatorFeedback};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub cycle_id: u64,
    pub processing_time_ns: u64,
    pub lock_wait_ns: u64,
    pub total_latency_ns: u64,
    pub deadline_met: bool,
    pub lateness_ns: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub timestamp: u64,
    pub sensor_data: Option<SensorData>,
    pub actuator_feedback: Option<(ActuatorType, ActuatorFeedback)>,
    pub metrics: Option<MetricsSnapshot>,
}

#[derive(Debug, Clone)]
pub struct DashboardBuffer {
    buffer: Arc<Mutex<VecDeque<DashboardData>>>,
    capacity: usize,
}

impl DashboardBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn add(&self, data: DashboardData) {
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.len() >= self.capacity {
            buffer.pop_front();
        }
        buffer.push_back(data);
    }

    pub fn get_all(&self) -> Vec<DashboardData> {
        let buffer = self.buffer.lock().unwrap();
        buffer.iter().cloned().collect()
    }

    pub fn get_recent(&self, count: usize) -> Vec<DashboardData> {
        let buffer = self.buffer.lock().unwrap();
        buffer.iter().rev().take(count).cloned().collect()
    }

    pub fn clear(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
    }

    pub fn len(&self) -> usize {
        let buffer = self.buffer.lock().unwrap();
        buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        let buffer = self.buffer.lock().unwrap();
        buffer.is_empty()
    }
}