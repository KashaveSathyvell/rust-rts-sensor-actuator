use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tokio::time::{Duration, Instant};

use common::metrics::CycleResult;
use common::pid::PidController;
use common::{BenchmarkRecorder, ExperimentConfig, SensorData};

pub async fn run_actuator_task(
    config: ExperimentConfig,
    mut receiver: mpsc::Receiver<SensorData>,
    recorder: Arc<BenchmarkRecorder>,
    shared_resource: Arc<Mutex<()>>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant, // Shared experiment clock
) {
    let mut pid = PidController::new(1.0, 0.1, 0.01);
    let deadline = Duration::from_millis(2);

    while !shutdown_flag.load(Ordering::Relaxed) {
        let sensor_data = match receiver.recv().await {
            Some(data) => data,
            None => break,
        };

        let cycle_start = Instant::now();

        // End-to-end latency (single clock domain)
        let now_ns = start_time.elapsed().as_nanos() as u64;
        let total_latency_ns =
            now_ns.saturating_sub(sensor_data.timestamp);

        // PID control
        let error = -sensor_data.position;
        let dt = config.sensor_period_ms as f64 / 1000.0;
        let _control_output = pid.compute(error, dt);

        // Shared resource contention
        let lock_start = Instant::now();
        {
            let _guard = shared_resource.lock().await;
        }
        let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;

        // Simulated actuation processing
        if config.processing_time_ns > 0 {
            busy_spin_ns(Instant::now(), config.processing_time_ns);
        }

        // Deadline evaluation
        let elapsed = cycle_start.elapsed();
        let deadline_met = elapsed <= deadline;
        let lateness_ns = if deadline_met {
            0
        } else {
            elapsed.as_nanos() as i64 - deadline.as_nanos() as i64
        };

        recorder.record(CycleResult {
            cycle_id: sensor_data.id,
            mode: config.mode.clone(),
            total_latency_ns,
            processing_time_ns: config.processing_time_ns,
            lock_wait_ns,
            deadline_met,
            lateness_ns,
        });
    }
}

/// True CPU burn for stress testing (intentional)
fn busy_spin_ns(start_time: Instant, duration_ns: u64) {
    let target = Duration::from_nanos(duration_ns);
    while start_time.elapsed() < target {
        std::hint::spin_loop();
    }
}
