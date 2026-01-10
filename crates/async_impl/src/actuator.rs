use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use common::{
    ActuatorFeedback, ActuatorStatus, ActuatorType,
    BenchmarkRecorder, ExperimentConfig, SensorData,
    DashboardBuffer, DashboardData, MetricsSnapshot,
};
use common::metrics::CycleResult;
use common::pid::PidController;

const FEEDBACK_DEADLINE_NS: u64 = 500_000; // 0.5 ms in nanoseconds

pub async fn run_actuator_task(
    actuator_type: ActuatorType,
    deadline: Duration,
    config: ExperimentConfig,
    mut receiver: mpsc::Receiver<SensorData>,
    feedback_tx: mpsc::Sender<ActuatorFeedback>,
    recorder: Arc<BenchmarkRecorder>,
    shutdown: Arc<AtomicBool>,
    start_time: Instant,
    dashboard: Option<DashboardBuffer>,
) {
    let mut pid = PidController::new(1.0, 0.1, 0.01);
    let mut error_threshold = 5.0; // Dynamic threshold for recalibration
    let mut cycle_count = 0u64;

    let init_time = start_time.elapsed().as_secs_f64();
    if config.enable_logging {
        println!("[{:>8}] [SYSTEM] {:?} actuator initialized - Deadline: {:.1}ms",
                 format!("{:.3}s", init_time), actuator_type, deadline.as_millis() as f64);
    }

    while !shutdown.load(Ordering::Relaxed) {
        let data = match receiver.recv().await {
            Some(d) => d,
            None => continue,
        };

        cycle_count += 1;
        let _timestamp_ns = start_time.elapsed().as_nanos() as u64;
        let cycle_start = Instant::now();
        let error = -data.position;
        let control = pid.compute(error, config.sensor_period_ms as f64 / 1000.0);

        // Determine actuator status based on error magnitude
        let status = if error.abs() > 10.0 {
            if config.enable_logging {
                let elapsed = start_time.elapsed().as_secs_f64();
                println!("[{:>8}] [EMERGENCY] {:?}: Entering emergency mode - Error: {:.2} (>10.0 threshold)",
                         format!("{:.3}s", elapsed), actuator_type, error.abs());
            }
            ActuatorStatus::Emergency
        } else if error.abs() > error_threshold {
            ActuatorStatus::Correcting
        } else {
            ActuatorStatus::Normal
        };

        // Log actuator processing more frequently for demonstration
        if config.enable_logging && cycle_count % 10 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            println!("[{:>8}] {:?}: Processed cycle #{:<4} - Error: {:.2}, Control: {:.2} ({:?})",
                     format!("{:.3}s", elapsed), actuator_type, data.id, error, control, status);
        }

        // Dynamic recalibration: adjust threshold based on recent performance
        if error.abs() < 2.0 {
            error_threshold = (error_threshold * 0.99).max(3.0); // Gradually lower threshold
        } else if error.abs() > 8.0 {
            error_threshold = (error_threshold * 1.01).min(7.0); // Gradually raise threshold
        }

        let processing_elapsed = cycle_start.elapsed();
        let deadline_met = processing_elapsed <= deadline;
        let processing_ms = processing_elapsed.as_nanos() as f64 / 1_000_000.0;
        let deadline_ms = deadline.as_nanos() as f64 / 1_000_000.0;

        // Log processing results more frequently
        if config.enable_logging && cycle_count % 10 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            println!("[{:>8}] {:?}: Processed {} (latency: {:.2}ms, deadline: {:.1}ms)",
                     format!("{:.3}s", elapsed), actuator_type, if deadline_met { "✓" } else { "✗" }, processing_ms, deadline_ms);
        }

        // Measure lock wait time
        let lock_start = Instant::now();
        let lateness_ns = if deadline_met { 0 } else {
            processing_elapsed.as_nanos() as i64 - deadline.as_nanos() as i64
        };
        let processing_time_ns = processing_elapsed.as_nanos() as u64;
        let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;
        let total_latency_ns = start_time.elapsed().as_nanos() as u64 - data.timestamp;

        // Log deadline misses with enhanced formatting
        if config.enable_logging && !deadline_met {
            let elapsed = start_time.elapsed().as_secs_f64();
            println!("[{:>8}] [DEADLINE] {:?}: Processing missed - {:.2}ms > {:.1}ms (cycle #{}) ✗",
                     format!("{:.3}s", elapsed), actuator_type, processing_ms, deadline_ms, data.id);
        }

        // Log shared resource access occasionally
        if config.enable_logging && cycle_count % 50 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            println!("[{:>8}] [SYNC] {:?} accessing shared recorder (performance metrics)",
                     format!("{:.3}s", elapsed), actuator_type);
        }

        recorder.record(CycleResult {
            cycle_id: data.id,
            mode: config.mode.clone(),
            actuator: Some(actuator_type),
            total_latency_ns,
            processing_time_ns,
            lock_wait_ns,
            deadline_met,
            lateness_ns,
        });

        // Send feedback within 0.5ms deadline
        let feedback_start = Instant::now();
        let feedback = ActuatorFeedback {
            sensor_id: data.id,
            status,
            control_output: control,
            error,
            timestamp: start_time.elapsed().as_nanos() as u64,
        };
        let feedback_sent = feedback_tx.try_send(feedback).is_ok();
        let feedback_time = feedback_start.elapsed();
        let feedback_deadline_met = feedback_time.as_nanos() as u64 <= FEEDBACK_DEADLINE_NS;

        // Log feedback transmission more frequently
        if config.enable_logging && cycle_count % 10 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let feedback_us = feedback_time.as_nanos() as f64 / 1000.0;
            println!("[{:>8}] {:?}: Feedback sent {} (latency: {:.2}μs, deadline: 500μs)",
                     format!("{:.3}s", elapsed), actuator_type, if feedback_sent && feedback_deadline_met { "✓" } else { "✗" }, feedback_us);
        }

        // Log feedback transmission failures
        if config.enable_logging && !feedback_sent {
            let elapsed = start_time.elapsed().as_secs_f64();
            println!("[{:>8}] [ERROR] {:?}: Feedback transmission failed - channel full (cycle #{})",
                     format!("{:.3}s", elapsed), actuator_type, data.id);
        }

        // Send to dashboard
        if let Some(dash) = &dashboard {
            dash.add(DashboardData {
                timestamp: start_time.elapsed().as_nanos() as u64,
                sensor_data: None,
                actuator_feedback: Some((actuator_type, feedback)),
                metrics: Some(MetricsSnapshot {
                    cycle_id: data.id,
                    processing_time_ns,
                    lock_wait_ns,
                    total_latency_ns,
                    deadline_met,
                    lateness_ns,
                }),
            });
        }
    }
}
