use common::config::{load_config, ExperimentConfig};
use common::metrics::CycleResult;
use common::sync_strategies::{SyncStrategy, MutexStrategy, RwLockStrategy, AtomicStrategy};
use common::ActuatorType;
use criterion::{black_box, Criterion};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn analyze_results_detailed(results: &[CycleResult], name: &str) {
    if results.is_empty() {
        println!("{}: No results to analyze", name);
        return;
    }

    let total = results.len();
    let missed_deadlines = results.iter().filter(|r| !r.deadline_met).count();
    let deadline_rate = (1.0 - (missed_deadlines as f64 / total as f64)) * 100.0;

    let processing_times: Vec<f64> = results.iter()
        .map(|r| r.processing_time_ns as f64 / 1000.0) // Convert to μs
        .collect();

    let latencies: Vec<f64> = results.iter()
        .map(|r| r.total_latency_ns as f64 / 1000.0) // Convert to μs
        .filter(|&l| l > 0.0)
        .collect();

    let lateness_values: Vec<i64> = results.iter()
        .map(|r| r.lateness_ns)
        .collect();

    println!("\n=== {} Detailed Analysis ===", name);
    println!("Total cycles: {}", total);
    println!("Deadline compliance: {:.2}% ({} missed)", deadline_rate, missed_deadlines);

    if !processing_times.is_empty() {
        let avg_proc = processing_times.iter().sum::<f64>() / processing_times.len() as f64;
        let min_proc = processing_times.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_proc = processing_times.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        println!("Processing time (μs): avg={:.2}, min={:.2}, max={:.2}", avg_proc, min_proc, max_proc);
    }

    if !latencies.is_empty() {
        let avg_lat = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let min_lat = latencies.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_lat = latencies.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        println!("Total latency (μs): avg={:.2}, min={:.2}, max={:.2}", avg_lat, min_lat, max_lat);
    }

    let max_lateness = lateness_values.iter().max().unwrap_or(&0);
    let late_count = lateness_values.iter().filter(|&&l| l > 0).count();
    println!("Max lateness: {} ns", max_lateness);
    println!("Cycles with lateness: {} ({:.2}%)", late_count, (late_count as f64 / total as f64) * 100.0);

    // Analyze by actuator type
    let actuator_results: HashMap<_, _> = results.iter()
        .filter(|r| r.actuator.is_some())
        .fold(HashMap::new(), |mut acc, r| {
            let actuator = r.actuator.unwrap();
            acc.entry(actuator).or_insert_with(Vec::new).push(r);
            acc
        });

    if !actuator_results.is_empty() {
        println!("\nActuator Performance:");
        for (actuator, acts) in actuator_results {
            let act_missed = acts.iter().filter(|r| !r.deadline_met).count();
            let act_processing: Vec<f64> = acts.iter().map(|r| r.processing_time_ns as f64 / 1000.0).collect();
            let act_avg_proc = act_processing.iter().sum::<f64>() / act_processing.len() as f64;
            println!("  {:?}: {} cycles, {:.2}% deadline met, avg {:.2} μs",
                actuator, acts.len(),
                (1.0 - (act_missed as f64 / acts.len() as f64)) * 100.0,
                act_avg_proc);
        }
    }
}

fn benchmark_threaded(c: &mut Criterion, config: &ExperimentConfig) {
    let config = config.clone();
    c.bench_function("threaded_experiment", |b| {
        b.iter(|| {
            let recorder = threaded_impl::run_experiment(black_box(config.clone()));
            black_box(recorder.get_results());
        });
    });
}

fn benchmark_async(c: &mut Criterion, config: &ExperimentConfig) {
    let config = config.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("async_experiment", |b| {
        b.iter(|| {
            let recorder = rt.block_on(async_impl::run_experiment(black_box(config.clone())));
            black_box(recorder.get_results());
        });
    });
}

// Synchronization strategy benchmarks
fn benchmark_sync_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_strategies");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    // Benchmark MutexStrategy
    group.bench_function("MutexStrategy", |b| {
        b.iter_batched(
            || create_sync_strategy("mutex"),
            |strategy| benchmark_sync_workload(black_box(strategy)),
            criterion::BatchSize::SmallInput
        );
    });

    // Benchmark RwLockStrategy
    group.bench_function("RwLockStrategy", |b| {
        b.iter_batched(
            || create_sync_strategy("rwlock"),
            |strategy| benchmark_sync_workload(black_box(strategy)),
            criterion::BatchSize::SmallInput
        );
    });

    // Benchmark AtomicStrategy
    group.bench_function("AtomicStrategy", |b| {
        b.iter_batched(
            || create_sync_strategy("atomic"),
            |strategy| benchmark_sync_workload(black_box(strategy)),
            criterion::BatchSize::SmallInput
        );
    });

    group.finish();
}

