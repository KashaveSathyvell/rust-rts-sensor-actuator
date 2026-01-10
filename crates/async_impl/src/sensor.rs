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

        // Log sensor data generation (more frequent for demonstration)
        if config.enable_logging && cycle_id % 10 == 0 { // Log every 10th cycle for better visibility
            let elapsed = now.duration_since(start_time).as_secs_f64();
            println!("[{:>8}] SENSOR: Generated cycle #{:<4} - Force: {:.2}, Position: {:.2}, Temp: {:.1}",
                     format!("{:.3}s", elapsed), cycle_id, raw_force, position_base, temperature);
        }

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
            if config.enable_logging {
                let elapsed = now.duration_since(start_time).as_secs_f64();
                println!("[{:>8}] [ERROR] SENSOR: Anomaly detected - Force: {:.2} (>80.0 threshold) at cycle #{}",
                         format!("{:.3}s", elapsed), filtered_force, cycle_id);
            }
        }

        let processing_time = processing_start.elapsed();
        let processing_time_ns = processing_time.as_nanos() as u64;
        let processing_deadline_met = processing_time_ns <= PROCESSING_DEADLINE_NS;

        // Log processing results
        if config.enable_logging && cycle_id % 10 == 0 {
            let elapsed = now.duration_since(start_time).as_secs_f64();
            let processing_us = processing_time_ns as f64 / 1000.0;
            println!("[{:>8}] SENSOR: Filtered data - Anomaly: {}, Processing: {:.2}μs {} (deadline: 200μs)",
                     format!("{:.3}s", elapsed), anomaly, processing_us,
                     if processing_deadline_met { "✓" } else { "✗" });
        }

        // Measure lock wait time when recording
        let lock_start = Instant::now();

        // Log shared resource access occasionally
        if config.enable_logging && cycle_id % 50 == 0 {
            let elapsed = now.duration_since(start_time).as_secs_f64();
            println!("[{:>8}] [SYNC] Sensor accessing shared recorder (benchmark metrics)",
                     format!("{:.3}s", elapsed));
        }

        // Periodic performance summary
        if config.enable_logging && cycle_id % 100 == 0 && cycle_id > 0 {
            let elapsed = now.duration_since(start_time).as_secs_f64();
            let cycles_per_sec = cycle_id as f64 / elapsed;
            let anomalies = diagnostics.anomaly_count.load(Ordering::Relaxed);
            let emergencies = diagnostics.emergency_stops.load(Ordering::Relaxed);
            println!("[{:>8}] [PERF] System running - {:.1} cycles/sec, Anomalies: {}, Emergencies: {}",
                     format!("{:.3}s", elapsed), cycles_per_sec, anomalies, emergencies);
        }

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

        // Log transmission results
        if config.enable_logging && cycle_id % 10 == 0 {
            let elapsed = now.duration_since(start_time).as_secs_f64();
            let transmission_us = transmission_time_ns as f64 / 1000.0;
            println!("[{:>8}] SENSOR: Transmitted to dispatcher {} (latency: {:.2}μs, deadline: 100μs)",
                     format!("{:.3}s", elapsed), if transmission_success { "✓" } else { "✗" }, transmission_us);
        }

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

        // Log deadline misses with enhanced formatting
        if config.enable_logging && !deadline_met {
            let elapsed = now.duration_since(start_time).as_secs_f64();
            if !processing_deadline_met {
                println!("[{:>8}] [DEADLINE] SENSOR: Processing missed - {:.2}μs > 200μs (cycle #{}) ✗",
                        format!("{:.3}s", elapsed), processing_time_ns as f64 / 1000.0, cycle_id);
            }
            if !transmission_deadline_met {
                println!("[{:>8}] [DEADLINE] SENSOR: Transmission missed - {:.2}μs > 100μs (cycle #{}) ✗",
                        format!("{:.3}s", elapsed), transmission_time_ns as f64 / 1000.0, cycle_id);
            }
            if !transmission_success {
                println!("[{:>8}] [ERROR] SENSOR: Transmission failed - channel full (cycle #{})",
                        format!("{:.3}s", elapsed), cycle_id);
            }
        }

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
            if config.enable_logging {
                let elapsed = now.duration_since(start_time).as_secs_f64();

                if matches!(feedback.status, common::ActuatorStatus::Emergency) {
                    println!("[{:>8}] [SYNC] Sensor accessing shared diagnostics (emergency recording)",
                            format!("{:.3}s", elapsed));
                    diagnostics.record_emergency();
                    println!("[{:>8}] [EMERGENCY] SENSOR: Emergency state received from actuator - cycle #{}",
                            format!("{:.3}s", elapsed), feedback.sensor_id);
                }

                // Log feedback reception more frequently for demonstration
                if cycle_id % 10 == 0 {
                    println!("[{:>8}] FEEDBACK: Received from actuator - Error: {:.2}, Control: {:.2}, Status: {:?}",
                            format!("{:.3}s", elapsed), feedback.error, feedback.control_output, feedback.status);
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
                let old_window = current_filter_window;
                current_filter_window = (current_filter_window + 1).min(MAX_FILTER_WINDOW);
                force_hist.resize(current_filter_window, 0.0);
                if config.enable_logging && cycle_id % 10 == 0 && old_window != current_filter_window {
                    let elapsed = now.duration_since(start_time).as_secs_f64();
                    println!("[{:>8}] [RECOVERY] SENSOR: Increased filter window {}→{} for better noise reduction (error: {:.2})",
                            format!("{:.3}s", elapsed), old_window, current_filter_window, feedback.error);
                }
            } else if feedback.error.abs() < 1.0 {
                // Reduce filter window size for faster response when system is stable
                let old_window = current_filter_window;
                current_filter_window = (current_filter_window - 1).max(MIN_FILTER_WINDOW);
                force_hist.resize(current_filter_window, 0.0);
                if config.enable_logging && cycle_id % 10 == 0 && old_window != current_filter_window {
                    let elapsed = now.duration_since(start_time).as_secs_f64();
                    println!("[{:>8}] [RECOVERY] SENSOR: Reduced filter window {}→{} for faster response (error: {:.2})",
                            format!("{:.3}s", elapsed), old_window, current_filter_window, feedback.error);
                }
            }

            // Adjust position base slightly based on actuator error to compensate for drift
            if feedback.error.abs() > 3.0 {
                position_base -= feedback.error * 0.01; // Small correction based on actuator feedback
                if config.enable_logging && cycle_id % 20 == 0 {
                    println!("[{:012}] RECOVERY: Position compensation applied ({:.3})", timestamp_ns, -feedback.error * 0.01);
                }
            }
        }

        cycle_id += 1;
    }
}
