use common::config::load_config;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: benchmark_runner <config_file>");
        std::process::exit(1);
    }

    let config_path = &args[1];
    let config = load_config(config_path).expect("Failed to load config");

    println!("Running THREADED experiment...");
    let threaded_recorder = threaded_impl::run_experiment(config.clone());
    threaded_recorder
        .save_to_csv("threaded_results.csv")
        .expect("Failed to save threaded CSV");

    println!("Running ASYNC experiment...");
    let async_recorder = async_impl::run_experiment(config.clone());
    async_recorder
        .save_to_csv("async_results.csv")
        .expect("Failed to save async CSV");

    println!("Benchmark complete.");
}