fn create_sync_strategy(strategy_type: &str) -> Arc<dyn SyncStrategy> {
    match strategy_type {
        "mutex" => Arc::new(MutexStrategy::new()),
        "rwlock" => Arc::new(RwLockStrategy::new()),
        "atomic" => Arc::new(AtomicStrategy::new()),
        _ => panic!("Unknown strategy type"),
    }
}

fn benchmark_sync_workload(strategy: Arc<dyn SyncStrategy>) {
    const NUM_THREADS: usize = 4; // Simulate sensor + 3 actuators
    const OPERATIONS_PER_THREAD: usize = 1000;

    let mut handles = vec![];

    for thread_id in 0..NUM_THREADS {
        let strategy_clone = Arc::clone(&strategy);

        let handle = thread::spawn(move || {
            for i in 0..OPERATIONS_PER_THREAD {
                let cycle_id = thread_id * OPERATIONS_PER_THREAD + i;

                // Create a realistic CycleResult
                let result = CycleResult {
                    cycle_id: cycle_id as u64,
                    mode: "benchmark".to_string(),
                    actuator: if thread_id > 0 {
                        Some(match thread_id {
                            1 => ActuatorType::Gripper,
                            2 => ActuatorType::Motor,
                            _ => ActuatorType::Stabilizer,
                        })
                    } else {
                        None // Sensor thread
                    },
                    total_latency_ns: (cycle_id % 1000) as u64 + 1000, // Some variance
                    processing_time_ns: 50 + (cycle_id % 100) as u64, // Processing time
                    lock_wait_ns: (cycle_id % 50) as u64, // Lock contention
                    deadline_met: cycle_id % 100 != 0, // ~1% deadline misses
                    lateness_ns: if cycle_id % 100 == 0 { 500 } else { 0 }, // Occasional lateness
                };

                // Record the result (write operation)
                strategy_clone.record(result);

                // Occasionally perform read operations to simulate dashboard/monitoring
                if i % 100 == 0 {
                    let _missed = strategy_clone.get_missed_deadlines();
                    let _count = strategy_clone.get_results_count();
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

fn run_sync_strategy_csv_benchmarks() {
    println!("Running Synchronization Strategy CSV Benchmarks...");
    println!("This will generate CSV files for report analysis.\n");

    let strategies = vec![
        ("MutexStrategy", create_sync_strategy("mutex")),
        ("RwLockStrategy", create_sync_strategy("rwlock")),
        ("AtomicStrategy", create_sync_strategy("atomic")),
    ];

    for (name, strategy) in strategies {
        println!("Benchmarking {}...", name);

        let start = Instant::now();
        benchmark_sync_workload(Arc::clone(&strategy));
        let duration = start.elapsed();

        let results = strategy.get_results();
        let missed = strategy.get_missed_deadlines();

        println!("  Completed in {:.2}ms", duration.as_millis());
        println!("  Recorded {} results, {} missed deadlines", results.len(), missed);

        // Save to CSV
        let filename = format!("{}_benchmark.csv", name.to_lowercase());
        strategy.save_to_csv(&filename).expect("Failed to save CSV");

        // Also save detailed analysis
        analyze_sync_strategy_performance(&results, name);
    }

    println!("\n========================================");
    println!("Sync Strategy CSV Benchmarks Complete!");
    println!("========================================");
    println!("Generated files:");
    println!("  - mutexstrategy_benchmark.csv");
    println!("  - rwlockstrategy_benchmark.csv");
    println!("  - atomicstrategy_benchmark.csv");
}

fn analyze_sync_strategy_performance(results: &[CycleResult], strategy_name: &str) {
    if results.is_empty() {
        return;
    }

    let total_operations = results.len();
    let avg_processing = results.iter().map(|r| r.processing_time_ns).sum::<u64>() as f64 / total_operations as f64;
    let avg_lock_wait = results.iter().map(|r| r.lock_wait_ns).sum::<u64>() as f64 / total_operations as f64;
    let max_lock_wait = results.iter().map(|r| r.lock_wait_ns).max().unwrap_or(0);
    let missed_deadlines = results.iter().filter(|r| !r.deadline_met).count();

    println!("  {} Performance Analysis:", strategy_name);
    println!("    Average processing time: {:.1} ns", avg_processing);
    println!("    Average lock wait time: {:.1} ns", avg_lock_wait);
    println!("    Maximum lock wait time: {} ns", max_lock_wait);
    println!("    Deadline compliance: {:.2}%", (total_operations - missed_deadlines) as f64 / total_operations as f64 * 100.0);
}


fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: benchmark_runner <config_file> [threaded|async|both|sync] [--criterion]");
        eprintln!("Examples:");
        eprintln!("  benchmark_runner configs/experiment_baseline.toml both              # Normal experiments");
        eprintln!("  benchmark_runner configs/experiment_baseline.toml both --criterion # Statistical analysis");
        eprintln!("  benchmark_runner configs/experiment_baseline.toml sync             # Sync strategy CSV benchmarks");
        eprintln!("  benchmark_runner configs/experiment_baseline.toml sync --criterion # Sync strategy Criterion benchmarks");
        std::process::exit(1);
    }

    let config_path = &args[1];
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("both");
    let use_criterion = args.contains(&"--criterion".to_string());
    let sync_only = mode == "sync";

    let mut config = load_config(config_path).expect("Failed to load config");

    // Disable logging during Criterion benchmarks for methodological validity
    if use_criterion {
        config.enable_logging = false;
    }

    println!("========================================");
    println!("Real-Time Sensor-Actuator Benchmark");
    println!("========================================");
    println!("Config: {}", config_path);
    println!("Experiment: {}", config.experiment_name);
    println!("Duration: {} seconds", config.duration_secs);
    println!("Sensor period: {} ms", config.sensor_period_ms);
    println!("Mode: {}", config.mode);
    if use_criterion {
        println!("Using Criterion for statistical analysis");
        println!("Logging disabled for benchmark validity");
    }
    println!("========================================\n");

    if use_criterion {
        // Use criterion for statistical benchmarking
        let mut criterion = Criterion::default()
            .sample_size(20)
            .measurement_time(std::time::Duration::from_secs(30));

        if sync_only {
            println!("Running SYNCHRONIZATION STRATEGY statistical benchmarks...");
            benchmark_sync_strategies(&mut criterion);

            println!("\n========================================");
            println!("Sync Strategy Benchmarking Complete!");
            println!("========================================");
            println!("HTML reports available in: target/criterion/sync_strategies/");
            println!("This benchmark compares MutexStrategy vs RwLockStrategy vs AtomicStrategy");
            println!("under realistic multi-threaded contention patterns.");
            return;
        }

        if mode == "threaded" || mode == "both" {
            println!("Running THREADED statistical benchmarks...");
            benchmark_threaded(&mut criterion, &config);
        }

        if mode == "async" || mode == "both" {
            println!("\nRunning ASYNC statistical benchmarks...");
            benchmark_async(&mut criterion, &config);
        }

        println!("\n========================================");
        println!("Criterion statistical analysis complete!");
        println!("Check the target/criterion directory for detailed HTML reports.");
        println!("========================================");
    } else if sync_only {
        run_sync_strategy_csv_benchmarks();
        return;
    } else {
        // Run normal experiments
        if mode == "threaded" || mode == "both" {
            println!("Running THREADED experiment...");
            let start = std::time::Instant::now();
            let threaded_recorder = threaded_impl::run_experiment(config.clone());
            let elapsed = start.elapsed();

            println!("Threaded experiment completed in {:.2} seconds", elapsed.as_secs_f64());

            // Get results for analysis
            let results = threaded_recorder.get_results();
            analyze_results_detailed(&results, "THREADED");

            threaded_recorder
                .save_to_csv("threaded_results.csv")
                .expect("Failed to save threaded CSV");
            println!("Results saved to threaded_results.csv");
        }

        if mode == "async" || mode == "both" {
            println!("\nRunning ASYNC experiment...");
            let start = std::time::Instant::now();
            let async_recorder = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async_impl::run_experiment(config.clone()));
            let elapsed = start.elapsed();

            println!("Async experiment completed in {:.2} seconds", elapsed.as_secs_f64());

            // Get results for analysis
            let results = async_recorder.get_results();
            analyze_results_detailed(&results, "ASYNC");

            async_recorder
                .save_to_csv("async_results.csv")
                .expect("Failed to save async CSV");
            println!("Results saved to async_results.csv");
        }

        println!("\n========================================");
        println!("Benchmark complete!");
        println!("========================================");
    }
}
