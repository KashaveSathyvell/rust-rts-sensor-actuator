mod menu;

use common::config::load_config;

fn main() {
    println!("===========================================");
    println!("Welcome to Real-Time Sensor-Actuator System");
    println!("===========================================");

    loop {
        menu::show_menu();

        match menu::get_user_choice() {
            Ok(1) => run_threaded_demo(),
            Ok(2) => run_async_demo(),
            Ok(3) => run_benchmark_comparison(),
            Ok(4) => run_realtime_dashboard(),
            Ok(5) => {
                println!("Goodbye!");
                break;
            }
            _ => println!("Invalid choice. Please select 1-5."),
        }
    }
}

fn run_threaded_demo() {
    println!("\n=== Running Threaded Implementation Demo ===");

    let mut config = load_config("configs/experiment_baseline.toml")
        .expect("Failed to load config");
    config.enable_logging = true; // Enable logging for demo

    println!("Configuration: {} mode, {}ms sensor period, {} seconds duration",
             config.mode, config.sensor_period_ms, config.duration_secs);

    let recorder = threaded_impl::run_experiment(config);
    display_results(&recorder.get_results());

    menu::wait_for_enter();
}

fn run_async_demo() {
    println!("\n=== Running Async Implementation Demo ===");

    let mut config = load_config("configs/experiment_baseline.toml")
        .expect("Failed to load config");
    config.enable_logging = true; // Enable logging for demo

    println!("Configuration: {} mode, {}ms sensor period, {} seconds duration",
             config.mode, config.sensor_period_ms, config.duration_secs);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let recorder = rt.block_on(async_impl::run_experiment(config));
    display_results(&recorder.get_results());

    menu::wait_for_enter();
}

fn run_benchmark_comparison() {
    println!("\n=== Running Benchmark Comparison (Async vs Threaded) ===");

    let config_path = "configs/experiment_baseline.toml";
    let mut config = load_config(config_path).expect("Failed to load config");
    config.enable_logging = false; // Disable logging for valid benchmarks

    println!("Benchmark Configuration:");
    println!("- Config: {}", config_path);
    println!("- Duration: {} seconds", config.duration_secs);
    println!("- Sensor period: {} ms", config.sensor_period_ms);
    println!("- Logging disabled for methodological validity");

    println!("\n--- Running THREADED Implementation ---");
    let threaded_start = std::time::Instant::now();
    let threaded_recorder = threaded_impl::run_experiment(config.clone());
    let threaded_duration = threaded_start.elapsed();

    let threaded_results = threaded_recorder.get_results();
    let threaded_missed = threaded_results.iter().filter(|r| !r.deadline_met).count();
    let threaded_compliance = if !threaded_results.is_empty() {
        (threaded_results.len() - threaded_missed) as f64 / threaded_results.len() as f64 * 100.0
    } else { 0.0 };

    println!("Threaded Results:");
    println!("- Execution time: {:.2}s", threaded_duration.as_secs_f64());
    println!("- Total cycles: {}", threaded_results.len());
    println!("- Deadline compliance: {:.1}% ({} missed)", threaded_compliance, threaded_missed);

    println!("\n--- Running ASYNC Implementation ---");
    let async_start = std::time::Instant::now();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let async_recorder = rt.block_on(async_impl::run_experiment(config.clone()));
    let async_duration = async_start.elapsed();

    let async_results = async_recorder.get_results();
    let async_missed = async_results.iter().filter(|r| !r.deadline_met).count();
    let async_compliance = if !async_results.is_empty() {
        (async_results.len() - async_missed) as f64 / async_results.len() as f64 * 100.0
    } else { 0.0 };

    println!("Async Results:");
    println!("- Execution time: {:.2}s", async_duration.as_secs_f64());
    println!("- Total cycles: {}", async_results.len());
    println!("- Deadline compliance: {:.1}% ({} missed)", async_compliance, async_missed);

    println!("\n=== Benchmark Comparison Summary ===");
    let time_diff = if async_duration > threaded_duration {
        format!("Async slower by {:.2}s", (async_duration - threaded_duration).as_secs_f64())
    } else {
        format!("Threaded slower by {:.2}s", (threaded_duration - async_duration).as_secs_f64())
    };

    println!("- Performance: {}", time_diff);
    println!("- Threaded compliance: {:.1}%", threaded_compliance);
    println!("- Async compliance: {:.1}%", async_compliance);

    menu::wait_for_enter();
}

fn run_realtime_dashboard() {
    println!("\n=== Launching Real-Time Dashboard ===");
    println!("Note: Close the GUI window to return to menu");

    let config_path = "configs/experiment_baseline.toml";
    let mode = "async";

    // Launch the visualiser - this will block until the GUI window is closed
    // We can't directly call the visualiser's main function from here,
    // so we'll execute it as a subprocess
    match std::process::Command::new("cargo")
        .args(&["run", "--release", "--bin", "visualiser", config_path, mode])
        .status() {
        Ok(status) if status.success() => {
            println!("Dashboard closed successfully.");
        }
        Ok(status) => {
            println!("Dashboard exited with status: {}", status);
        }
        Err(e) => {
            println!("Failed to launch dashboard: {}", e);
            println!("Make sure you have the visualiser binary available.");
        }
    }

    menu::wait_for_enter();
}

fn display_results(results: &[common::metrics::CycleResult]) {
    if results.is_empty() {
        println!("No results to display.");
        return;
    }

    let total_cycles = results.len();
    let missed_deadlines = results.iter().filter(|r| !r.deadline_met).count();
    let deadline_compliance = (total_cycles - missed_deadlines) as f64 / total_cycles as f64 * 100.0;

    println!("\n=== Experiment Results ===");
    println!("Total Cycles: {}", total_cycles);
    println!("Deadline Compliance: {:.2}% ({} missed)", deadline_compliance, missed_deadlines);

    // Show actuator breakdown
    let mut actuator_stats = std::collections::HashMap::new();
    for result in results {
        if let Some(actuator) = &result.actuator {
            let entry = actuator_stats.entry(*actuator).or_insert((0, 0));
            entry.0 += 1; // total cycles
            if !result.deadline_met {
                entry.1 += 1; // missed deadlines
            }
        }
    }

    if !actuator_stats.is_empty() {
        println!("Actuator Performance:");
        for (actuator, (total, missed)) in actuator_stats {
            let compliance = if total > 0 {
                (total - missed) as f64 / total as f64 * 100.0
            } else { 0.0 };
            println!("- {:?}: {:.1}% compliance ({} cycles)", actuator, compliance, total);
        }
    }
}
