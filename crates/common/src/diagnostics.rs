use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct SharedDiagnostics {
    pub anomaly_count: AtomicU64,
    pub emergency_stops: AtomicU64,
}

impl SharedDiagnostics {
    pub fn record_anomaly(&self) {
        self.anomaly_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_emergency(&self) {
        self.emergency_stops.fetch_add(1, Ordering::Relaxed);
    }
}
