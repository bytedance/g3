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
PGO_DATA_DIR="/tmp/pgo-data"

# Default components for PGO (memory-efficient choices)
DEFAULT_COMPONENTS=("g3mkcert")
# All available components
ALL_COMPONENTS=("g3mkcert" "g3proxy" "g3bench" "g3fcgen" "g3iploc" "g3keymess" "g3statsd" "g3tiles")

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

    # Check cargo-binutils for profdata
    if ! cargo profdata --help >/dev/null 2>&1; then
        log_warn "cargo profdata not found. Please install cargo-binutils: cargo install cargo-binutils && cargo binutils setup"
    fi
    
    log_info "Prerequisites check passed"
}

# Clean previous builds and profile data
TEMP_DIRS=()
cleanup_temp_dirs() {
    log_info "Cleaning up temporary directories..."
    for dir in "${TEMP_DIRS[@]}"; do
        rm -rf "$dir" 2>/dev/null || true
    done
    rm -rf "${PGO_DATA_DIR}"
}

clean_previous() {
    log_info "Cleaning previous builds and profile data..."
    cargo clean
    cleanup_temp_dirs
    mkdir -p "${PGO_DATA_DIR}"
}

# Build instrumented binaries for profile generation
build_instrumented() {
    log_info "Building instrumented binaries for profile generation..."
    log_info "Target components: ${PGO_COMPONENTS[*]}"
    
    cd "${PROJECT_DIR}"
    for component in "${PGO_COMPONENTS[@]}"; do
        log_info "Building instrumented binary for $component ..."
        RUSTFLAGS="-Cprofile-generate=${PGO_DATA_DIR}" cargo build --release -p "$component"
    done
    log_info "Instrumented binaries built successfully"
}

get_binary_path() {
    local binary_name="$1"
    echo "./target/release/${binary_name}"
}

