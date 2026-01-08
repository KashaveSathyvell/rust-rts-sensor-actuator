use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct ExperimentConfig {
    pub experiment_name: String,
    pub duration_secs: u64,
    pub sensor_period_ms: u64,  // Changed to ms for clarity
    pub cpu_load_threads: usize,
    pub mode: String,
    pub processing_time_ns: u64, // NEW: Configurable busy-wait time
}

pub fn load_config(path: &str) -> Result<ExperimentConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: ExperimentConfig = toml::from_str(&content)?;
    Ok(config)
}

#[derive(Debug, Deserialize)]
pub struct CpuLoadConfig {
    pub enabled: bool,
    pub threads: usize,
}

#[derive(Debug, Deserialize)]
pub struct SharedResourceConfig {
    pub high_contention: bool,
}

impl ExperimentConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: ExperimentConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}
