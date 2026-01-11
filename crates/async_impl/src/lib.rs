use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::time::Instant;

use tokio::sync::mpsc;
use tokio::time::Duration;

use common::{
    ActuatorFeedback, ActuatorType, BenchmarkRecorder,
    ExperimentConfig, SensorData, SharedDiagnostics,
    DashboardBuffer,
};

mod actuator;
mod sensor;

pub async fn run_experiment(config: ExperimentConfig) -> Arc<BenchmarkRecorder> {
    run_experiment_with_dashboard(config, None).await
}

pub async fn run_experiment_with_dashboard(
    config: ExperimentConfig,
    dashboard: Option<DashboardBuffer>,
) -> Arc<BenchmarkRecorder> {
    if config.enable_logging {
        println!("===========================================");
        println!("Real-Time Sensor-Actuator System Starting");
        println!("===========================================");
        println!("Configuration: {}", config.experiment_name);
        println!("Duration: {} seconds", config.duration_secs);
        println!("Sensor period: {} ms", config.sensor_period_ms);
        println!("Mode: {}", config.mode);
        println!("Components: Sensor + Dispatcher + 3 Actuators");
        println!("Deadlines: Sensor(0.2ms/0.1ms), Actuators(1-2ms), Feedback(0.5ms)");
        println!("===========================================");
    }

    let recorder = Arc::new(BenchmarkRecorder::new());
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let diagnostics = Arc::new(SharedDiagnostics::default());

    // Sensor -> dispatcher
    let (sensor_tx, mut dispatcher_rx) = mpsc::channel::<SensorData>(100);

    // Dispatcher -> actuators
    let (gripper_tx, gripper_rx) = mpsc::channel::<SensorData>(100);
    let (motor_tx, motor_rx) = mpsc::channel::<SensorData>(100);
    let (stabilizer_tx, stabilizer_rx) = mpsc::channel::<SensorData>(100);

    // Feedback channel
    let (feedback_tx, feedback_rx) = mpsc::channel::<ActuatorFeedback>(100);

    let start_time = Instant::now();

    // ---------------- SENSOR ----------------
    {
        let cfg = config.clone();
        let rec = Arc::clone(&recorder);
        let diag = Arc::clone(&diagnostics);
        let shutdown = Arc::clone(&shutdown_flag);
        let tx = sensor_tx;
        let feedback_recv = feedback_rx;
        let dash = dashboard.clone();

        tokio::spawn(async move {
            sensor::run_sensor_task(
                cfg,
                tx,
                feedback_recv,
                rec,
                diag,
                shutdown,
                start_time,
                dash,
            ).await;
        });
    }

    // ---------------- DISPATCHER ----------------
    {
        let tx1 = gripper_tx.clone();
        let tx2 = motor_tx.clone();
        let tx3 = stabilizer_tx.clone();

        let dispatcher_config = config.clone();
        tokio::spawn(async move {
            let mut cycle_count = 0u64;
            let dispatcher_start = Instant::now();
            if dispatcher_config.enable_logging {
                println!("[{:>8}] [SYSTEM] Dispatcher initialized - routing sensor data to 3 actuators",
                         format!("{:.3}s", dispatcher_start.duration_since(start_time).as_secs_f64()));
            }

            while let Some(data) = dispatcher_rx.recv().await {
                cycle_count += 1;
                // SensorData is Copy, so we can clone it cheaply for each actuator
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
                        println!("[{:>8}] [ERROR] DISPATCHER: Failed to route cycle #{} - channels full",
                                format!("{:.3}s", elapsed), cycle_count);
                    }
                }
            }

            let total_time = dispatcher_start.elapsed().as_secs_f64();
            if config.enable_logging {
                println!("[{:>8}] [SYSTEM] Dispatcher shutdown - processed {} cycles in {:.2}s",
                         format!("{:.3}s", total_time), cycle_count, total_time);
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
        dashboard.clone(),
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
        dashboard.clone(),
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
        dashboard.clone(),
    );

    // ---------------- RUN ----------------
    if config.enable_logging {
        println!("[SYSTEM] Experiment running for {} seconds...", config.duration_secs);
    }

    // Performance monitoring loop
    let recorder_clone = Arc::clone(&recorder);
    let shutdown_flag_clone = Arc::clone(&shutdown_flag);
    let perf_config = config.clone();
    let perf_monitor = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        let mut cycle_count = 0u64;

        loop {
            interval.tick().await;
            let results = recorder_clone.get_results();
            let current_cycles = results.len() as u64;

            if current_cycles > cycle_count {
                let new_cycles = current_cycles - cycle_count;
                let missed = results.iter().rev().take(new_cycles as usize).filter(|r| !r.deadline_met).count();
                let throughput = new_cycles as f64 / 2.0; // cycles per second over 2 second window

                let compliance_rate = if new_cycles > 0 {
                    (new_cycles - missed as u64) as f64 / new_cycles as f64 * 100.0
                } else { 100.0 };
                if perf_config.enable_logging {
                    if config.enable_logging {
                        println!("[PERF] Throughput: {:.1} cycles/sec, Recent compliance: {:.1}% ({}/{} cycles met)",
                                throughput, compliance_rate, new_cycles - missed as u64, new_cycles);
                    }
                }
                cycle_count = current_cycles;
            }

            if shutdown_flag_clone.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    tokio::time::sleep(Duration::from_secs(config.duration_secs)).await;
    shutdown_flag.store(true, Ordering::Relaxed);

    if config.enable_logging {
        println!("===========================================");
        println!("Experiment completed - initiating shutdown");
    }
    tokio::time::sleep(Duration::from_millis(100)).await; // Brief pause for cleanup

    // Wait for performance monitor to finish
    let _ = perf_monitor.await;

    let results = recorder.get_results();
    let total_cycles = results.len();
    let missed_deadlines = results.iter().filter(|r| !r.deadline_met).count();
    let deadline_compliance = if total_cycles > 0 {
        (total_cycles - missed_deadlines) as f64 / total_cycles as f64 * 100.0
    } else { 0.0 };

    let anomalies = diagnostics.anomaly_count.load(Ordering::Relaxed);
    let emergencies = diagnostics.emergency_stops.load(Ordering::Relaxed);

    if config.enable_logging {
        println!("===========================================");
        println!("FINAL SYSTEM RESULTS");
        println!("===========================================");
        println!("Total Cycles: {}", total_cycles);
        println!("Deadline Compliance: {:.2}% ({} missed)", deadline_compliance, missed_deadlines);
        println!("Anomalies Detected: {}", anomalies);
        println!("Emergency Events: {}", emergencies);
        println!("===========================================");
    }

    recorder
}

fn spawn_actuator(
    actuator_type: ActuatorType,
    deadline: Duration,
    config: ExperimentConfig,
    receiver: mpsc::Receiver<SensorData>,
    feedback_tx: mpsc::Sender<ActuatorFeedback>,
    recorder: Arc<BenchmarkRecorder>,
    shutdown: Arc<AtomicBool>,
    start_time: Instant,
    dashboard: Option<DashboardBuffer>,
) {
    tokio::spawn(async move {
        actuator::run_actuator_task(
            actuator_type,
            deadline,
            config,
            receiver,
            feedback_tx,
            recorder,
            shutdown,
            start_time,
            dashboard,
        ).await;
    });
}
