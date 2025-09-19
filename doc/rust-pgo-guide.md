# g3 Project Rust PGO User Guide

## Overview

Profile-Guided Optimization (PGO) is a compiler optimization technique that uses runtime performance data to guide the compiler in generating more efficient code. This guide explains how to use Rust PGO in the g3 project.

## Prerequisites

### System Requirements

- Rust toolchain (latest stable recommended)
- `cargo-binutils` (installed via `cargo install cargo-binutils`)
- Sufficient memory and storage space

### Environment Verification

```bash
# Check Rust version
rustc --version

# Check if cargo profdata is available
cargo profdata --help
```

If cargo-binutils is missing, install it:

```bash
cargo install cargo-binutils
```

## Usage

### Basic Usage

The simplest way to use PGO is to run with default configuration:

```bash
cd g3
./scripts/pgo/example_run.sh
```

### Custom Component Selection

Select specific components for optimization:

```bash
# Optimize a single component
./scripts/pgo/example_run.sh --components g3mkcert

# Optimize multiple components
./scripts/pgo/example_run.sh --components g3mkcert,g3keymess,g3fcgen

# Optimize all available components
./scripts/pgo/example_run.sh --all
```

### Performance Benchmarking

Run performance benchmarks after optimization to verify effectiveness:

```bash
# Optimize and automatically run benchmarks
./scripts/pgo/example_run.sh --components g3mkcert --benchmark

# Or run benchmarks separately
./scripts/pgo/example_run.sh --benchmark
```

## PGO Workflow

### 1. Instrumentation Build

The script builds binaries with the `-Cprofile-generate` flag (the script internally uses a variable `PGO_DATA_DIR` whose default is `/tmp/pgo-data`):

```bash
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release -p <component>
```

### 2. Profile Data Collection

Run instrumented binaries with comprehensive workloads to collect runtime data:

```bash
# For g3mkcert
target/release/g3mkcert --version
target/release/g3mkcert --help
target/release/g3mkcert --root --common-name "G3 Test CA" --rsa 2048 --output-cert rootCA-rsa.crt --output-key rootCA-rsa.key
target/release/g3mkcert --intermediate --common-name "G3 Intermediate CA" --rsa 2048 --output-cert intermediateCA-rsa.crt --output-key intermediateCA-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key
target/release/g3mkcert --tls-server --host "www.example.com" --host "*.example.net" --rsa 2048 --output-cert tls-server-rsa.crt --output-key tls-server-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key

# For other components - basic functionality testing
target/release/g3proxy --version
target/release/g3proxy --help
```

### 3. Profile Data Merging

Use `cargo profdata` to merge collected profile data:

```bash
cargo profdata -- merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data/*.profraw
```

### 4. Optimized Build

Rebuild using merged profile data:

```bash
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release -p <component>
```

## Output Information

### Successful Output Example

```text
[INFO] Starting Rust PGO optimization process...
[INFO] Project directory: /home/user/g3
[INFO] PGO scripts directory: ./scripts/pgo
[INFO] Using specified components: g3mkcert
[INFO] Checking Rust PGO prerequisites...
[INFO] Prerequisites check passed
[INFO] Cleaning previous builds and profile data...
     Removed 1054 files, 312.4MiB total
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
./scripts/pgo/example_run.sh --components g3mkcert --benchmark
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
./scripts/pgo/example_run.sh --components g3mkcert

# Compare baseline vs PGO-optimized version
hyperfine --shell=none --warmup 3 --runs 10 '/tmp/g3mkcert-baseline --version' 'target/release/g3mkcert --version'
```

This provides a statistically robust comparison of the performance before and after PGO.

## Known Limitations

### Current Workload Coverage

At present, only `g3mkcert` has a meaningful workload wired into the PGO example script. Other components are invoked only with trivial `--help/--version` commands, which do NOT produce representative execution profiles and therefore will not yield noticeable performance improvements yet. Future work can extend realistic workloads (or reuse coverage scripts after dependency isolation) for additional components.

### Memory Usage

- `g3proxy` component requires substantial memory during PGO build
- If memory is insufficient, avoid selecting `g3proxy` or use default components

### Disk Space

- Profile data may consume several hundred MB of space
- Ensure `/tmp/pgo-data` has sufficient space

## Troubleshooting

### Common Issues

**Issue**: `cargo profdata` not found

**Solution**: Install cargo-binutils

```bash
cargo install cargo-binutils
```

**Issue**: Out of memory error

**Solution**: Reduce concurrent builds or select fewer components

```bash
# Select only lightweight components
./scripts/pgo/example_run.sh --components g3mkcert,g3fcgen,g3bench
```

**Issue**: Profile data not generated

**Solution**: Check permissions and disk space

```bash
# Check /tmp permissions
ls -la /tmp/pgo-data/

# Clean and retry
rm -rf /tmp/pgo-data
```

**Issue**: Build terminated with SIGKILL (exit code 137)

**Solution**: This usually indicates memory exhaustion during build

```bash
# Use fewer components or avoid memory-intensive ones
./scripts/pgo/example_run.sh --components g3mkcert,g3fcgen,g3bench

# Or increase system memory/swap space
```

## Best Practices

### Component Selection Strategy

1. **Development Testing**: Use default components (`g3mkcert`)
2. **Lightweight Optimization**: Select a few key components (`g3mkcert,g3fcgen,g3bench`)
3. **Full Optimization**: Use `--all` on high-spec machines (note: may require substantial memory for `g3proxy`)

### Performance Testing Recommendations

1. Always compare before/after in the same environment
2. Run multiple tests and take averages
3. Focus on real-world usage scenario metrics
4. Use `hyperfine` for statistically robust comparisons

### Automation Integration

PGO can be integrated into CI/CD pipelines:

```bash
# Use PGO in release builds
./scripts/pgo/example_run.sh --components g3mkcert,g3fcgen
cp target/release/* ./release-artifacts/
```

## More Information

- For issues, please check project Issues or submit a new Issue
