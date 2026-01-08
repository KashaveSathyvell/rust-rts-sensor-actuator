use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{sleep_until, Duration, Instant};

use common::metrics::CycleResult;
use common::{BenchmarkRecorder, ExperimentConfig, SensorData};

pub async fn run_sensor_task(
    config: ExperimentConfig,
    sender: mpsc::Sender<SensorData>,
    recorder: Arc<BenchmarkRecorder>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let period = Duration::from_millis(config.sensor_period_ms);
    let mut cycle_id: u64 = 0;

    let mut next_tick = start_time;

    while !shutdown_flag.load(Ordering::Relaxed) {
        // Absolute periodic scheduling
        next_tick += period;
        sleep_until(next_tick).await;

        let actual_wake = Instant::now();
        let timestamp_ns = actual_wake
            .duration_since(start_time)
            .as_nanos() as u64;

        // --- Simulated sensing ---
        let t = cycle_id as f64 * 0.1;
        let force = 50.0 + 10.0 * t.sin();
        let position = 100.0 + 5.0 * t.cos();
        let temperature = 25.0 + (cycle_id % 10) as f64;

        // --- Simulated processing (CPU burn) ---
        let processing_start = Instant::now();
        if config.processing_time_ns > 0 {
            busy_spin_ns(Instant::now(), config.processing_time_ns);
        }
        let processing_time_ns =
            processing_start.elapsed().as_nanos() as u64;

        let sensor_data = SensorData {
            id: cycle_id,
            timestamp: timestamp_ns,
            force,
            position,
            temperature,
        };

        let deadline_met = sender.try_send(sensor_data).is_ok();

        // Sensor jitter = wake-up error
        let jitter_ns =
            actual_wake.duration_since(next_tick).as_nanos() as i64;

        recorder.record(CycleResult {
            cycle_id,
            mode: config.mode.clone(),
            total_latency_ns: 0, // Not known at sensor stage
            processing_time_ns,
            lock_wait_ns: 0,
            deadline_met,
            lateness_ns: jitter_ns,
        });

        cycle_id += 1;
    }
}

/// True CPU burn for stress testing (intentional)
fn busy_spin_ns(start_time: Instant, duration_ns: u64) {
    let target = Duration::from_nanos(duration_ns);
    while start_time.elapsed() < target {
        std::hint::spin_loop();
    }
}
