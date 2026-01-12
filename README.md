# Real-Time Sensor-Actuator System in Rust

This project implements a real-time sensor-actuator control system using Rust, comparing multi-threaded and asynchronous concurrency models. The system simulates a robotic manufacturing station with deterministic timing constraints, featuring PID control, anomaly detection, and fail-safe mechanisms. It benchmarks performance characteristics including latency, jitter, and deadline compliance to evaluate concurrency model effectiveness for real-time applications.

## To run the System

Run these commands in order:

```bash
cargo clean
cargo build
cargo run
```
