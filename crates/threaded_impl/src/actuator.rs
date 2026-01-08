use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread;
use std::time::Instant;

use common::{
    BenchmarkRecorder, ExperimentConfig, SensorData, Feedback, SharedDiagnostics,
};
use common::metrics::CycleResult;
use common::pid::PidController;

pub fn run_actuator_thread(
    config: ExperimentConfig,
    receiver: Receiver<SensorData>,
    feedback_tx: SyncSender<Feedback>,
    recorder: Arc<BenchmarkRecorder>,
    diagnostics: Arc<SharedDiagnostics>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let mut pid = PidController::new(1.0, 0.1, 0.01);

    while !shutdown_flag.load(Ordering::Relaxed) {
        if let Ok(data) = receiver.recv() {
            let lock_start = Instant::now();
            let output = pid.compute(data.force, 0.1);
            let lock_wait_ns =
                lock_start.elapsed().as_nanos() as u64;

            let emergency = output.abs() > 100.0;

            let latency_ns = Instant::now()
                .duration_since(start_time)
                .as_nanos() as u64
                - data.timestamp;

            let _ = feedback_tx.send(Feedback {
                sensor_id: data.id,
                emergency,
                timestamp: latency_ns,
            });

            recorder.record(CycleResult {
                cycle_id: data.id,
                mode: config.mode.clone(),
                total_latency_ns: latency_ns,
                processing_time_ns: 0,
                lock_wait_ns,
                deadline_met: latency_ns < 500_000,
                lateness_ns: 0,
            });
        }
    }
}
