use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{
    BenchmarkRecorder,
    ExperimentConfig,
    SensorData,
};
use common::metrics::CycleResult;
use common::pid::PidController;

pub fn run_actuator_thread(
    config: ExperimentConfig,
    receiver: Receiver<SensorData>,
    recorder: Arc<BenchmarkRecorder>,
    shared_resource: Arc<Mutex<()>>,
    shutdown_flag: Arc<AtomicBool>,
) {
    let mut pid = PidController::new(1.0, 0.1, 0.01);
    let actuator_deadline = Duration::from_millis(2); // 1–2ms as per brief

    let mut last_timestamp_ns: Option<u64> = None;

    while !shutdown_flag.load(Ordering::Relaxed) {
        let sensor_data = match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(data) => data,
            Err(_) => continue,
        };

        let start = Instant::now();

        // --- Latency ---
        let now_ns = start.elapsed().as_nanos() as u64;
        let total_latency_ns = now_ns.saturating_sub(sensor_data.timestamp);

        // --- PID computation ---
        let dt = if let Some(prev) = last_timestamp_ns {
            (sensor_data.timestamp - prev) as f64 / 1_000_000_000.0
        } else {
            config.sensor_period_ms as f64 / 1000.0
        };

        let setpoint = 0.0;
        let error = setpoint - sensor_data.position;
        let _control_output = pid.compute(error, dt);

        last_timestamp_ns = Some(sensor_data.timestamp);

        // --- Shared resource access ---
        let lock_start = Instant::now();
        let _guard = shared_resource.lock().unwrap();
        let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;

        // --- Optional processing load ---
        if config.processing_time_ns > 0 {
            busy_wait_ns(config.processing_time_ns);
        }

        let elapsed = start.elapsed();
        let deadline_met = elapsed <= actuator_deadline;

        let lateness_ns = if deadline_met {
            0
        } else {
            elapsed.as_nanos() as i64 - actuator_deadline.as_nanos() as i64
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

fn busy_wait_ns(duration_ns: u64) {
    let start = Instant::now();
    let target = Duration::from_nanos(duration_ns);
    while start.elapsed() < target {
        std::hint::spin_loop();
    }
}
