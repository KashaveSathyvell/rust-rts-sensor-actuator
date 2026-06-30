# RTS Sensor-Actuator Benchmark

A Rust benchmarking system comparing two concurrency models — traditional OS threads versus async/await — in a real-time, deadline-driven sensor-actuator pipeline.

## Overview

The system simulates a high-frequency sensor producing data that gets routed through a dispatcher to three actuators (Gripper, Motor, Stabilizer), each with a strict execution deadline (ranging from roughly 1ms to 1.5ms). It measures **deadline compliance** — the percentage of cycles each actuator completes within its allotted time — under both a threaded implementation and an async (Tokio-based) implementation, then compares the two head-to-head.

## How it works

- **Threaded implementation** (`std::thread` + `mpsc::sync_channel`) — a sensor thread generates data and sends it to a dispatcher thread, which fans it out to three actuator threads, each running with its own deadline constraint. Feedback flows back through a dedicated channel.
- **Async implementation** (Tokio runtime) — mirrors the same sensor → dispatcher → actuator pipeline using async tasks instead of OS threads, to compare scheduling overhead and deadline compliance against the threaded version.
- **Benchmark comparison** — runs both implementations back-to-back under the same configuration and reports deadline compliance and timing for each.
- **Criterion-based benchmarks** — a statistical benchmark suite for more rigorous performance measurement beyond the simple comparison runner.
- **CLI menu** — lets you run the threaded demo, async demo, head-to-head comparison, a real-time dashboard, or the Criterion suite, all from one entry point.

## Tech stack

- **Rust**
- **Tokio** — async runtime for the async implementation
- **Criterion** — statistical benchmarking
- **std::sync::mpsc** — channel-based message passing for the threaded implementation

## Running it

```bash
cargo run
```

This launches the CLI menu where you can choose which demo or benchmark to run.

To run the Criterion benchmark suite directly:

```bash
cargo bench
```

## Background

Built to explore the practical tradeoffs between OS-thread concurrency and async concurrency in a real-time systems context, where missing a deadline is a measurable failure rather than just a performance dip.
