@echo off
echo Running slv-rust with comprehensive debugging enabled...
echo.

echo Setting environment variables for debugging:
set RUST_LOG=debug
set RUST_BACKTRACE=1
set WGPU_LOG=1
set WGPU_VALIDATION=1

echo RUST_LOG=%RUST_LOG%
echo RUST_BACKTRACE=%RUST_BACKTRACE%
echo WGPU_LOG=%WGPU_LOG%
echo WGPU_VALIDATION=%WGPU_VALIDATION%
echo.

echo Building and running the application...
cargo run

echo.
echo Application finished.
pause 