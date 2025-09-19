#!/bin/bash

# PGO Build Script for Rust Code
# This script implements Profile-Guided Optimization for g3 Rust components

set -e

SCRIPTS_DIR=$(dirname "$0")
PROJECT_DIR=$(realpath "${SCRIPTS_DIR}/../..")
PGO_DIR="${SCRIPTS_DIR}"
PROFILE_DIR="${PGO_DIR}/profile"
BENCHMARK_DIR="${PGO_DIR}/benchmark"
BUILD_DIR="${PROJECT_DIR}/target"

# Default components for PGO (memory-efficient choices)
DEFAULT_COMPONENTS=("g3mkcert" "g3proxy-ctl")
# All available components
ALL_COMPONENTS=("g3mkcert" "g3proxy-ctl" "g3proxy" "g3bench" "g3fcgen" "g3iploc" "g3keymess" "g3keymess-ctl" "g3statsd" "g3statsd-ctl" "g3tiles" "g3tiles-ctl" "g3proxy-ftp")

# Components to build with PGO (set by command line args)
declare -a PGO_COMPONENTS

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking Rust PGO prerequisites..."
    
    # Check Rust and Cargo
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Cargo not found. Please install Rust."
        exit 1
    fi
    
    # Check if llvm-profdata is available
    local llvm_profdata=""
    if [ -f "$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata" ]; then
        llvm_profdata="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"
        log_info "Found llvm-profdata from rustup: $llvm_profdata"
    elif command -v llvm-profdata >/dev/null 2>&1; then
        llvm_profdata="llvm-profdata"
        log_info "Found system llvm-profdata: $llvm_profdata"
    else
        log_warn "llvm-profdata not found, will try cargo profdata as fallback"
    fi
    
    log_info "Prerequisites check passed"
}

# Clean previous builds and profile data
clean_previous() {
    log_info "Cleaning previous builds and profile data..."
    cargo clean
    rm -rf /tmp/pgo-data
    mkdir -p /tmp/pgo-data
}

# Build instrumented binaries for profile generation
build_instrumented() {
    log_info "Building instrumented binaries for profile generation..."
    log_info "Target components: ${PGO_COMPONENTS[*]}"
    
    cd "${PROJECT_DIR}"
    
    # Clean previous builds
    cargo clean
    
    # Build instrumented binaries using cargo-pgo
    local build_args=""
    for component in "${PGO_COMPONENTS[@]}"; do
        build_args="$build_args -p $component"
    done
    
    log_info "Building packages:$build_args"
    if ! cargo pgo build -- --release $build_args; then
        log_error "Failed to build instrumented binaries"
        return 1
    fi
    
    log_info "Instrumented binaries built successfully"
}

# Function to get the correct binary path (cargo-pgo uses different paths)
get_binary_path() {
    local binary_name="$1"
    local cargo_pgo_path="./target/x86_64-unknown-linux-gnu/release/${binary_name}"
    local standard_path="./target/release/${binary_name}"
    
    if [ -f "$cargo_pgo_path" ]; then
        echo "$cargo_pgo_path"
    elif [ -f "$standard_path" ]; then
        echo "$standard_path"
    else
        echo "$standard_path"  # fallback
    fi
}

