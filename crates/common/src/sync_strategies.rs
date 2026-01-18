use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use crate::metrics::CycleResult;

/// Trait for synchronization strategies used in benchmarking
pub trait SyncStrategy: Send + Sync {
    fn record(&self, result: CycleResult);
    fn get_results(&self) -> Vec<CycleResult>;
    fn get_missed_deadlines(&self) -> usize;
    fn get_results_count(&self) -> usize;
    fn save_to_csv(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>>;
}

/// Mutex-based synchronization strategy
pub struct MutexStrategy {
    results: Arc<Mutex<Vec<CycleResult>>>,
    missed_deadlines: Arc<AtomicUsize>,
}

impl MutexStrategy {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::with_capacity(10_000))),
            missed_deadlines: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl SyncStrategy for MutexStrategy {
    fn record(&self, result: CycleResult) {
        if let Ok(mut data) = self.results.lock() {
            if !result.deadline_met {
                self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
            }
            data.push(result);
        }
    }

    fn get_results(&self) -> Vec<CycleResult> {
        self.results.lock().unwrap().clone()
    }

    fn get_missed_deadlines(&self) -> usize {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    fn get_results_count(&self) -> usize {
        self.results.lock().unwrap().len()
    }

    fn save_to_csv(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
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

/// RwLock-based synchronization strategy
pub struct RwLockStrategy {
    results: Arc<RwLock<Vec<CycleResult>>>,
    missed_deadlines: Arc<AtomicUsize>,
}

impl RwLockStrategy {
    pub fn new() -> Self {
        Self {
            results: Arc::new(RwLock::new(Vec::with_capacity(10_000))),
            missed_deadlines: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl SyncStrategy for RwLockStrategy {
    fn record(&self, result: CycleResult) {
        if let Ok(mut data) = self.results.write() {
            if !result.deadline_met {
                self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
            }
            data.push(result);
        }
    }

    fn get_results(&self) -> Vec<CycleResult> {
        self.results.read().unwrap().clone()
    }

    fn get_missed_deadlines(&self) -> usize {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    fn get_results_count(&self) -> usize {
        self.results.read().unwrap().len()
    }

    fn save_to_csv(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let data = self.results.read().unwrap();
        let mut wtr = csv::Writer::from_path(filename)?;
        for record in data.iter() {
            wtr.serialize(record)?;
        }
        wtr.flush()?;
        println!("Saved {} records to {}", data.len(), filename);
        Ok(())
    }
}

/// Atomic-based synchronization strategy using atomic operations
/// This strategy uses a more complex approach with atomics for fine-grained locking
pub struct AtomicStrategy {
    results: Arc<Mutex<Vec<CycleResult>>>,
    missed_deadlines: Arc<AtomicUsize>,
    results_count: Arc<AtomicUsize>,
}

impl AtomicStrategy {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::with_capacity(10_000))),
            missed_deadlines: Arc::new(AtomicUsize::new(0)),
            results_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl SyncStrategy for AtomicStrategy {
    fn record(&self, result: CycleResult) {
        if let Ok(mut data) = self.results.lock() {
            if !result.deadline_met {
                self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
            }
            data.push(result);
            self.results_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn get_results(&self) -> Vec<CycleResult> {
        self.results.lock().unwrap().clone()
    }

    fn get_missed_deadlines(&self) -> usize {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    fn get_results_count(&self) -> usize {
        self.results_count.load(Ordering::Relaxed)
    }

    fn save_to_csv(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
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