use std::sync::atomic::{AtomicBool};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use common::{BenchmarkRecorder, ExperimentConfig, SharedDiagnostics};

mod sensor;
mod actuator;

pub fn run_experiment(config: ExperimentConfig) -> Arc<BenchmarkRecorder> {
    let recorder = Arc::new(BenchmarkRecorder::new());
    let diagnostics = Arc::new(SharedDiagnostics::default());
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    let (sensor_tx, actuator_rx) = mpsc::sync_channel(100);
    let (feedback_tx, feedback_rx) = mpsc::sync_channel(100);

    let s_cfg = config.clone();
    let s_rec = recorder.clone();
    let s_diag = diagnostics.clone();
    let s_shutdown = shutdown_flag.clone();

    let sensor = thread::spawn(move || {
        sensor::run_sensor_thread(
            s_cfg,
            sensor_tx,
            feedback_rx,
            s_rec,
            s_diag,
            s_shutdown,
            start_time,
        );
    });

    let a_cfg = config.clone();
    let a_rec = recorder.clone();
    let a_diag = diagnostics.clone();
    let a_shutdown = shutdown_flag.clone();

    let actuator = thread::spawn(move || {
        actuator::run_actuator_thread(
            a_cfg,
            actuator_rx,
            feedback_tx,
            a_rec,
            a_diag,
            a_shutdown,
            start_time,
        );
    });

    thread::sleep(Duration::from_secs(config.duration_secs));
    shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);

    let _ = sensor.join();
    let _ = actuator.join();

    recorder
}
