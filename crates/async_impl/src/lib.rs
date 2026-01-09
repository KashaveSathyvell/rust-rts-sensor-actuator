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

        tokio::spawn(async move {
            while let Some(data) = dispatcher_rx.recv().await {
                // SensorData is Copy, so we can clone it cheaply for each actuator
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
    tokio::time::sleep(Duration::from_secs(config.duration_secs)).await;
    shutdown_flag.store(true, Ordering::Relaxed);

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
