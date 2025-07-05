# Debugging Guide for slv-rust

This guide explains how to use the comprehensive logging system to debug issues with the application.

## Quick Start

### Windows (PowerShell)
```powershell
.\run_with_debug.ps1
```

### Windows (Command Prompt)
```cmd
run_with_debug.bat
```

### Manual Setup
```bash
# Set environment variables
export RUST_LOG=debug
export RUST_BACKTRACE=1
export WGPU_LOG=1
export WGPU_VALIDATION=1

# Run the application
cargo run
```

## Environment Variables

### RUST_LOG
Controls the logging level for the application:
- `error` - Only errors
- `warn` - Warnings and errors
- `info` - Info, warnings, and errors (default)
- `debug` - Debug, info, warnings, and errors
- `trace` - All log levels

### RUST_BACKTRACE
Enables backtrace generation on panics:
- `0` - Disabled (default)
- `1` - Enabled

### WGPU_LOG
Enables detailed WGPU logging:
- `0` - Disabled
- `1` - Enabled (default)

### WGPU_VALIDATION
Enables WGPU validation layers:
- `0` - Disabled
- `1` - Enabled (default)

## What You'll See

With debugging enabled, you'll see detailed information about:

### System Information
- OS and architecture
- Rust version
- Graphics backend information
- Available WGPU adapters

### WGPU Initialization
- Instance creation
- Surface creation
- Adapter selection with detailed specs
- Device creation with limits and features
- Shader compilation
- Pipeline creation
- Resource loading

### Runtime Information
- Frame rendering details
- Surface errors and warnings
- Memory usage
- Performance metrics

### Error Handling
- Detailed error messages with context
- Stack traces for panics
- WGPU validation errors
- Resource loading failures

## Common Issues and Solutions

### Missing Vulkan Validation Layer
If you see warnings about `VK_LAYER_KHRONOS_validation`:
- This is usually just a warning and doesn't affect functionality
- Install Vulkan SDK if you want validation layers
- The application will still work without them

### WGPU Fatal Errors
The application now provides detailed information about WGPU errors:
- Check the adapter information to ensure compatibility
- Verify your graphics drivers are up to date
- Look for specific error messages in the logs

### Performance Issues
Use the debug logs to identify:
- Resource loading bottlenecks
- Frame rendering issues
- Memory allocation problems

## Log File Output

The logs will show:
- Timestamp and log level
- File and line number where the log was generated
- Thread information
- Detailed error context

Example output:
```
2024-01-15T10:30:45.123Z INFO  slv_rust::main > slv-rust starting up
2024-01-15T10:30:45.124Z INFO  slv_rust::rendering::engine > Initializing WGPU render engine
2024-01-15T10:30:45.125Z INFO  slv_rust::rendering::engine > WGPU instance created successfully
2024-01-15T10:30:45.126Z INFO  slv_rust::rendering::engine > WGPU surface created successfully
2024-01-15T10:30:45.127Z INFO  slv_rust::rendering::engine > WGPU adapter selected
2024-01-15T10:30:45.128Z INFO  slv_rust::rendering::engine > === WGPU Adapter Information ===
2024-01-15T10:30:45.129Z INFO  slv_rust::rendering::engine > Name: NVIDIA GeForce RTX 3080
2024-01-15T10:30:45.130Z INFO  slv_rust::rendering::engine > Backend: Vulkan
2024-01-15T10:30:45.131Z INFO  slv_rust::rendering::engine > Device Type: DiscreteGpu
```

## Troubleshooting

### If the application crashes immediately:
1. Check the logs for initialization errors
2. Verify your graphics drivers are up to date
3. Try running with `RUST_LOG=error` to see only critical errors

### If rendering is slow or glitchy:
1. Check the frame rendering logs
2. Look for surface errors or warnings
3. Verify the adapter information shows your expected GPU

### If resources fail to load:
1. Check the file paths in the logs
2. Verify the assets directory structure
3. Look for specific loading error messages

## Advanced Configuration

You can customize the logging further by modifying the environment variables:

```bash
# Only show WGPU errors
export RUST_LOG=wgpu=error

# Show all debug info for the application
export RUST_LOG=slv_rust=debug

# Show everything
export RUST_LOG=trace
``` 