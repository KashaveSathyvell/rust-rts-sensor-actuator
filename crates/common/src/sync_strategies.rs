use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use std::hint::black_box;
use crate::metrics::CycleResult;

/// Synchronization strategy trait for benchmarking different approaches
pub trait SyncStrategy: Send + Sync {
    fn record(&self, result: CycleResult);
    fn get_missed_deadlines(&self) -> usize;
    fn get_results_count(&self) -> usize;
    fn get_results(&self) -> Vec<CycleResult>;
    fn save_to_csv(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>>;
}

/// Strategy 1: Mutex-based synchronization
#[derive(Clone)]
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
        // Measure lock acquisition latency - time from before lock() until lock is acquired
        let lock_start = Instant::now();

        if let Ok(mut data) = self.results.lock() {
            let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;
            black_box(&mut *data); // Prevent optimization of lock operations

            if !result.deadline_met {
                self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
            }

            // Use measured lock wait time instead of synthetic value
            let mut result_with_lock_time = result;
            result_with_lock_time.lock_wait_ns = lock_wait_ns;

            data.push(result_with_lock_time);
        }
    }

    fn get_missed_deadlines(&self) -> usize {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    fn get_results_count(&self) -> usize {
        self.results.lock().map(|r| r.len()).unwrap_or(0)
    }

    fn get_results(&self) -> Vec<CycleResult> {
        self.results.lock().unwrap().clone()
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

/// Strategy 2: RwLock-based synchronization (allows concurrent reads)
#[derive(Clone)]
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
        // Measure lock acquisition latency - time from before write() until write lock is acquired
        let lock_start = Instant::now();

        if let Ok(mut data) = self.results.write() {
            let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;
            black_box(&mut *data); // Prevent optimization of lock operations

            if !result.deadline_met {
                self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
            }

            // Use measured lock wait time instead of synthetic value
            let mut result_with_lock_time = result;
            result_with_lock_time.lock_wait_ns = lock_wait_ns;

            data.push(result_with_lock_time);
        }
    }

    fn get_missed_deadlines(&self) -> usize {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    fn get_results_count(&self) -> usize {
        self.results.read().map(|r| r.len()).unwrap_or(0)
    }

    fn get_results(&self) -> Vec<CycleResult> {
        self.results.read().unwrap().clone()
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

/// Strategy 3: Lock-free atomic-based approach (for simple counters)
/// Note: Full lock-free recording requires more complex structures (e.g., lock-free queues)
/// This is a simplified version that uses atomics for counters
#[derive(Clone)]
pub struct AtomicStrategy {
    results: Arc<Mutex<Vec<CycleResult>>>, // Still need mutex for Vec, but minimize contention
    missed_deadlines: Arc<AtomicUsize>,
    total_cycles: Arc<AtomicUsize>,
}

impl AtomicStrategy {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::with_capacity(10_000))),
            missed_deadlines: Arc::new(AtomicUsize::new(0)),
            total_cycles: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl SyncStrategy for AtomicStrategy {
    fn record(&self, result: CycleResult) {
        // Use atomic for counter updates (lock-free) - these should have minimal contention
        black_box(self.total_cycles.fetch_add(1, Ordering::Relaxed));
        if !result.deadline_met {
            black_box(self.missed_deadlines.fetch_add(1, Ordering::Relaxed));
        }

        // Measure lock acquisition latency for the Vec storage (Mutex)
        let lock_start = Instant::now();

        if let Ok(mut data) = self.results.lock() {
            let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;
            black_box(&mut *data); // Prevent optimization of lock operations

            // Use measured lock wait time instead of synthetic value
            let mut result_with_lock_time = result;
            result_with_lock_time.lock_wait_ns = lock_wait_ns;

            data.push(result_with_lock_time);
        }
    }

    fn get_missed_deadlines(&self) -> usize {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    fn get_results_count(&self) -> usize {
        self.total_cycles.load(Ordering::Relaxed)
    }

    fn get_results(&self) -> Vec<CycleResult> {
        self.results.lock().unwrap().clone()
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

