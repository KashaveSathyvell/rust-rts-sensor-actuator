use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::{Duration, Instant};

use common::{
    BenchmarkRecorder, ExperimentConfig, SensorData, Feedback, SharedDiagnostics,
};
use common::metrics::CycleResult;

const FILTER_WINDOW: usize = 5;

pub fn run_sensor_thread(
    config: ExperimentConfig,
    sender: SyncSender<SensorData>,
    feedback_rx: std::sync::mpsc::Receiver<Feedback>,
    recorder: Arc<BenchmarkRecorder>,
    diagnostics: Arc<SharedDiagnostics>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let period = Duration::from_millis(config.sensor_period_ms);
    let mut cycle_id = 0u64;
    let mut next_tick = start_time;

    let mut force_hist = Vec::with_capacity(FILTER_WINDOW);

    while !shutdown_flag.load(Ordering::Relaxed) {
        let expected = next_tick;
        next_tick += period;

        if Instant::now() < expected {
            thread::sleep(expected - Instant::now());
        }

        let actual = Instant::now();
        let timestamp_ns = actual.duration_since(start_time).as_nanos() as u64;

        let raw_force = 50.0 + (cycle_id as f64 * 0.1).sin() * 10.0;
        force_hist.push(raw_force);
        if force_hist.len() > FILTER_WINDOW {
            force_hist.remove(0);
        }

        let filtered_force =
            force_hist.iter().sum::<f64>() / force_hist.len() as f64;

        let anomaly = filtered_force.abs() > 80.0;
        if anomaly {
            diagnostics.record_anomaly();
        }

        let data = SensorData {
            id: cycle_id,
            timestamp: timestamp_ns,
            force: filtered_force,
            position: 0.0,
            temperature: 25.0,
        };

        let deadline_met = sender.send(data).is_ok();

        let jitter_ns =
            actual.duration_since(expected).as_nanos() as i64;

        recorder.record(CycleResult {
            cycle_id,
            mode: config.mode.clone(),
            total_latency_ns: 0,
            processing_time_ns: 0,
            lock_wait_ns: 0,
            deadline_met,
            lateness_ns: jitter_ns,
        });

        while let Ok(feedback) = feedback_rx.try_recv() {
            if feedback.emergency {
                diagnostics.record_emergency();
            }
        }

        cycle_id += 1;
    }
}
