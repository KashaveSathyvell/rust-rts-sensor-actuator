use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use common::{BenchmarkRecorder, ExperimentConfig, SensorData};
use common::metrics::CycleResult;

pub fn run_sensor_thread(
    config: ExperimentConfig,
    sender: SyncSender<SensorData>,
    recorder: Arc<BenchmarkRecorder>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let period = Duration::from_millis(config.sensor_period_ms);
    let mut cycle_id: u64 = 0;

    let mut next_tick = start_time;

    while !shutdown_flag.load(Ordering::Relaxed) {
        // --- Absolute timing (drift-free) ---
        let expected_wake = next_tick;
        next_tick += period;

        let now = Instant::now();
        if now < expected_wake {
            thread::sleep(expected_wake - now);
        }

        let actual_wake = Instant::now();
        let timestamp_ns = actual_wake
            .duration_since(start_time)
            .as_nanos() as u64;

        // --- Simulated sensor signals ---
        let t = cycle_id as f64 * 0.1;
        let force = 50.0 + 10.0 * t.sin();
        let position = 5.0 * t.cos();
        let temperature = 25.0 + (cycle_id % 10) as f64;

        let sensor_data = SensorData {
            id: cycle_id,
            timestamp: timestamp_ns,
            force,
            position,
            temperature,
        };

        let deadline_met = sender.try_send(sensor_data).is_ok();

        // Sensor jitter relative to expected wake
        let jitter_ns = actual_wake
            .duration_since(expected_wake)
            .as_nanos() as i64;

        recorder.record(CycleResult {
            cycle_id,
            mode: config.mode.clone(),
            total_latency_ns: 0,
            processing_time_ns: 0,
            lock_wait_ns: 0,
            deadline_met,
            lateness_ns: jitter_ns,
        });

        cycle_id += 1;
    }
}