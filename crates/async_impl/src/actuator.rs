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

    while !shutdown.load(Ordering::Relaxed) {
        let data = match receiver.recv().await {
            Some(d) => d,
            None => continue,
        };

        let cycle_start = Instant::now();
        let error = -data.position;
        let control = pid.compute(error, config.sensor_period_ms as f64 / 1000.0);

        // Determine actuator status based on error magnitude
        let status = if error.abs() > 10.0 {
            ActuatorStatus::Emergency
        } else if error.abs() > error_threshold {
            ActuatorStatus::Correcting
        } else {
            ActuatorStatus::Normal
        };

        // Dynamic recalibration: adjust threshold based on recent performance
        if error.abs() < 2.0 {
            error_threshold = (error_threshold * 0.99).max(3.0); // Gradually lower threshold
        } else if error.abs() > 8.0 {
            error_threshold = (error_threshold * 1.01).min(7.0); // Gradually raise threshold
        }

        let processing_elapsed = cycle_start.elapsed();
        let deadline_met = processing_elapsed <= deadline;

        // Measure lock wait time
        let lock_start = Instant::now();
        let lateness_ns = if deadline_met { 0 } else {
            processing_elapsed.as_nanos() as i64 - deadline.as_nanos() as i64
        };
        let processing_time_ns = processing_elapsed.as_nanos() as u64;
        let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;
        let total_latency_ns = start_time.elapsed().as_nanos() as u64 - data.timestamp;
        
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
        let _feedback_sent = feedback_tx.try_send(feedback).is_ok();
        let feedback_time = feedback_start.elapsed();
        let _feedback_deadline_met = feedback_time.as_nanos() as u64 <= FEEDBACK_DEADLINE_NS;

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
