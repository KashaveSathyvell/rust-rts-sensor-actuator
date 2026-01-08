use std::sync::atomic::{AtomicBool};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;

use common::{BenchmarkRecorder, ExperimentConfig, SharedDiagnostics};

mod sensor;
mod actuator;

pub async fn run_experiment(config: ExperimentConfig) -> Arc<BenchmarkRecorder> {
    let recorder = Arc::new(BenchmarkRecorder::new());
    let diagnostics = Arc::new(SharedDiagnostics::default());
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    let (sensor_tx, actuator_rx) = mpsc::channel(100);
    let (feedback_tx, feedback_rx) = mpsc::channel(100);

    let s_cfg = config.clone();
    let s_rec = recorder.clone();
    let s_diag = diagnostics.clone();
    let s_shutdown = shutdown_flag.clone();

    tokio::spawn(async move {
        sensor::run_sensor_task(
            s_cfg,
            sensor_tx,
            feedback_rx,
            s_rec,
            s_diag,
            s_shutdown,
            start_time,
        ).await;
    });

    let a_cfg = config.clone();
    let a_rec = recorder.clone();
    let a_diag = diagnostics.clone();
    let a_shutdown = shutdown_flag.clone();

    tokio::spawn(async move {
        actuator::run_actuator_task(
            a_cfg,
            actuator_rx,
            feedback_tx,
            a_rec,
            a_diag,
            a_shutdown,
            start_time,
        ).await;
    });

    tokio::time::sleep(Duration::from_secs(config.duration_secs)).await;
    shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);

    recorder
}
