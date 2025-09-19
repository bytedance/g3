# g3 Project Rust PGO User Guide

## Overview

Profile-Guided Optimization (PGO) is a compiler optimization technique that uses runtime performance data to guide the compiler in generating more efficient code. This guide explains how to use Rust PGO in the g3 project.

## Prerequisites

### System Requirements

- Rust toolchain (latest stable recommended)
- `cargo-pgo` (installed via `cargo install cargo-pgo`)
- `llvm-profdata` tool (installed via rustup)
- Sufficient memory and storage space

### Environment Verification

```bash
# Check Rust version
rustc --version

# Check if llvm-profdata is available
rustup component list | grep llvm-tools-preview
```

If llvm-tools-preview is missing, install it:

```bash
rustup component add llvm-tools-preview
```

## Usage

### Basic Usage

The simplest way to use PGO is to run with default configuration:

```bash
cd g3
./scripts/pgo/pgo_rust.sh
```

### Custom Component Selection

Select specific components for optimization:

```bash
# Optimize a single component
./scripts/pgo/pgo_rust.sh --components g3mkcert

# Optimize multiple components
./scripts/pgo/pgo_rust.sh --components g3mkcert,g3keymess-ctl,g3fcgen

# Optimize all available components
./scripts/pgo/pgo_rust.sh --all
```

### Performance Benchmarking

Run performance benchmarks after optimization to verify effectiveness:

```bash
# Optimize and automatically run benchmarks
./scripts/pgo/pgo_rust.sh --components g3mkcert --benchmark

# Or run benchmarks separately
./scripts/pgo/pgo_rust.sh --benchmark
```

## Available Components

- `g3mkcert` - Certificate generation tool
- `g3proxy-ctl` - Proxy control tool
- `g3proxy` - Main proxy service
- `g3bench` - Performance testing tool
- `g3fcgen` - Flow control generator
- `g3iploc` - IP location service
- `g3keymess` - Key management service
- `g3keymess-ctl` - Key management control tool
- `g3statsd` - Statistics data collector
- `g3statsd-ctl` - Statistics data control tool
- `g3tiles` - Tile service
- `g3tiles-ctl` - Tile control tool
- `g3proxy-ftp` - FTP proxy tool

## PGO Workflow

### 1. Instrumentation Build

The script builds binaries with the `-C profile-generate` flag:

```bash
RUSTFLAGS="-C profile-generate=/tmp/pgo-data" cargo build --release -p <component>
```

### 2. Profile Data Collection

Run instrumented binaries to collect runtime data:

```bash
target/release/g3mkcert --version
target/release/g3mkcert --help
target/release/g3mkcert generate --ca-key /tmp/ca.key --ca-cert /tmp/ca.crt
```

### 3. Profile Data Merging

Use `llvm-profdata` to merge collected profile data:

```bash
llvm-profdata merge -output=/tmp/pgo-data/merged.profdata /tmp/pgo-data/*.profraw
```

### 4. Optimized Build

Rebuild using merged profile data:

```bash
RUSTFLAGS="-C profile-use=/tmp/pgo-data/merged.profdata" cargo build --release -p <component>
```

## Output Information

### Successful Output Example

```text
[INFO] Starting Rust PGO optimization process...
[INFO] Using specified components: g3mkcert
[INFO] Checking Rust PGO prerequisites...
[INFO] Prerequisites check passed
[INFO] Building instrumented binaries for profile generation...
[INFO] Generating PGO profiles...
[INFO] Profile files found: 42
[INFO] Building optimized binaries using profile data...
[INFO] PGO optimization completed successfully!
```

### Optimized Binary Locations

- Optimized binaries: `target/release/`
- Profile data: `/tmp/pgo-data/`

## Performance Verification

### Automatic Benchmarking

Use the `--benchmark` option to automatically compare performance before and after optimization:

```bash
./scripts/pgo/pgo_rust.sh --components g3mkcert --benchmark
```

**Note**: The script will automatically use `hyperfine` if available for more precise measurements, otherwise it falls back to the basic `time` command. For best results, install `hyperfine`:

```bash
cargo install hyperfine
```

Example output with `time`:

```text
=== Baseline Performance (without PGO) ===
Testing g3mkcert operations...
real    0m0.152s

=== PGO-Optimized Performance ===  
Testing PGO-optimized g3mkcert operations...
real    0m0.135s
```

Example output with `hyperfine`:

```text
=== PGO-Optimized Performance ===
Comparing baseline vs PGO-optimized g3mkcert...
Benchmark 1: /tmp/g3mkcert-baseline --version
  Time (mean ± σ):       4.0 ms ±   1.2 ms    [User: 2.1 ms, System: 1.9 ms]
  Range (min … max):     2.8 ms …   8.1 ms    15 runs

Benchmark 2: target/release/g3mkcert --version  
  Time (mean ± σ):       3.6 ms ±   1.1 ms    [User: 1.8 ms, System: 1.8 ms]
  Range (min … max):     2.5 ms …   6.9 ms    15 runs

Summary
  target/release/g3mkcert --version ran 1.11 ± 0.38 times faster than /tmp/g3mkcert-baseline --version
```

### Manual Verification

For more precise measurements, `hyperfine` is recommended. You can install it via `cargo install hyperfine`.

```bash
# Install hyperfine if you haven't already
cargo install hyperfine

# Create a baseline build first
cargo build --release --bin g3mkcert

# Save the baseline binary
mv target/release/g3mkcert /tmp/g3mkcert-baseline

# Run PGO optimization
./scripts/pgo/pgo_rust.sh --components g3mkcert

# Compare baseline vs PGO-optimized version
hyperfine --shell=none --warmup 3 '/tmp/g3mkcert-baseline --version' 'target/release/g3mkcert --version'
```

This provides a statistically robust comparison of the performance before and after PGO.

## Known Limitations

### Memory Usage

- `g3proxy` component requires substantial memory during PGO build
- If memory is insufficient, avoid selecting `g3proxy` or use default components

### Disk Space

- Profile data may consume several hundred MB of space
- Ensure `/tmp/pgo-data` has sufficient space

## Troubleshooting

### Common Issues

**Issue**: `llvm-profdata not found`

**Solution**: Install llvm-tools-preview component

```bash
rustup component add llvm-tools-preview
```

**Issue**: Out of memory error

**Solution**: Reduce concurrent builds or select fewer components

```bash
# Select only lightweight components
./scripts/pgo/pgo_rust.sh --components g3mkcert,g3keymess-ctl
```

**Issue**: Profile data not generated

**Solution**: Check permissions and disk space

```bash
# Check /tmp permissions
ls -la /tmp/pgo-data/

# Clean and retry
rm -rf /tmp/pgo-data
```

## Best Practices

### Component Selection Strategy

1. **Development Testing**: Use default components (`g3mkcert`, `g3proxy-ctl`)
2. **Lightweight Optimization**: Select a few key components
3. **Full Optimization**: Use `--all` on high-spec machines

### Performance Testing Recommendations

1. Always compare before/after in the same environment
2. Run multiple tests and take averages
3. Focus on real-world usage scenario metrics

### Automation Integration

PGO can be integrated into CI/CD pipelines:

```bash
# Use PGO in release builds
./scripts/pgo/pgo_rust.sh --components g3mkcert,g3proxy-ctl
cp target/release/* ./release-artifacts/
```

## More Information

- For issues, please check project Issues or submit a new Issue
