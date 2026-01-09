// use std::sync::atomic::{AtomicBool, Ordering};
// use std::sync::{Arc, Mutex};
// use std::sync::mpsc::{Receiver, SyncSender};
// use std::thread;
// use std::time::Instant;

// use common::{
//     BenchmarkRecorder, ExperimentConfig, SensorData, Feedback, SharedDiagnostics,
// };
// use common::metrics::CycleResult;
// use common::pid::PidController;

// pub fn run_actuator_thread(
//     config: ExperimentConfig,
//     receiver: Receiver<SensorData>,
//     feedback_tx: SyncSender<Feedback>,
//     recorder: Arc<BenchmarkRecorder>,
//     diagnostics: Arc<SharedDiagnostics>,
//     shutdown_flag: Arc<AtomicBool>,
//     start_time: Instant,
// ) {
//     let mut pid = PidController::new(1.0, 0.1, 0.01);

//     while !shutdown_flag.load(Ordering::Relaxed) {
//         if let Ok(data) = receiver.recv() {
//             let lock_start = Instant::now();
//             let output = pid.compute(data.force, 0.1);
//             let lock_wait_ns =
//                 lock_start.elapsed().as_nanos() as u64;

//             let emergency = output.abs() > 100.0;

//             let latency_ns = Instant::now()
//                 .duration_since(start_time)
//                 .as_nanos() as u64
//                 - data.timestamp;

//             let _ = feedback_tx.send(Feedback {
//                 sensor_id: data.id,
//                 emergency,
//                 timestamp: latency_ns,
//             });

//             recorder.record(CycleResult {
//                 cycle_id: data.id,
//                 mode: config.mode.clone(),
//                 total_latency_ns: latency_ns,
//                 processing_time_ns: 0,
//                 lock_wait_ns,
//                 deadline_met: latency_ns < 500_000,
//                 lateness_ns: 0,
//             });
//         }
//     }
// }




use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{
    ActuatorType, ActuatorStatus, BenchmarkRecorder,
    ExperimentConfig, SensorData, ActuatorFeedback,
};
use common::metrics::CycleResult;
use common::pid::PidController;

const FEEDBACK_DEADLINE_NS: u64 = 500_000; // 0.5 ms in nanoseconds

pub fn run_actuator_thread(
    actuator_type: ActuatorType,
    deadline: Duration,
    config: ExperimentConfig,
    receiver: Receiver<SensorData>,
    feedback_tx: SyncSender<ActuatorFeedback>,
    recorder: Arc<BenchmarkRecorder>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let mut pid = PidController::new(1.0, 0.1, 0.01);
    let mut error_threshold = 5.0; // Dynamic threshold for recalibration

    while !shutdown_flag.load(Ordering::Relaxed) {
        let data = match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(d) => d,
            Err(_) => continue,
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
        recorder.record(CycleResult {
            cycle_id: data.id,
            mode: config.mode.clone(),
            actuator: Some(actuator_type),
            total_latency_ns: start_time.elapsed().as_nanos() as u64 - data.timestamp,
            processing_time_ns: processing_elapsed.as_nanos() as u64,
            lock_wait_ns: lock_start.elapsed().as_nanos() as u64,
            deadline_met,
            lateness_ns: if deadline_met {
                0
            } else {
                processing_elapsed.as_nanos() as i64 - deadline.as_nanos() as i64
            },
        });

        // Send feedback within 0.5ms deadline
        let feedback_start = Instant::now();
        let _feedback_sent = feedback_tx.try_send(ActuatorFeedback {
            sensor_id: data.id,
            status,
            control_output: control,
            error,
            timestamp: start_time.elapsed().as_nanos() as u64,
        }).is_ok();
        let feedback_time = feedback_start.elapsed();
        let feedback_deadline_met = feedback_time.as_nanos() as u64 <= FEEDBACK_DEADLINE_NS;

        // Log if feedback deadline is missed (for analysis)
        if !feedback_deadline_met {
            // Could record this in diagnostics if needed
        }
    }
}
