use std::sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex};
use std::time::Instant;
use serde::Serialize;
use crate::ActuatorType;

#[derive(Debug, Serialize, Clone)]
pub struct CycleResult {
    pub cycle_id: u64,
    pub mode: String,
    pub actuator: Option<ActuatorType>,
    pub total_latency_ns: u64,
    pub processing_time_ns: u64,
    pub lock_wait_ns: u64,
    pub deadline_met: bool,
    pub lateness_ns: i64,
}

/// Thread-safe recorder with Internal Mutability.
/// You can clone this struct cheaply (it clones the Arcs, not the data).
#[derive(Clone)]
pub struct BenchmarkRecorder {
    // The Mutex is INSIDE, so users don't need to wrap the struct
    results: Arc<Mutex<Vec<CycleResult>>>,
    pub missed_deadlines: Arc<AtomicUsize>,
    #[allow(dead_code)]
    start_time: Instant,
}

impl BenchmarkRecorder {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::with_capacity(10_000))),
            missed_deadlines: Arc::new(AtomicUsize::new(0)),
            start_time: Instant::now(),
        }
    }

    pub fn record(&self, result: CycleResult) {
        // We handle the locking here, internally.
        // This is the "Shared Resource" access for the assignment.
        // Note: lock_wait_ns should be measured by the caller before calling record()
        // This method measures the actual lock acquisition time
        let _lock_start = std::time::Instant::now();
        if let Ok(mut data) = self.results.lock() {
            // The lock_wait_ns in result is measured by caller before calling record()
            if !result.deadline_met {
                self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
            }
            data.push(result);
            // Lock is released here when 'data' goes out of scope
        }
    }

    pub fn get_results(&self) -> Vec<CycleResult> {
        self.results.lock().unwrap().clone()
    }

    pub fn save_to_csv(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let data = self.results.lock().unwrap();
        let mut wtr = csv::Writer::from_path(filename)?;
        for record in data.iter() {
            wtr.serialize(record)?;
        }
        wtr.flush()?;
        println!("Saved {} records to {}", data.len(), filename);
        Ok(())
    }
}