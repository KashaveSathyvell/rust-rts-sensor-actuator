use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use common::{BenchmarkRecorder, ExperimentConfig, SensorData, ActuatorFeedback, SharedDiagnostics, DashboardBuffer, DashboardData, MetricsSnapshot};
use common::metrics::CycleResult;

const FILTER_WINDOW: usize = 5;
const MAX_FILTER_WINDOW: usize = 10;
const MIN_FILTER_WINDOW: usize = 3;
const PROCESSING_DEADLINE_NS: u64 = 200_000; // 0.2 ms in nanoseconds
const TRANSMISSION_DEADLINE_NS: u64 = 100_000; // 0.1 ms in nanoseconds

pub async fn run_sensor_task(
    config: ExperimentConfig,
    sender: mpsc::Sender<SensorData>,
    mut feedback_rx: mpsc::Receiver<ActuatorFeedback>,
    recorder: Arc<BenchmarkRecorder>,
    diagnostics: Arc<SharedDiagnostics>,
    shutdown: Arc<AtomicBool>,
    start_time: Instant,
    dashboard: Option<DashboardBuffer>,
) {
    let period = Duration::from_millis(config.sensor_period_ms);
    let mut next_tick = start_time;
    let mut cycle_id = 0u64;
    let mut force_hist = Vec::with_capacity(FILTER_WINDOW);
    let mut position_base = 10.0;
    let mut current_filter_window = FILTER_WINDOW;
    let mut temperature = 25.0;

    while !shutdown.load(Ordering::Relaxed) {
        let cycle_start = Instant::now();
        let expected = next_tick;
        next_tick += period;
        tokio::time::sleep_until(next_tick).await;

        let generation_start = Instant::now();
        let now = Instant::now();
        let timestamp_ns = now.duration_since(start_time).as_nanos() as u64;

        // Generate realistic sensor data with variations
        let raw_force = 50.0 + (cycle_id as f64 * 0.1).sin() * 10.0 + (cycle_id as f64 * 0.05).cos() * 5.0;
        position_base += (cycle_id as f64 * 0.02).sin() * 0.1;
        temperature += (cycle_id as f64 * 0.01).sin() * 0.5;
        temperature = temperature.max(20.0).min(30.0);

        let _generation_time = generation_start.elapsed();

        // Process data: Apply moving average filter
        let processing_start = Instant::now();
        force_hist.push(raw_force);
        if force_hist.len() > FILTER_WINDOW {
            force_hist.remove(0);
        }
        let filtered_force = force_hist.iter().sum::<f64>() / force_hist.len() as f64;

        // Anomaly detection
        let anomaly = filtered_force.abs() > 80.0;
        if anomaly {
            diagnostics.record_anomaly();
        }

        let processing_time = processing_start.elapsed();
        let processing_time_ns = processing_time.as_nanos() as u64;
        let processing_deadline_met = processing_time_ns <= PROCESSING_DEADLINE_NS;

        // Measure lock wait time when recording
        let lock_start = Instant::now();
        let data = SensorData {
            id: cycle_id,
            timestamp: timestamp_ns,
            force: filtered_force,
            position: position_base,
            temperature,
        };

        // Transmit data
        let transmission_start = Instant::now();
        let transmission_success = sender.try_send(data).is_ok();
        let transmission_time = transmission_start.elapsed();
        let transmission_time_ns = transmission_time.as_nanos() as u64;
        let transmission_deadline_met = transmission_time_ns <= TRANSMISSION_DEADLINE_NS;

        // Record metrics with proper timing
        let _jitter_ns = now.duration_since(expected).as_nanos() as i64;
        let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;
        
        let lateness_ns = if processing_deadline_met && transmission_deadline_met {
            0
        } else {
            let processing_late = if processing_time_ns > PROCESSING_DEADLINE_NS {
                processing_time_ns - PROCESSING_DEADLINE_NS
            } else { 0 };
            let transmission_late = if transmission_time_ns > TRANSMISSION_DEADLINE_NS {
                transmission_time_ns - TRANSMISSION_DEADLINE_NS
            } else { 0 };
            (processing_late.max(transmission_late)) as i64
        };
        
        let deadline_met = processing_deadline_met && transmission_deadline_met && transmission_success;
        
        recorder.record(CycleResult {
            cycle_id,
            mode: config.mode.clone(),
            actuator: None,
            total_latency_ns: cycle_start.elapsed().as_nanos() as u64,
            processing_time_ns,
            lock_wait_ns,
            deadline_met,
            lateness_ns,
        });
        
        // Send to dashboard
        if let Some(dash) = &dashboard {
            dash.add(DashboardData {
                timestamp: timestamp_ns,
                sensor_data: Some(data),
                actuator_feedback: None,
                metrics: Some(MetricsSnapshot {
                    cycle_id,
                    processing_time_ns,
                    lock_wait_ns,
                    total_latency_ns: cycle_start.elapsed().as_nanos() as u64,
                    deadline_met,
                    lateness_ns,
                }),
            });
        }

        // Process feedback (non-blocking) for dynamic recalibration
        while let Ok(feedback) = feedback_rx.try_recv() {
            if matches!(feedback.status, common::ActuatorStatus::Emergency) {
                diagnostics.record_emergency();
            }

            // Dynamic recalibration based on actuator feedback
            if feedback.error.abs() > 5.0 {
                // Increase filter window size for better noise reduction when errors are high
                current_filter_window = (current_filter_window + 1).min(MAX_FILTER_WINDOW);
                force_hist.resize(current_filter_window, 0.0);
            } else if feedback.error.abs() < 1.0 {
                // Reduce filter window size for faster response when system is stable
                current_filter_window = (current_filter_window - 1).max(MIN_FILTER_WINDOW);
                force_hist.resize(current_filter_window, 0.0);
            }

            // Adjust position base slightly based on actuator error to compensate for drift
            if feedback.error.abs() > 3.0 {
                position_base -= feedback.error * 0.01; // Small correction based on actuator feedback
            }
        }

        cycle_id += 1;
    }
}
