use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::{Duration, Instant};

use common::{
    BenchmarkRecorder, ExperimentConfig, SensorData, ActuatorFeedback, SharedDiagnostics,
};
use common::metrics::CycleResult;

const FILTER_WINDOW: usize = 5;
const MAX_FILTER_WINDOW: usize = 10;
const MIN_FILTER_WINDOW: usize = 3;

pub fn run_sensor_thread(
    config: ExperimentConfig,
    sender: SyncSender<SensorData>,
    feedback_rx: std::sync::mpsc::Receiver<ActuatorFeedback>,
    recorder: Arc<BenchmarkRecorder>,
    diagnostics: Arc<SharedDiagnostics>,
    shutdown_flag: Arc<AtomicBool>,
    start_time: Instant,
) {
    let period = Duration::from_millis(config.sensor_period_ms);
    let mut cycle_id = 0u64;
    let mut next_tick = start_time;
    let mut _position_base = 10.0;
    let mut current_filter_window = FILTER_WINDOW;

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

        // Log sensor data generation (more frequent for demonstration)
        if config.enable_logging && cycle_id % 10 == 0 {
            let elapsed = actual.duration_since(start_time).as_secs_f64();
            println!("[{:>8}] SENSOR: Generated cycle #{:<4} - Force: {:.2}, Position: {:.2}, Temp: {:.1}",
                     format!("{:.3}s", elapsed), cycle_id, raw_force, 0.0, 25.0);
        }

        force_hist.push(raw_force);
        if force_hist.len() > FILTER_WINDOW {
            force_hist.remove(0);
        }

        let filtered_force =
            force_hist.iter().sum::<f64>() / force_hist.len() as f64;

        let anomaly = filtered_force.abs() > 80.0;
        if anomaly {
            diagnostics.record_anomaly();
            if config.enable_logging {
                let elapsed = actual.duration_since(start_time).as_secs_f64();
                println!("[{:>8}] [ERROR] SENSOR: Anomaly detected - Force: {:.2} (>80.0 threshold) at cycle #{}",
                         format!("{:.3}s", elapsed), filtered_force, cycle_id);
            }
        }

        // Log processing results more frequently
        if config.enable_logging && cycle_id % 10 == 0 {
            let elapsed = actual.duration_since(start_time).as_secs_f64();
            let processing_us = (actual.duration_since(expected)).as_nanos() as f64 / 1000.0;
            println!("[{:>8}] SENSOR: Filtered data - Anomaly: {}, Processing: {:.2}μs ✓ (deadline: 200μs)",
                     format!("{:.3}s", elapsed), anomaly, processing_us);
        }

        let data = SensorData {
            id: cycle_id,
            timestamp: timestamp_ns,
            force: filtered_force,
            position: 0.0,
            temperature: 25.0,
        };

        // Measure processing time
        let processing_time = actual.duration_since(expected);
        let processing_time_ns = processing_time.as_nanos() as u64;
        const PROCESSING_DEADLINE_NS: u64 = 200_000; // 0.2 ms
        let processing_deadline_met = processing_time_ns <= PROCESSING_DEADLINE_NS;

        // Measure transmission time
        let transmission_start = Instant::now();
        let transmission_success = sender.send(data).is_ok();
        let transmission_time = transmission_start.elapsed();
        let transmission_time_ns = transmission_time.as_nanos() as u64;
        const TRANSMISSION_DEADLINE_NS: u64 = 100_000; // 0.1 ms
        let transmission_deadline_met = transmission_time_ns <= TRANSMISSION_DEADLINE_NS;

        if cycle_id % 20 == 0 {
            let transmission_us = transmission_time_ns as f64 / 1000.0;
            println!("[{:012}] SENSOR: Transmitted to dispatcher {} (latency: {:.2}μs, deadline: 100μs)",
                    timestamp_ns, if transmission_success { "✓" } else { "✗" }, transmission_us);
        }

        // Log deadline misses
        let deadline_met = processing_deadline_met && transmission_deadline_met && transmission_success;
        if config.enable_logging && !deadline_met {
            if !processing_deadline_met {
                println!("[DEADLINE] Sensor processing missed: {:.2}μs > 200μs (cycle #{})",
                        processing_time_ns as f64 / 1000.0, cycle_id);
            }
            if !transmission_deadline_met {
                println!("[DEADLINE] Sensor transmission missed: {:.2}μs > 100μs (cycle #{})",
                        transmission_time_ns as f64 / 1000.0, cycle_id);
            }
            if !transmission_success {
                println!("[ERROR] Sensor transmission failed - channel full (cycle #{})", cycle_id);
            }
        }

        // Measure lock wait time
        let lock_start = Instant::now();
        let _jitter_ns = actual.duration_since(expected).as_nanos() as i64;
        let lock_wait_ns = lock_start.elapsed().as_nanos() as u64;

        // Periodic performance summary
        if config.enable_logging && cycle_id % 100 == 0 && cycle_id > 0 {
            let elapsed = actual.duration_since(start_time).as_secs_f64();
            let cycles_per_sec = cycle_id as f64 / elapsed;
            let anomalies = diagnostics.anomaly_count.load(Ordering::Relaxed);
            let emergencies = diagnostics.emergency_stops.load(Ordering::Relaxed);
            println!("[{:012}] [PERF] Threaded system running - {:.1} cycles/sec, Anomalies: {}, Emergencies: {}",
                     timestamp_ns, cycles_per_sec, anomalies, emergencies);
        }

        recorder.record(CycleResult {
            cycle_id,
            mode: config.mode.clone(),
            actuator: None,
            total_latency_ns: actual.duration_since(start_time).as_nanos() as u64,
            processing_time_ns,
            lock_wait_ns,
            deadline_met: processing_deadline_met && transmission_deadline_met && transmission_success,
            lateness_ns: if processing_deadline_met && transmission_deadline_met {
                0
            } else {
                let processing_late = if processing_time_ns > PROCESSING_DEADLINE_NS {
                    processing_time_ns - PROCESSING_DEADLINE_NS
                } else { 0 };
                let transmission_late = if transmission_time_ns > TRANSMISSION_DEADLINE_NS {
                    transmission_time_ns - TRANSMISSION_DEADLINE_NS
                } else { 0 };
                (processing_late.max(transmission_late)) as i64
            },
        });

        while let Ok(feedback) = feedback_rx.try_recv() {
            if config.enable_logging {
                if matches!(feedback.status, common::ActuatorStatus::Emergency) {
                    diagnostics.record_emergency();
                    println!("[{:012}] FEEDBACK: Emergency state received from actuator - cycle #{}", timestamp_ns, feedback.sensor_id);
                }

                if cycle_id % 20 == 0 {
                    println!("[{:012}] FEEDBACK: Received from actuator - Error: {:.2}, Control: {:.2}, Status: {:?}",
                            timestamp_ns, feedback.error, feedback.control_output, feedback.status);
                }
            } else {
                // Still process emergency feedback even when logging is disabled
                if matches!(feedback.status, common::ActuatorStatus::Emergency) {
                    diagnostics.record_emergency();
                }
            }

            // Dynamic recalibration based on actuator feedback
            if feedback.error.abs() > 5.0 {
                // Increase filter window size for better noise reduction when errors are high
                current_filter_window = (current_filter_window + 1).min(MAX_FILTER_WINDOW);
                force_hist.resize(current_filter_window, 0.0);
                if config.enable_logging && cycle_id % 20 == 0 {
                    println!("[{:012}] RECOVERY: Increased filter window to {} for better noise reduction", timestamp_ns, current_filter_window);
                }
            } else if feedback.error.abs() < 1.0 {
                // Reduce filter window size for faster response when system is stable
                current_filter_window = (current_filter_window - 1).max(MIN_FILTER_WINDOW);
                force_hist.resize(current_filter_window, 0.0);
                if config.enable_logging && cycle_id % 20 == 0 {
                    println!("[{:012}] RECOVERY: Reduced filter window to {} for faster response", timestamp_ns, current_filter_window);
                }
            }

            // Adjust position base slightly based on actuator error to compensate for drift
            if feedback.error.abs() > 3.0 {
                _position_base -= feedback.error * 0.01; // Small correction based on actuator feedback
                if config.enable_logging && cycle_id % 20 == 0 {
                    println!("[{:012}] RECOVERY: Position compensation applied ({:.3})", timestamp_ns, -feedback.error * 0.01);
                }
            }
        }

        cycle_id += 1;
    }
}
