use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::Instant;

use common::{
    BenchmarkRecorder, ExperimentConfig, SensorData, Feedback, SharedDiagnostics,
};
use common::metrics::CycleResult;
use common::pid::PidController;

pub async fn run_actuator_task(
    config: ExperimentConfig,
    mut receiver: mpsc::Receiver<SensorData>,
    feedback_tx: mpsc::Sender<Feedback>,
    recorder: Arc<BenchmarkRecorder>,
    diagnostics: Arc<SharedDiagnostics>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let mut pid = PidController::new(1.0, 0.1, 0.01);

    while !shutdown_flag.load(Ordering::Relaxed) {
        if let Some(data) = receiver.recv().await {
            let control_output = pid.compute(data.force, 0.1);

            let emergency = control_output.abs() > 100.0;
            if emergency {
                diagnostics.record_emergency();
            }

            let latency_ns = Instant::now()
                .duration_since(start_time)
                .as_nanos() as u64
                - data.timestamp;

            let _ = feedback_tx.send(Feedback {
                sensor_id: data.id,
                emergency,
                timestamp: latency_ns,
            }).await;

            recorder.record(CycleResult {
                cycle_id: data.id,
                mode: config.mode.clone(),
                total_latency_ns: latency_ns,
                processing_time_ns: 0,
                lock_wait_ns: 0,
                deadline_met: latency_ns < 500_000,
                lateness_ns: 0,
            });
        }
    }
}
