# Testing and Verification Guide

This guide explains how to test and verify that all core system functions are working properly.

## Quick Start

### 1. Build the Project

```powershell
cargo build --release
```

### 2. Run Basic Benchmark

```powershell
cargo run --bin benchmark_runner -- configs/experiment_baseline.toml both
```

This will:
- Run both threaded and async implementations
- Generate detailed statistics
- Save results to CSV files
- Show deadline compliance, latency, and performance metrics

## Detailed Testing Procedures

### Test 1: Basic Functionality Test

**Purpose**: Verify all components are working

```powershell
# Run threaded implementation only
cargo run --bin benchmark_runner -- configs/experiment_baseline.toml threaded

# Run async implementation only
cargo run --bin benchmark_runner -- configs/experiment_baseline.toml async
```

**What to Check**:
- ✅ No compilation errors
- ✅ Experiment completes without crashes
- ✅ CSV files are generated (`threaded_results.csv`, `async_results.csv`)
- ✅ Statistics are printed showing deadline compliance > 0%

### Test 2: Component A - Sensor Data Simulator

**Verify Sensor Data Generation**:
1. Check CSV output - sensor cycles should have `actuator: None`
2. Verify timestamps are increasing
3. Check that force, position, and temperature values vary (not constant)

**Verify Data Processing**:
1. Look for `processing_time_ns` values in CSV (should be > 0)
2. Check that processing times are measured
3. Verify anomaly detection is working (check `SharedDiagnostics`)

**Verify Shared Resource Synchronization**:
1. Check `lock_wait_ns` values in CSV (may be 0 under low contention)
2. Run with high contention config to see lock wait times

**Verify Transmission**:
1. Check `deadline_met` field - should be mostly `true`
2. Transmission should complete within 0.1ms deadline

### Test 3: Component B - Actuator Commander

**Verify Actuator Reception**:
1. Check CSV - actuator cycles should have `actuator: Some(Gripper/Motor/Stabilizer)`
2. Verify each actuator type appears in results

**Verify Control Algorithm**:
1. Check that `processing_time_ns` is measured for actuators
2. Verify PID controller is working (control_output values should vary)

**Verify Multiple Actuators**:
1. Check that all three actuator types appear in results:
   - Gripper (1ms deadline)
   - Motor (2ms deadline)
   - Stabilizer (1.5ms deadline)
2. Each should have different deadline compliance rates

**Verify Feedback Loop**:
1. Check that feedback is being sent (no errors in console)
2. Verify emergency status propagation (check diagnostics)

### Test 4: Deadline Compliance Testing

**Run with different configurations**:

```powershell
# Baseline (normal load)
cargo run --bin benchmark_runner -- configs/experiment_baseline.toml both

# High contention
cargo run --bin benchmark_runner -- configs/experiment_contention.toml both

# Stress test
cargo run --bin benchmark_runner -- configs/experiment_stress.toml both
```

**What to Check**:
- Deadline compliance percentage (should be > 80% in baseline)
- Average processing times
- Lock wait times (should increase under contention)
- Lateness values (should be minimal in baseline)

### Test 5: Performance Comparison

**Compare Threaded vs Async**:

```powershell
cargo run --bin benchmark_runner -- configs/experiment_baseline.toml both
```

**Compare the output**:
- Which has better deadline compliance?
- Which has lower latency?
- Which has lower lock contention?

### Test 6: CSV Analysis

**Open the generated CSV files** and verify:

1. **Sensor Cycles** (`actuator` is `None`):
   - `processing_time_ns` > 0
   - `total_latency_ns` measured
   - `deadline_met` mostly true

2. **Actuator Cycles** (`actuator` is `Some(...)`):
   - `processing_time_ns` > 0
   - `lock_wait_ns` measured
   - `deadline_met` tracked per actuator
   - `lateness_ns` calculated correctly

3. **Timing Measurements**:
   - All timing fields are populated
   - Values are in nanoseconds
   - No negative values (except lateness_ns which can be 0)

### Test 7: Dynamic Recalibration

**Verify**:
1. Run experiment for longer duration (30+ seconds)
2. Check that actuator error thresholds adjust over time
3. Monitor if error values decrease over time (indicating recalibration)

### Test 8: Anomaly Detection

**Verify**:
1. Check console output for any anomaly/emergency messages
2. Verify `SharedDiagnostics` counters are incrementing when anomalies occur
3. Check that emergency status is propagated through feedback loop

## Expected Results

### Baseline Configuration (10 seconds, 10ms period)

**Threaded Implementation**:
- Total cycles: ~1000-1200 (sensor + 3 actuators)
- Deadline compliance: > 90%
- Avg processing time: < 200μs
- Lock wait time: < 1000ns (low contention)

**Async Implementation**:
- Total cycles: ~1000-1200
- Deadline compliance: > 85%
- Avg processing time: < 200μs
- Lock wait time: < 1000ns

### What Success Looks Like

✅ **All tests pass**:
- No crashes or panics
- CSV files generated with data
- Statistics show reasonable values
- Deadline compliance > 80%
- Processing times measured correctly
- Lock wait times tracked
- All three actuators appear in results

⚠️ **Warnings (acceptable)**:
- Some deadline misses under high load (expected)
- Lock contention increases under stress (expected)
- Processing times may exceed deadlines occasionally (normal on non-RTOS)

❌ **Failures (need investigation)**:
- Crashes or panics
- No data in CSV files
- 0% deadline compliance
- All processing times are 0
- Missing actuator types

## Advanced Testing

### Custom Configuration

Create your own test config:

```toml
experiment_name = "custom_test"
duration_secs = 30
sensor_period_ms = 5
cpu_load_threads = 0
mode = "test"
processing_time_ns = 200_000
```

Then run:
```powershell
cargo run --bin benchmark_runner -- your_config.toml both
```

### Performance Profiling

For detailed performance analysis, use the CSV files with external tools:
- Excel/Python for statistical analysis
- Plot deadline compliance over time
- Compare threaded vs async performance
- Analyze lock contention patterns

## Troubleshooting

### Issue: "No results to analyze"
**Solution**: Check that experiment duration is sufficient (at least 5 seconds)

### Issue: All processing times are 0
**Solution**: Verify timing measurements are implemented (should be fixed in current code)

### Issue: High number of missed deadlines
**Solution**: 
- Reduce sensor period (increase time between cycles)
- Check system load
- Verify deadlines are realistic for your system

### Issue: CSV file is empty
**Solution**: 
- Check file permissions
- Verify experiment completed successfully
- Check console for error messages

## Verification Checklist

- [ ] Project compiles without errors
- [ ] Benchmark runner executes successfully
- [ ] CSV files are generated
- [ ] Statistics are printed
- [ ] Sensor data generation works (varying values)
- [ ] Data processing works (filtering, anomaly detection)
- [ ] Shared resource synchronization works (lock wait times measured)
- [ ] Transmission timing works (0.1ms deadline tracked)
- [ ] Actuator reception works (all 3 types appear)
- [ ] PID control works (control outputs vary)
- [ ] Multiple actuators work (Gripper, Motor, Stabilizer)
- [ ] Feedback loop works (feedback sent)
- [ ] Dynamic recalibration works (thresholds adjust)
- [ ] Deadline compliance tracked (> 80% in baseline)
- [ ] All timing measurements present in CSV

## Next Steps

After verification:
1. Analyze CSV results for your report
2. Compare threaded vs async performance
3. Test different synchronization strategies (if implemented)
4. Document findings and trade-offs
5. Prepare performance analysis charts






