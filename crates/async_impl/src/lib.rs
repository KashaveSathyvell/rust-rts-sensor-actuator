use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration, Instant};

use common::{BenchmarkRecorder, ExperimentConfig, SensorData};

mod sensor;
mod actuator;

pub fn run_experiment(config: ExperimentConfig) -> Arc<BenchmarkRecorder> {
    // Create a dedicated Tokio runtime for async benchmark
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    rt.block_on(async {
        // --- Single shared clock (CRITICAL) ---
        let start_time = Instant::now();

        // --- Shared state ---
        let recorder = Arc::new(BenchmarkRecorder::new());
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shared_resource = Arc::new(Mutex::new(()));

        // Bounded channel to simulate backpressure
        let (sender, receiver) = mpsc::channel::<SensorData>(100);

        // --- Spawn async sensor ---
        let sensor_config = config.clone();
        let sensor_recorder = Arc::clone(&recorder);
        let sensor_shutdown = Arc::clone(&shutdown_flag);

        let sensor_handle = tokio::spawn(sensor::run_sensor_task(
            sensor_config,
            sender,
            sensor_recorder,
            sensor_shutdown,
            start_time,
        ));

        // --- Spawn async actuator ---
        let actuator_config = config.clone();
        let actuator_recorder = Arc::clone(&recorder);
        let actuator_shutdown = Arc::clone(&shutdown_flag);
        let actuator_resource = Arc::clone(&shared_resource);

        let actuator_handle = tokio::spawn(actuator::run_actuator_task(
            actuator_config,
            receiver,
            actuator_recorder,
            actuator_resource,
            actuator_shutdown,
            start_time,
        ));

        // --- Run experiment for configured duration ---
        sleep(Duration::from_secs(config.duration_secs)).await;

        // --- Signal shutdown ---
        shutdown_flag.store(true, Ordering::Relaxed);

        // --- Wait for tasks to exit ---
        let _ = tokio::join!(sensor_handle, actuator_handle);

        recorder
    })
}
