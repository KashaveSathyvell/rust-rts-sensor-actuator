use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use common::{BenchmarkRecorder, ExperimentConfig, SensorData};

mod actuator;
mod sensor;

pub fn run_experiment(config: ExperimentConfig) -> Arc<BenchmarkRecorder> {
    let recorder = Arc::new(BenchmarkRecorder::new());
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shared_resource = Arc::new(Mutex::new(()));

    let (sender, receiver) = mpsc::sync_channel::<SensorData>(100);
    let start_time = Instant::now();

    let sensor_config = config.clone();
    let sensor_recorder = Arc::clone(&recorder);
    let sensor_shutdown = Arc::clone(&shutdown_flag);
    let sensor_handle = thread::spawn(move || {
        sensor::run_sensor_thread(
            sensor_config,
            sender,
            sensor_recorder,
            sensor_shutdown,
            start_time,
        )
    });

    let actuator_config = config.clone();
    let actuator_recorder = Arc::clone(&recorder);
    let actuator_shutdown = Arc::clone(&shutdown_flag);
    let actuator_resource = Arc::clone(&shared_resource);
    let actuator_handle = thread::spawn(move || {
        actuator::run_actuator_thread(
            actuator_config,
            receiver,
            actuator_recorder,
            actuator_resource,
            actuator_shutdown,
            start_time,
        )
    });

    thread::sleep(Duration::from_secs(config.duration_secs));

    shutdown_flag.store(true, Ordering::Relaxed);

    let _ = sensor_handle.join();
    let _ = actuator_handle.join();

    recorder
}