# Run comprehensive g3mkcert workload based on coverage scripts
run_g3mkcert_comprehensive_workload() {
    local g3mkcert_bin="$1"
    local temp_dir="/tmp/pgo-g3mkcert-$$"
    local original_dir="$(pwd)"
    
    echo "Running comprehensive g3mkcert workload based on coverage scripts..."
    mkdir -p "$temp_dir"
    cd "$temp_dir"
    
    # Convert relative path to absolute path
    if [[ "$g3mkcert_bin" != /* ]]; then
        g3mkcert_bin="${original_dir}/${g3mkcert_bin}"
    fi
    
    # Basic operations first
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --version || echo "g3mkcert --version failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --help || echo "g3mkcert --help failed"
    
    # Root CA certificates with different algorithms and key sizes
    echo "Generating Root CA certificates..."
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --root --common-name "G3 Test CA" --rsa 2048 --output-cert rootCA-rsa.crt --output-key rootCA-rsa.key || echo "Root CA RSA generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --root --common-name "G3 Test CA" --ec256 --output-cert rootCA-ec256.crt --output-key rootCA-ec256.key || echo "Root CA EC256 generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --root --common-name "G3 Test CA" --ed25519 --output-cert rootCA-ed25519.crt --output-key rootCA-ed25519.key || echo "Root CA Ed25519 generation failed"
    
    # Intermediate CA certificates
    echo "Generating Intermediate CA certificates..."
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --intermediate --common-name "G3 Intermediate CA" --rsa 2048 --output-cert intermediateCA-rsa.crt --output-key intermediateCA-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key || echo "Intermediate CA RSA generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --intermediate --common-name "G3 Intermediate CA" --ec384 --output-cert intermediateCA-ec384.crt --output-key intermediateCA-ec384.key --ca-cert rootCA-ec256.crt --ca-key rootCA-ec256.key || echo "Intermediate CA EC384 generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --intermediate --common-name "G3 Intermediate CA" --ed25519 --output-cert intermediateCA-ed25519.crt --output-key intermediateCA-ed25519.key --ca-cert rootCA-ed25519.crt --ca-key rootCA-ed25519.key || echo "Intermediate CA Ed25519 generation failed"
    
    # TLS Server certificates with different algorithms and hosts
    echo "Generating TLS Server certificates..."
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --tls-server --host "www.example.com" --host "*.example.net" --rsa 2048 --output-cert tls-server-rsa.crt --output-key tls-server-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key || echo "TLS Server RSA generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --tls-server --host "www.example.com" --host "*.example.net" --ec256 --output-cert tls-server-ec256.crt --output-key tls-server-ec256.key --ca-cert intermediateCA-rsa.crt --ca-key intermediateCA-rsa.key || echo "TLS Server EC256 generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --tls-server --host "www.example.com" --host "*.example.net" --ed25519 --output-cert tls-server-ed25519.crt --output-key tls-server-ed25519.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key || echo "TLS Server Ed25519 generation failed"
    
    # TLS Client certificates
    echo "Generating TLS Client certificates..."
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --tls-client --host "www.example.com" --rsa 4096 --output-cert tls-client-rsa.crt --output-key tls-client-rsa.key --ca-cert intermediateCA-ec384.crt --ca-key intermediateCA-ec384.key || echo "TLS Client RSA generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --tls-client --host "www.example.com" --ec256 --output-cert tls-client-ec256.crt --output-key tls-client-ec256.key --ca-cert rootCA-ec256.crt --ca-key rootCA-ec256.key || echo "TLS Client EC256 generation failed"
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3mkcert_bin" --tls-client --host "www.example.com" --ed25519 --output-cert tls-client-ed25519.crt --output-key tls-client-ed25519.key --ca-cert intermediateCA-ed25519.crt --ca-key intermediateCA-ed25519.key || echo "TLS Client Ed25519 generation failed"
    
    cd "$original_dir"
    rm -rf "$temp_dir" 2>/dev/null || true
    
    echo "Comprehensive g3mkcert workload completed"
}

# Run profile generation workloads
generate_profiles() {
    local profile_data_dir="${PROJECT_DIR}/target/pgo-profiles"
    
    echo "Generating PGO profiles..."
    
    # Ensure profile data directory exists
    mkdir -p "${profile_data_dir}"
    
    # Set environment for profile generation
    export LLVM_PROFILE_FILE="${profile_data_dir}/cargo-pgo-%p-%m.profraw"
    export TEST_NAME="rust-pgo"
    
    echo "Environment variables:"
    echo "  LLVM_PROFILE_FILE=${LLVM_PROFILE_FILE}"
    echo "  TEST_NAME=${TEST_NAME}"
    echo "  RUSTFLAGS=${RUSTFLAGS}"
    
    # Run workloads based on available components
    for component in "${PGO_COMPONENTS[@]}"; do
        case $component in
            g3mkcert)
                local g3mkcert_bin=$(get_binary_path "g3mkcert")
                run_g3mkcert_comprehensive_workload "$g3mkcert_bin"
                ;;
            g3proxy-ctl)
                echo "Running g3proxy-ctl workload..."
                local g3proxy_ctl_bin=$(get_binary_path "g3proxy-ctl")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3proxy_ctl_bin" --help || echo "g3proxy-ctl help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3proxy_ctl_bin" version || echo "g3proxy-ctl version failed"
                ;;
            g3proxy)
                echo "Running g3proxy workload..."
                local g3proxy_bin=$(get_binary_path "g3proxy")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3proxy_bin" --help || echo "g3proxy help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3proxy_bin" --version || echo "g3proxy version failed"
                ;;
            g3keymess-ctl)
                echo "Running g3keymess-ctl workload..."
                local g3keymess_ctl_bin=$(get_binary_path "g3keymess-ctl")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3keymess_ctl_bin" --help || echo "g3keymess-ctl help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3keymess_ctl_bin" --version || echo "g3keymess-ctl version failed"
                ;;
            g3statsd-ctl)
                echo "Running g3statsd-ctl workload..."
                local g3statsd_ctl_bin=$(get_binary_path "g3statsd-ctl")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3statsd_ctl_bin" --help || echo "g3statsd-ctl help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3statsd_ctl_bin" --version || echo "g3statsd-ctl version failed"
                ;;
            g3tiles-ctl)
                echo "Running g3tiles-ctl workload..."
                local g3tiles_ctl_bin=$(get_binary_path "g3tiles-ctl")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3tiles_ctl_bin" --help || echo "g3tiles-ctl help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3tiles_ctl_bin" --version || echo "g3tiles-ctl version failed"
                ;;
            g3proxy-ftp)
                echo "Running g3proxy-ftp workload..."
                local g3proxy_ftp_bin=$(get_binary_path "g3proxy-ftp")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3proxy_ftp_bin" --help || echo "g3proxy-ftp help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3proxy_ftp_bin" --version || echo "g3proxy-ftp version failed"
                ;;
            g3bench)
                echo "Running g3bench workload..."
                local g3bench_bin=$(get_binary_path "g3bench")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3bench_bin" --help || echo "g3bench help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3bench_bin" --version || echo "g3bench version failed"
                ;;
            g3fcgen)
                echo "Running g3fcgen workload..."
                local g3fcgen_bin=$(get_binary_path "g3fcgen")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3fcgen_bin" --help || echo "g3fcgen help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3fcgen_bin" --version || echo "g3fcgen version failed"
                ;;
            g3iploc)
                echo "Running g3iploc workload..."
                local g3iploc_bin=$(get_binary_path "g3iploc")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3iploc_bin" --help || echo "g3iploc help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3iploc_bin" --version || echo "g3iploc version failed"
                ;;
            g3keymess)
                echo "Running g3keymess workload..."
                local g3keymess_bin=$(get_binary_path "g3keymess")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3keymess_bin" --help || echo "g3keymess help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3keymess_bin" --version || echo "g3keymess version failed"
                ;;
            g3statsd)
                echo "Running g3statsd workload..."
                local g3statsd_bin=$(get_binary_path "g3statsd")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3statsd_bin" --help || echo "g3statsd help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3statsd_bin" --version || echo "g3statsd version failed"
                ;;
            g3tiles)
                echo "Running g3tiles workload..."
                local g3tiles_bin=$(get_binary_path "g3tiles")
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3tiles_bin" --help || echo "g3tiles help failed"
                env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$g3tiles_bin" --version || echo "g3tiles version failed"
                ;;
            *)
                echo "Running workload for ${component}..."
                local component_bin=$(get_binary_path "${component}")
                if [ -f "$component_bin" ]; then
                    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$component_bin" --help 2>/dev/null || echo "${component} help failed"
                    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" "$component_bin" --version 2>/dev/null || echo "${component} version failed"
                fi
                ;;
        esac
    done
    
    echo "Running unit tests to generate profiles..."
    # Run tests to generate more profile data with explicit environment
    env LLVM_PROFILE_FILE="${LLVM_PROFILE_FILE}" cargo test --release --package g3-types || echo "g3-types tests failed, continuing..."
    
    # Check if any profile files were generated
    local profile_count=$(find "${profile_data_dir}" -name "*.profraw" 2>/dev/null | wc -l)
    echo "Generated ${profile_count} profile files in ${profile_data_dir}"
    
    if [ "${profile_count}" -eq 0 ]; then
        echo "Warning: No profile files generated"
        ls -la "${profile_data_dir}" || echo "Profile directory doesn't exist"
        echo "Checking if instrumented binaries have profile capabilities..."
        local g3mkcert_bin=$(get_binary_path "g3mkcert")
        if [ -f "$g3mkcert_bin" ]; then
            ldd "$g3mkcert_bin" | grep -i profile || echo "No profile libraries found"
        fi
        return 1
    fi
    
    echo "Profile generation completed successfully!"
    return 0
}

# Build optimized binary using profile data
build_optimized() {
    log_info "Building optimized binaries using profile data..."
    
    cd "${PROJECT_DIR}"
    
    # Build only the selected components using cargo-pgo optimize
    local build_args=""
    for component in "${PGO_COMPONENTS[@]}"; do
        build_args="${build_args} -p ${component}"
    done
    
    log_info "Building optimized packages:${build_args}"
    if ! cargo pgo optimize build -- --release${build_args}; then
        log_error "Failed to build optimized binaries"
        return 1
    fi
    
    log_info "Optimized binaries built successfully"
}

# Check if hyperfine is available for more precise benchmarking
check_benchmark_tool() {
    if command -v hyperfine >/dev/null 2>&1; then
        echo "hyperfine"
    else
        echo "time"
    fi
}

# Run benchmark to measure performance improvement
run_benchmark() {
    log_info "Running performance benchmark for ${#PGO_COMPONENTS[@]} components"
    
    local benchmark_tool=$(check_benchmark_tool)
    if [ "$benchmark_tool" = "time" ]; then
        log_warn "Using 'time' for basic benchmarking. For more precise results, install 'hyperfine': cargo install hyperfine"
    else
        log_info "Using 'hyperfine' for precise benchmarking"
    fi
    
    for component in "${PGO_COMPONENTS[@]}"; do
        case "$component" in
            "g3mkcert")
                log_info "g3mkcert benchmark: Certificate operations"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3mkcert --version benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 15 "${BUILD_DIR}/release/g3mkcert --version"
                else
                    echo "Running g3mkcert operations..."
                    time "${BUILD_DIR}/release/g3mkcert" --version >/dev/null 2>&1
                fi
                ;;
            "g3proxy-ctl")
                log_info "g3proxy-ctl benchmark: Control commands execution speed"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3proxy-ctl --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3proxy-ctl --help"
                else
                    echo "Running g3proxy-ctl --help multiple times..."
                    time (for i in {1..100}; do "${BUILD_DIR}/release/g3proxy-ctl" --help >/dev/null 2>&1; done)
                fi
                ;;
            "g3proxy")
                log_info "g3proxy benchmark: Main proxy service startup"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3proxy --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3proxy --help"
                else
                    echo "Running g3proxy help operations..."
                    time "${BUILD_DIR}/release/g3proxy" --help >/dev/null 2>&1
                fi
                ;;
            "g3bench")
                log_info "g3bench benchmark: Performance testing tool"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3bench --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3bench --help"
                else
                    echo "Running g3bench help operations..."
                    time "${BUILD_DIR}/release/g3bench" --help >/dev/null 2>&1
                fi
                ;;
            "g3fcgen")
                log_info "g3fcgen benchmark: Flow control generator"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3fcgen --version benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 15 "${BUILD_DIR}/release/g3fcgen --version"
                else
                    echo "Running g3fcgen operations..."
                    time "${BUILD_DIR}/release/g3fcgen" --version >/dev/null 2>&1
                fi
                ;;
            "g3iploc")
                log_info "g3iploc benchmark: IP location service"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3iploc --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3iploc --help"
                else
                    echo "Running g3iploc help operations..."
                    time "${BUILD_DIR}/release/g3iploc" --help >/dev/null 2>&1
                fi
                ;;
            "g3keymess")
                log_info "g3keymess benchmark: Key management service"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3keymess --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3keymess --help"
                else
                    echo "Running g3keymess help operations..."
                    time "${BUILD_DIR}/release/g3keymess" --help >/dev/null 2>&1
                fi
                ;;
            "g3keymess-ctl")
                log_info "g3keymess-ctl benchmark: Key management control"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3keymess-ctl --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3keymess-ctl --help"
                else
                    echo "Running g3keymess-ctl help operations..."
                    time "${BUILD_DIR}/release/g3keymess-ctl" --help >/dev/null 2>&1
                fi
                ;;
            "g3statsd")
                log_info "g3statsd benchmark: Statistics data collector"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3statsd --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3statsd --help"
                else
                    echo "Running g3statsd help operations..."
                    time "${BUILD_DIR}/release/g3statsd" --help >/dev/null 2>&1
                fi
                ;;
            "g3statsd-ctl")
                log_info "g3statsd-ctl benchmark: Statistics control"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3statsd-ctl --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3statsd-ctl --help"
                else
                    echo "Running g3statsd-ctl help operations..."
                    time "${BUILD_DIR}/release/g3statsd-ctl" --help >/dev/null 2>&1
                fi
                ;;
            "g3tiles")
                log_info "g3tiles benchmark: Tile service"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3tiles --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3tiles --help"
                else
                    echo "Running g3tiles help operations..."
                    time "${BUILD_DIR}/release/g3tiles" --help >/dev/null 2>&1
                fi
                ;;
            "g3tiles-ctl")
                log_info "g3tiles-ctl benchmark: Tile control"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3tiles-ctl --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3tiles-ctl --help"
                else
                    echo "Running g3tiles-ctl help operations..."
                    time "${BUILD_DIR}/release/g3tiles-ctl" --help >/dev/null 2>&1
                fi
                ;;
            "g3proxy-ftp")
                log_info "g3proxy-ftp benchmark: FTP proxy tool"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running g3proxy-ftp --help benchmark with hyperfine..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/g3proxy-ftp --help"
                else
                    echo "Running g3proxy-ftp help operations..."
                    time "${BUILD_DIR}/release/g3proxy-ftp" --help >/dev/null 2>&1
                fi
                ;;
            *)
                log_info "Generic benchmark for component: $component"
                if [ -f "${BUILD_DIR}/release/${component}" ]; then
                    if [ "$benchmark_tool" = "hyperfine" ]; then
                        echo "Running ${component} --help benchmark with hyperfine..."
                        hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/${component} --help" 2>/dev/null || echo "${component} hyperfine benchmark completed"
                    else
                        echo "Running ${component} help operations..."
                        time "${BUILD_DIR}/release/${component}" --help >/dev/null 2>&1 || echo "${component} benchmark completed"
                    fi
                else
                    log_warn "Binary not found for component: $component"
                fi
                ;;
        esac
    done
}

run_performance_benchmark() {
    log_info "Running performance benchmark to measure PGO effectiveness"
    
    local benchmark_tool=$(check_benchmark_tool)
    if [ "$benchmark_tool" = "time" ]; then
        log_warn "Using 'time' for basic benchmarking. For more precise results, install 'hyperfine': cargo install hyperfine"
    else
        log_info "Using 'hyperfine' for precise benchmarking"
    fi
    
    # First test with regular build for comparison
    log_info "Building baseline (non-PGO) version for comparison..."
    (cd "${PROJECT_DIR}" && cargo build --release >/dev/null 2>&1)
    
    echo ""
    log_info "=== Baseline Performance (without PGO) ==="
    for component in "${PGO_COMPONENTS[@]}"; do
        case "$component" in
            "g3mkcert")
                echo "Testing g3mkcert operations..."
                local g3mkcert_bin=$(get_binary_path "g3mkcert")
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    # Save baseline for hyperfine comparison
                    cp "$g3mkcert_bin" "/tmp/g3mkcert-baseline"
                    echo "Baseline saved for comparison"
                else
                    time "$g3mkcert_bin" --version >/dev/null 2>&1 || echo "Baseline test completed"
                fi
                ;;
            "g3proxy-ctl"|"g3keymess-ctl"|"g3statsd-ctl"|"g3tiles-ctl")
                echo "Testing ${component} help display..."
                local component_bin=$(get_binary_path "${component}")
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running ${component} --help benchmark..."
                    hyperfine --shell=none --warmup 3 --runs 10 "$component_bin --help"
                else
                    time (for i in {1..50}; do "$component_bin" --help >/dev/null 2>&1; done)
                fi
                ;;
            "g3proxy"|"g3bench"|"g3fcgen"|"g3iploc"|"g3keymess"|"g3statsd"|"g3tiles"|"g3proxy-ftp")
                echo "Testing ${component} basic operations..."
                local component_bin=$(get_binary_path "${component}")
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running ${component} --help benchmark..."
                    hyperfine --shell=none --warmup 3 --runs 10 "$component_bin --help" 2>/dev/null || echo "${component} baseline benchmark completed"
                else
                    time (for i in {1..20}; do "$component_bin" --help >/dev/null 2>&1; done) 2>/dev/null || echo "${component} baseline test completed"
                fi
                ;;
            *)
                echo "Testing ${component} basic operations..."
                local component_bin=$(get_binary_path "${component}")
                if [ "$benchmark_tool" = "time" ]; then
                    time (for i in {1..20}; do "$component_bin" --help >/dev/null 2>&1; done) 2>/dev/null || echo "${component} baseline test completed"
                fi
                ;;
        esac
    done
    
    echo ""
    log_info "=== PGO-Optimized Performance ==="
    for component in "${PGO_COMPONENTS[@]}"; do
        case "$component" in
            "g3mkcert")
                echo "Testing PGO-optimized g3mkcert operations..."
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Comparing baseline vs PGO-optimized g3mkcert..."
                    hyperfine --shell=none --warmup 3 --runs 15 "/tmp/g3mkcert-baseline --version" "${BUILD_DIR}/release/g3mkcert --version"
                else
                    time "${BUILD_DIR}/release/g3mkcert" --version >/dev/null 2>&1 || echo "PGO test completed"
                fi
                ;;
            "g3proxy-ctl"|"g3keymess-ctl"|"g3statsd-ctl"|"g3tiles-ctl")
                echo "Testing PGO-optimized ${component} help display..."
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running PGO-optimized ${component} --help benchmark..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/${component} --help"
                else
                    time (for i in {1..50}; do "${BUILD_DIR}/release/${component}" --help >/dev/null 2>&1; done)
                fi
                ;;
            "g3proxy"|"g3bench"|"g3fcgen"|"g3iploc"|"g3keymess"|"g3statsd"|"g3tiles"|"g3proxy-ftp")
                echo "Testing PGO-optimized ${component} basic operations..."
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Running PGO-optimized ${component} --help benchmark..."
                    hyperfine --shell=none --warmup 3 --runs 10 "${BUILD_DIR}/release/${component} --help" 2>/dev/null || echo "${component} PGO benchmark completed"
                else
                    time (for i in {1..20}; do "${BUILD_DIR}/release/${component}" --help >/dev/null 2>&1; done) 2>/dev/null || echo "${component} PGO test completed"
                fi
                ;;
            *)
                echo "Testing PGO-optimized ${component} basic operations..."
                if [ "$benchmark_tool" = "time" ]; then
                    time (for i in {1..20}; do "${BUILD_DIR}/release/${component}" --help >/dev/null 2>&1; done) 2>/dev/null || echo "${component} PGO test completed"
                fi
                ;;
        esac
    done
    
    echo ""
    log_info "Performance benchmark completed!"
    if [ "$benchmark_tool" = "time" ]; then
        log_info "Compare the 'real' times above to see PGO optimization effect"
        log_info "For more precise measurements, install hyperfine: cargo install hyperfine"
    else
        log_info "Hyperfine results show statistical comparison with confidence intervals"
    fi
}

# Parse component arguments
parse_components() {
    if [ ${#PGO_COMPONENTS[@]} -eq 0 ]; then
        # No components specified, use defaults
        PGO_COMPONENTS=("${DEFAULT_COMPONENTS[@]}")
        log_info "No components specified, using defaults: ${PGO_COMPONENTS[*]}"
    else
        # Validate specified components
        for component in "${PGO_COMPONENTS[@]}"; do
            if [[ ! " ${ALL_COMPONENTS[*]} " =~ " ${component} " ]]; then
                log_error "Invalid component: $component"
                log_info "Available components: ${ALL_COMPONENTS[*]}"
                exit 1
            fi
        done
        log_info "Using specified components: ${PGO_COMPONENTS[*]}"
    fi
}

# Main function
main() {
    log_info "Starting Rust PGO optimization process..."
    log_info "Project directory: ${PROJECT_DIR}"
    log_info "PGO scripts directory: ${PGO_DIR}"
    
    # Parse and validate components
    parse_components
    
    # Check prerequisites
    check_prerequisites
    
    # Step 1: Clean previous builds
    clean_previous
    
    # Step 2: Build instrumented binary
    build_instrumented
    
    # Step 3: Generate profiles
    generate_profiles
    
    # Step 4: Build optimized binary
    build_optimized
    
    # Step 5: Run benchmark (optional)
    if [ "${RUN_BENCHMARK}" = "true" ]; then
        run_performance_benchmark
    fi
    
    log_info "PGO optimization completed successfully!"
    log_info "Components optimized: ${PGO_COMPONENTS[*]}"
    log_info "Optimized binaries are available in: ${BUILD_DIR}/release/"
    
    # Show profile data location
    if [ -d "/tmp/pgo-data" ]; then
        profile_count=$(find /tmp/pgo-data -name "*.profraw" -o -name "*.profdata" | wc -l)
        log_info "Profile data files generated: ${profile_count}"
        log_info "Profile data location: /tmp/pgo-data"
    fi
    
    if [ "${RUN_BENCHMARK}" != "true" ]; then
        echo ""
        log_info "To verify optimization effectiveness, run:"
        echo "  $0 --benchmark"
        echo "or manually test optimized binaries in: ${BUILD_DIR}/release/"
    fi
}

# Parse command line arguments
RUN_BENCHMARK=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --components|-c)
            shift
            # Parse comma-separated component list
            IFS=',' read -ra PGO_COMPONENTS <<< "$1"
            shift
            ;;
        --all|-a)
            PGO_COMPONENTS=("${ALL_COMPONENTS[@]}")
            shift
            ;;
        --benchmark|-b)
            RUN_BENCHMARK=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -c, --components LIST  Comma-separated list of components to optimize"
            echo "                         Default: ${DEFAULT_COMPONENTS[*]}"
            echo "  -a, --all             Optimize all available components"
            echo "  -b, --benchmark       Run performance benchmark after optimization"
            echo "  -h, --help            Show this help message"
            echo ""
            echo "Available components:"
            echo "  ${ALL_COMPONENTS[*]}"
            echo ""
            echo "Examples:"
            echo "  $0                                    # Use default components"
            echo "  $0 --components g3mkcert,g3proxy     # Optimize specific components"
            echo "  $0 --all --benchmark                 # Optimize all and run benchmark"
            echo ""
            echo "This script performs Profile-Guided Optimization (PGO) for g3 Rust components:"
            echo "  1. Builds instrumented binaries"
            echo "  2. Runs representative workloads to collect profile data"
            echo "  3. Builds optimized binaries using the profile data"
            echo "  4. Optionally runs benchmarks to measure performance improvement"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

main