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

        thread::spawn(move || {
            while let Ok(data) = dispatcher_rx.recv() {
                let _ = tx1.try_send(data);
                let _ = tx2.try_send(data);
                let _ = tx3.try_send(data);
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
    thread::sleep(Duration::from_secs(config.duration_secs));
    shutdown_flag.store(true, Ordering::Relaxed);

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
