use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{sleep_until, Duration, Instant};

use common::{
    BenchmarkRecorder, ExperimentConfig, SensorData, Feedback, SharedDiagnostics,
};
use common::metrics::CycleResult;

const FILTER_WINDOW: usize = 5;

pub async fn run_sensor_task(
    config: ExperimentConfig,
    sender: mpsc::Sender<SensorData>,
    mut feedback_rx: mpsc::Receiver<Feedback>,
    recorder: Arc<BenchmarkRecorder>,
    diagnostics: Arc<SharedDiagnostics>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let period = Duration::from_millis(config.sensor_period_ms);
    let mut cycle_id: u64 = 0;
    let mut next_tick = start_time;

    let mut force_hist = Vec::with_capacity(FILTER_WINDOW);

    while !shutdown_flag.load(Ordering::Relaxed) {
        next_tick += period;
        sleep_until(next_tick).await;

        let actual_wake = Instant::now();
        let timestamp_ns =
            actual_wake.duration_since(start_time).as_nanos() as u64;

        // --- Simulated sensing ---
        let raw_force =
            50.0 + (cycle_id as f64 * 0.1).sin() * 10.0;

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

        let sensor_data = SensorData {
            id: cycle_id,
            timestamp: timestamp_ns,
            force: filtered_force,
            position: 0.0,
            temperature: 25.0,
        };

        let deadline_met = sender.try_send(sensor_data).is_ok();

        let jitter_ns =
            actual_wake.duration_since(next_tick).as_nanos() as i64;

        recorder.record(CycleResult {
            cycle_id,
            mode: config.mode.clone(),
            total_latency_ns: 0,
            processing_time_ns: 0,
            lock_wait_ns: 0,
            deadline_met,
            lateness_ns: jitter_ns,
        });

        // --- Handle actuator feedback ---
        while let Ok(feedback) = feedback_rx.try_recv() {
            if feedback.emergency {
                diagnostics.record_emergency();
            }
        }

        cycle_id += 1;
    }
}
