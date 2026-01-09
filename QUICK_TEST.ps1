# Quick Test Script for Real-Time Sensor-Actuator System
# Run this to verify everything is working

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Real-Time Sensor-Actuator System Test" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build the project
Write-Host "Step 1: Building project..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "✓ Build successful" -ForegroundColor Green
Write-Host ""

# Step 2: Run baseline test
Write-Host "Step 2: Running baseline benchmark..." -ForegroundColor Yellow
cargo run --release --bin benchmark_runner -- configs/experiment_baseline.toml both
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Benchmark failed!" -ForegroundColor Red
    exit 1
}
Write-Host ""

# Step 3: Check output files
Write-Host "Step 3: Verifying output files..." -ForegroundColor Yellow
if (Test-Path "threaded_results.csv") {
    $threadedLines = (Get-Content "threaded_results.csv" | Measure-Object -Line).Lines
    Write-Host "✓ threaded_results.csv exists ($threadedLines lines)" -ForegroundColor Green
} else {
    Write-Host "✗ threaded_results.csv missing!" -ForegroundColor Red
}

if (Test-Path "async_results.csv") {
    $asyncLines = (Get-Content "async_results.csv" | Measure-Object -Line).Lines
    Write-Host "✓ async_results.csv exists ($asyncLines lines)" -ForegroundColor Green
} else {
    Write-Host "✗ async_results.csv missing!" -ForegroundColor Red
}
Write-Host ""

# Step 4: Summary
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Test Complete!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Review the CSV files for detailed metrics"
Write-Host "2. Check TESTING_GUIDE.md for comprehensive testing procedures"
Write-Host "3. Run with different configs: experiment_contention.toml, experiment_stress.toml"
Write-Host ""