# Run comprehensive g3mkcert workload based on coverage scripts
run_g3mkcert_comprehensive_workload() {
    local g3mkcert_bin="$1"
    local temp_dir="/tmp/pgo-g3mkcert-$$"
    local original_dir="$(pwd)"
    TEMP_DIRS+=("$temp_dir")
    echo "Running comprehensive g3mkcert workload based on coverage scripts..."
    mkdir -p "$temp_dir"
    cd "$temp_dir"
    
    # Convert relative path to absolute path
    if [[ "$g3mkcert_bin" != /* ]]; then
        g3mkcert_bin="${original_dir}/${g3mkcert_bin}"
    fi
    
    # Basic operations first
    "$g3mkcert_bin" --version || echo "g3mkcert --version failed"
    "$g3mkcert_bin" --help || echo "g3mkcert --help failed"
    
    # Root CA certificates with different algorithms and key sizes
    echo "Generating Root CA certificates..."
    "$g3mkcert_bin" --root --common-name "G3 Test CA" --rsa 2048 --output-cert rootCA-rsa.crt --output-key rootCA-rsa.key || echo "Root CA RSA generation failed"
    "$g3mkcert_bin" --root --common-name "G3 Test CA" --ec256 --output-cert rootCA-ec256.crt --output-key rootCA-ec256.key || echo "Root CA EC256 generation failed"
    "$g3mkcert_bin" --root --common-name "G3 Test CA" --ed25519 --output-cert rootCA-ed25519.crt --output-key rootCA-ed25519.key || echo "Root CA Ed25519 generation failed"
    
    # Intermediate CA certificates
    echo "Generating Intermediate CA certificates..."
    "$g3mkcert_bin" --intermediate --common-name "G3 Intermediate CA" --rsa 2048 --output-cert intermediateCA-rsa.crt --output-key intermediateCA-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key || echo "Intermediate CA RSA generation failed"
    "$g3mkcert_bin" --intermediate --common-name "G3 Intermediate CA" --ec384 --output-cert intermediateCA-ec384.crt --output-key intermediateCA-ec384.key --ca-cert rootCA-ec256.crt --ca-key rootCA-ec256.key || echo "Intermediate CA EC384 generation failed"
    "$g3mkcert_bin" --intermediate --common-name "G3 Intermediate CA" --ed25519 --output-cert intermediateCA-ed25519.crt --output-key intermediateCA-ed25519.key --ca-cert rootCA-ed25519.crt --ca-key rootCA-ed25519.key || echo "Intermediate CA Ed25519 generation failed"
    
    # TLS Server certificates with different algorithms and hosts
    echo "Generating TLS Server certificates..."
    "$g3mkcert_bin" --tls-server --host "www.example.com" --host "*.example.net" --rsa 2048 --output-cert tls-server-rsa.crt --output-key tls-server-rsa.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key || echo "TLS Server RSA generation failed"
    "$g3mkcert_bin" --tls-server --host "www.example.com" --host "*.example.net" --ec256 --output-cert tls-server-ec256.crt --output-key tls-server-ec256.key --ca-cert intermediateCA-rsa.crt --ca-key intermediateCA-rsa.key || echo "TLS Server EC256 generation failed"
    "$g3mkcert_bin" --tls-server --host "www.example.com" --host "*.example.net" --ed25519 --output-cert tls-server-ed25519.crt --output-key tls-server-ed25519.key --ca-cert rootCA-rsa.crt --ca-key rootCA-rsa.key || echo "TLS Server Ed25519 generation failed"
    
    # TLS Client certificates
    echo "Generating TLS Client certificates..."
    "$g3mkcert_bin" --tls-client --host "www.example.com" --rsa 4096 --output-cert tls-client-rsa.crt --output-key tls-client-rsa.key --ca-cert intermediateCA-ec384.crt --ca-key intermediateCA-ec384.key || echo "TLS Client RSA generation failed"
    "$g3mkcert_bin" --tls-client --host "www.example.com" --ec256 --output-cert tls-client-ec256.crt --output-key tls-client-ec256.key --ca-cert rootCA-ec256.crt --ca-key rootCA-ec256.key || echo "TLS Client EC256 generation failed"
    "$g3mkcert_bin" --tls-client --host "www.example.com" --ed25519 --output-cert tls-client-ed25519.crt --output-key tls-client-ed25519.key --ca-cert intermediateCA-ed25519.crt --ca-key intermediateCA-ed25519.key || echo "TLS Client Ed25519 generation failed"
    
    cd "$original_dir"
    echo "Comprehensive g3mkcert workload completed"
}

# Run profile generation workloads
generate_profiles() {
    # Use unified directory for both compiler generated and runtime (LLVM_PROFILE_FILE) output
    local profile_data_dir="${PGO_DATA_DIR}"
    echo "Generating PGO profiles..."
    mkdir -p "${PGO_DATA_DIR}"
    # Pattern ensures per-process/profile separation
    export LLVM_PROFILE_FILE="${PGO_DATA_DIR}/cargo-pgo-%p-%m.profraw"
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
            g3proxy)
                echo "Running g3proxy workload..."
                local g3proxy_bin=$(get_binary_path "g3proxy")
                "$g3proxy_bin" --help || echo "g3proxy help failed"
                "$g3proxy_bin" --version || echo "g3proxy version failed"
                ;;
            g3bench)
                echo "Running g3bench workload..."
                local g3bench_bin=$(get_binary_path "g3bench")
                "$g3bench_bin" help || echo "g3bench help command failed"
                "$g3bench_bin" version || echo "g3bench version command failed"
                ;;
            g3fcgen)
                echo "Running g3fcgen workload..."
                local g3fcgen_bin=$(get_binary_path "g3fcgen")
                "$g3fcgen_bin" --help || echo "g3fcgen help failed"
                "$g3fcgen_bin" --version || echo "g3fcgen version failed"
                ;;
            g3iploc)
                echo "Running g3iploc workload..."
                local g3iploc_bin=$(get_binary_path "g3iploc")
                "$g3iploc_bin" --help || echo "g3iploc help failed"
                "$g3iploc_bin" --version || echo "g3iploc version failed"
                ;;
            g3keymess)
                echo "Running g3keymess workload..."
                local g3keymess_bin=$(get_binary_path "g3keymess")
                "$g3keymess_bin" --help || echo "g3keymess help failed"
                "$g3keymess_bin" --version || echo "g3keymess version failed"
                ;;
            g3statsd)
                echo "Running g3statsd workload..."
                local g3statsd_bin=$(get_binary_path "g3statsd")
                "$g3statsd_bin" --help || echo "g3statsd help failed"
                "$g3statsd_bin" --version || echo "g3statsd version failed"
                ;;
            g3tiles)
                echo "Running g3tiles workload..."
                local g3tiles_bin=$(get_binary_path "g3tiles")
                "$g3tiles_bin" --help || echo "g3tiles help failed"
                "$g3tiles_bin" --version || echo "g3tiles version failed"
                ;;
            *)
                echo "Running workload for ${component}..."
                local component_bin=$(get_binary_path "${component}")
                if [ -f "$component_bin" ]; then
                    "$component_bin" --help 2>/dev/null || echo "${component} help failed"
                    "$component_bin" --version 2>/dev/null || echo "${component} version failed"
                fi
                ;;
        esac
    done
    
    echo "Running unit tests to generate profiles..."
    # Run tests to generate more profile data
    cargo test --release --package g3-types || echo "g3-types tests failed, continuing..."
    
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
    log_info "Merging profile data..."
    shopt -s nullglob
    local profraw_files=("${PGO_DATA_DIR}"/*.profraw)
    if [ ${#profraw_files[@]} -eq 0 ]; then
        log_error "No .profraw files found in ${PGO_DATA_DIR}; skip optimized build."
        shopt -u nullglob
        return 1
    fi
    local count=${#profraw_files[@]}
    log_info "Found ${count} raw profile files"
    cargo profdata -- merge -o "${PGO_DATA_DIR}/merged.profdata" "${profraw_files[@]}"
    shopt -u nullglob
    log_info "Building optimized binaries using profile data..."
    cd "${PROJECT_DIR}"
    for component in "${PGO_COMPONENTS[@]}"; do
        log_info "Building optimized binary for $component ..."
        RUSTFLAGS="-Cprofile-use=${PGO_DATA_DIR}/merged.profdata" cargo build --release -p "$component"
    done
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

run_performance_benchmark() {
    log_info "Running performance benchmark to measure PGO effectiveness"
    
    local benchmark_tool=$(check_benchmark_tool)
    if [ "$benchmark_tool" = "time" ]; then
        log_warn "Using 'time' for basic benchmarking. For more precise results, install 'hyperfine': cargo install hyperfine"
    else
        log_info "Using 'hyperfine' for precise benchmarking"
    fi

    # Preserve current (PGO optimized) binaries before building baseline which would overwrite them
    log_info "Preserving PGO optimized binaries..."
    for component in "${PGO_COMPONENTS[@]}"; do
        local pgo_bin="${BUILD_DIR}/release/${component}"
        if [ -x "$pgo_bin" ]; then
            cp "$pgo_bin" "/tmp/${component}-pgo" || log_warn "Failed to preserve PGO binary for ${component}"
        else
            log_warn "Expected optimized binary not found for ${component}: $pgo_bin"
        fi
    done
    log_info "PGO binaries saved to /tmp/<component>-pgo"
    
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
                local cert_out="/tmp/rootCA-bench-baseline.crt"
                local key_out="/tmp/rootCA-bench-baseline.key"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    # Save baseline for hyperfine comparison
                    cp "$g3mkcert_bin" "/tmp/g3mkcert-baseline"
                    echo "Baseline saved for comparison"
                else
                    time "$g3mkcert_bin" --root --common-name "G3 Test CA" --rsa 2048 --output-cert "$cert_out" --output-key "$key_out" >/dev/null 2>&1 || echo "Baseline test completed"
                fi
                ;;
            "g3proxy"|"g3bench"|"g3fcgen"|"g3iploc"|"g3keymess"|"g3statsd"|"g3tiles")
                echo "Testing ${component} basic operations..."
                local component_bin=$(get_binary_path "${component}")
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    # Save baseline for hyperfine comparison
                    cp "$component_bin" "/tmp/${component}-baseline"
                    echo "Baseline saved for comparison"
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
                local cert_out="/tmp/rootCA-bench-pgo.crt"
                local key_out="/tmp/rootCA-bench-pgo.key"
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Comparing baseline vs PGO-optimized g3mkcert (root CA generation)..."
                    hyperfine --shell=none --warmup 3 --runs 15 "/tmp/g3mkcert-baseline --root --common-name 'G3 Test CA' --rsa 2048 --output-cert /tmp/rootCA-bench-baseline.crt --output-key /tmp/rootCA-bench-baseline.key" "/tmp/g3mkcert-pgo --root --common-name 'G3 Test CA' --rsa 2048 --output-cert /tmp/rootCA-bench-pgo.crt --output-key /tmp/rootCA-bench-pgo.key"
                else
                    time "/tmp/g3mkcert-pgo" --root --common-name "G3 Test CA" --rsa 2048 --output-cert "$cert_out" --output-key "$key_out" >/dev/null 2>&1 || echo "PGO test completed"
                fi
                rm -f /tmp/rootCA-bench-baseline.crt /tmp/rootCA-bench-baseline.key /tmp/rootCA-bench-pgo.crt /tmp/rootCA-bench-pgo.key
                ;;
            "g3proxy"|"g3bench"|"g3fcgen"|"g3iploc"|"g3keymess"|"g3statsd"|"g3tiles")
                echo "Testing PGO-optimized ${component} basic operations..."
                if [ "$benchmark_tool" = "hyperfine" ]; then
                    echo "Comparing baseline vs PGO-optimized ${component} (help output)..."
                    hyperfine --shell=none --warmup 3 --runs 15 "/tmp/${component}-baseline --help" "/tmp/${component}-pgo --help"
                else
                    time (for i in {1..20}; do "/tmp/${component}-pgo" --help >/dev/null 2>&1; done) 2>/dev/null || echo "${component} PGO test completed"
                fi
                ;;
            *)
                echo "Testing PGO-optimized ${component} basic operations..."
                if [ "$benchmark_tool" = "time" ]; then
                    time (for i in {1..20}; do "/tmp/${component}-pgo" --help >/dev/null 2>&1; done) 2>/dev/null || echo "${component} PGO test completed"
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

    for component in "${PGO_COMPONENTS[@]}"; do
        if [ -f "/tmp/${component}-pgo" ]; then
            cp "/tmp/${component}-pgo" "${BUILD_DIR}/release/${component}" 2>/dev/null || true
        fi
    done
    log_info "Restored PGO binaries to ${BUILD_DIR}/release/ after benchmarking"
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
    if [ -d "${PGO_DATA_DIR}" ]; then
        profile_count=$(find "${PGO_DATA_DIR}" -name "*.profraw" -o -name "*.profdata" | wc -l)
        log_info "Profile data files generated: ${profile_count}"
        log_info "Profile data location: ${PGO_DATA_DIR}"
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
            echo "  -a, --all              Optimize all available components"
            echo "  -b, --benchmark        Run performance benchmark after optimization"
            echo "  -h, --help             Show this help message"
            echo ""
            echo "Available components:"
            echo "  ${ALL_COMPONENTS[*]}"
            echo ""
            echo "Examples:"
            echo "  $0                                   # Use default components"
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

trap 'cleanup_temp_dirs; exit 1' INT TERM
main