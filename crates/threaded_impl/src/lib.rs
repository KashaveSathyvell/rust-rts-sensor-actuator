use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use common::{
    ActuatorFeedback, ActuatorType, BenchmarkRecorder,
    ExperimentConfig, SensorData, SharedDiagnostics,
};

mod actuator;
mod sensor;

pub fn run_experiment(config: ExperimentConfig) -> Arc<BenchmarkRecorder> {
    if config.enable_logging {
        println!("[SYSTEM] Real-Time Sensor-Actuator System Started (Threaded)");
        println!("[SYSTEM] Configuration loaded: {} mode, {}ms sensor period", config.mode, config.sensor_period_ms);
        println!("[SYSTEM] Components initialized: Sensor + 3 Actuators (Gripper, Motor, Stabilizer)");
        println!("[SYSTEM] Starting experiment - Duration: {} seconds", config.duration_secs);
    }

    let recorder = Arc::new(BenchmarkRecorder::new());
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let diagnostics = Arc::new(SharedDiagnostics::default());

    // Sensor -> dispatcher
    let (sensor_tx, dispatcher_rx) = mpsc::sync_channel::<SensorData>(100);

    // Dispatcher -> actuators
    let (gripper_tx, gripper_rx) = mpsc::sync_channel::<SensorData>(100);
    let (motor_tx, motor_rx) = mpsc::sync_channel::<SensorData>(100);
    let (stabilizer_tx, stabilizer_rx) = mpsc::sync_channel::<SensorData>(100);

    // Feedback channel
    let (feedback_tx, feedback_rx) = mpsc::sync_channel::<ActuatorFeedback>(100);

    let start_time = Instant::now();

    // ---------------- SENSOR ----------------
    {
        let cfg = config.clone();
        let rec = Arc::clone(&recorder);
        let diag = Arc::clone(&diagnostics);
        let shutdown = Arc::clone(&shutdown_flag);
        let tx = sensor_tx;
        let feedback_recv = feedback_rx;

        thread::spawn(move || {
            sensor::run_sensor_thread(
                cfg,
                tx,
                feedback_recv,
                rec,
                diag,
                shutdown,
                start_time,
            );
        });
    }

    // ---------------- DISPATCHER ----------------
    {
        let tx1 = gripper_tx.clone();
        let tx2 = motor_tx.clone();
        let tx3 = stabilizer_tx.clone();

        let dispatcher_config = config.clone();
        thread::spawn(move || {
            let mut cycle_count = 0u64;
            let dispatcher_start = Instant::now();
            if dispatcher_config.enable_logging {
                if config.enable_logging {
                    println!("[{:>8}] [SYSTEM] Threaded dispatcher initialized - routing sensor data to 3 actuators",
                             format!("{:.3}s", dispatcher_start.duration_since(start_time).as_secs_f64()));
                }
            }

            while let Ok(data) = dispatcher_rx.recv() {
                cycle_count += 1;
                let gripper_sent = tx1.try_send(data).is_ok();
                let motor_sent = tx2.try_send(data).is_ok();
                let stabilizer_sent = tx3.try_send(data).is_ok();

                // Log dispatcher activity more frequently for demonstration
                if dispatcher_config.enable_logging && cycle_count % 5 == 0 {
                    let elapsed = dispatcher_start.duration_since(start_time).as_secs_f64();
                    if config.enable_logging {
                        println!("[{:>8}] DISPATCHER: Routed cycle #{:<4} to actuators (G:{:?}, M:{:?}, S:{:?})",
                                format!("{:.3}s", elapsed), cycle_count, gripper_sent, motor_sent, stabilizer_sent);
                    }
                }

                // Log transmission failures
                if dispatcher_config.enable_logging && (!gripper_sent || !motor_sent || !stabilizer_sent) {
                    let elapsed = dispatcher_start.duration_since(start_time).as_secs_f64();
                    if config.enable_logging {
                        println!("[{:>8}] [WARNING] DISPATCHER: Transmission failed - G:{:?}, M:{:?}, S:{:?} (cycle #{})",
                                format!("{:.3}s", elapsed), gripper_sent, motor_sent, stabilizer_sent, cycle_count);
                    }
                }
            }
            if dispatcher_config.enable_logging {
                if config.enable_logging {
                    println!("[SYSTEM] Threaded dispatcher shutting down");
                }
            }
        });
    }

    // ---------------- ACTUATORS ----------------
    spawn_actuator(
        ActuatorType::Gripper,
        Duration::from_millis(1),
        config.clone(),
        gripper_rx,
        feedback_tx.clone(),
        Arc::clone(&recorder),
        Arc::clone(&shutdown_flag),
        start_time,
    );

    spawn_actuator(
        ActuatorType::Motor,
        Duration::from_millis(2),
        config.clone(),
        motor_rx,
        feedback_tx.clone(),
        Arc::clone(&recorder),
        Arc::clone(&shutdown_flag),
        start_time,
    );

    spawn_actuator(
        ActuatorType::Stabilizer,
        Duration::from_micros(1500),
        config.clone(),
        stabilizer_rx,
        feedback_tx,
        Arc::clone(&recorder),
        Arc::clone(&shutdown_flag),
        start_time,
    );

    // ---------------- RUN ----------------
    if config.enable_logging {
        println!("[SYSTEM] Threaded experiment running for {} seconds...", config.duration_secs);
    }

    // Performance monitoring thread
    let recorder_clone = Arc::clone(&recorder);
    let shutdown_flag_clone = Arc::clone(&shutdown_flag);
    let perf_config = config.clone();
    let perf_monitor = thread::spawn(move || {
        let mut last_cycle_count = 0u64;
        while !shutdown_flag_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(2));
            let results = recorder_clone.get_results();
            let current_cycles = results.len() as u64;

            if perf_config.enable_logging && current_cycles > last_cycle_count {
                let new_cycles = current_cycles - last_cycle_count;
                let missed = results.iter().rev().take(new_cycles as usize).filter(|r| !r.deadline_met).count();
                let throughput = new_cycles as f64 / 2.0; // cycles per second over 2 second window

                let compliance_rate = if new_cycles > 0 {
                    (new_cycles - missed as u64) as f64 / new_cycles as f64 * 100.0
                } else { 100.0 };

                if config.enable_logging {
                    println!("[PERF] Threaded throughput: {:.1} cycles/sec, Recent compliance: {:.1}% ({}/{} cycles met)",
                            throughput, compliance_rate, new_cycles - missed as u64, new_cycles);
                }
                last_cycle_count = current_cycles;
            }
        }
    });

    thread::sleep(Duration::from_secs(config.duration_secs));
    shutdown_flag.store(true, Ordering::Relaxed);

    if config.enable_logging {
        println!("===========================================");
        println!("Threaded experiment completed - initiating shutdown");
    }
    thread::sleep(Duration::from_millis(100)); // Brief pause for cleanup

    // Wait for performance monitor to finish
    let _ = perf_monitor.join();

    let results = recorder.get_results();
    let total_cycles = results.len();
    let missed_deadlines = results.iter().filter(|r| !r.deadline_met).count();
    let deadline_compliance = if total_cycles > 0 {
        (total_cycles - missed_deadlines) as f64 / total_cycles as f64 * 100.0
    } else { 0.0 };

    if config.enable_logging {
        println!("===========================================");
        println!("FINAL THREADED SYSTEM RESULTS");
        println!("===========================================");
        println!("Total Cycles: {}", total_cycles);
        println!("Deadline Compliance: {:.2}% ({} missed)", deadline_compliance, missed_deadlines);
        println!("===========================================");
    }

    recorder
}

fn spawn_actuator(
    actuator_type: ActuatorType,
    deadline: Duration,
    config: ExperimentConfig,
    receiver: mpsc::Receiver<SensorData>,
    feedback_tx: mpsc::SyncSender<ActuatorFeedback>,
    recorder: Arc<BenchmarkRecorder>,
    shutdown: Arc<AtomicBool>,
    start_time: Instant,
) {
    thread::spawn(move || {
        actuator::run_actuator_thread(
            actuator_type,
            deadline,
            config,
            receiver,
            feedback_tx,
            recorder,
            shutdown,
            start_time,
        );
    });
}
