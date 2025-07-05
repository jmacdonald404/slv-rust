Write-Host "Running slv-rust with comprehensive debugging enabled..." -ForegroundColor Green
Write-Host ""

Write-Host "Setting environment variables for debugging:" -ForegroundColor Yellow
$env:RUST_LOG = "debug"
$env:RUST_BACKTRACE = "1"
$env:WGPU_LOG = "1"
$env:WGPU_VALIDATION = "1"

Write-Host "RUST_LOG=$env:RUST_LOG" -ForegroundColor Cyan
Write-Host "RUST_BACKTRACE=$env:RUST_BACKTRACE" -ForegroundColor Cyan
Write-Host "WGPU_LOG=$env:WGPU_LOG" -ForegroundColor Cyan
Write-Host "WGPU_VALIDATION=$env:WGPU_VALIDATION" -ForegroundColor Cyan
Write-Host ""

Write-Host "Building and running the application..." -ForegroundColor Yellow
cargo run

Write-Host ""
Write-Host "Application finished." -ForegroundColor Green
Read-Host "Press Enter to continue" 