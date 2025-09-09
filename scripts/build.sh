#!/bin/bash

# Build script for Arcus-G3 SWG
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
BUILD_TYPE="release"
TARGET=""
FEATURES=""
DOCKER_BUILD=false
PUSH_IMAGE=false
REGISTRY="ghcr.io"
IMAGE_NAME="arcus-g3/swg/arcus-g3"
TAG="latest"

# Help function
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Build script for Arcus-G3 SWG

OPTIONS:
    -t, --target TARGET        Build target (e.g., x86_64-unknown-linux-gnu)
    -f, --features FEATURES    Comma-separated list of features to enable
    -d, --docker              Build Docker image
    -p, --push                Push Docker image to registry
    -r, --registry REGISTRY   Docker registry (default: ghcr.io)
    -i, --image IMAGE_NAME    Docker image name (default: arcus-g3/swg/arcus-g3)
    --tag TAG                 Docker image tag (default: latest)
    --debug                   Build in debug mode
    -h, --help                Show this help message

EXAMPLES:
    $0                                    # Build release binary
    $0 --debug                           # Build debug binary
    $0 --docker                          # Build Docker image
    $0 --docker --push --tag v1.0.0     # Build and push Docker image with tag
    $0 --target x86_64-unknown-linux-gnu # Cross-compile for Linux

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--target)
            TARGET="$2"
            shift 2
            ;;
        -f|--features)
            FEATURES="$2"
            shift 2
            ;;
        -d|--docker)
            DOCKER_BUILD=true
            shift
            ;;
        -p|--push)
            PUSH_IMAGE=true
            shift
            ;;
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        -i|--image)
            IMAGE_NAME="$2"
            shift 2
            ;;
        --tag)
            TAG="$2"
            shift 2
            ;;
        --debug)
            BUILD_TYPE="debug"
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [[ ! -f "Cargo.toml" ]]; then
    log_error "Not in a Rust project directory. Please run this script from the project root."
    exit 1
fi

# Build Rust binary
build_rust() {
    log_info "Building Arcus-G3 SWG..."
    
    local cargo_args=()
    
    if [[ "$BUILD_TYPE" == "release" ]]; then
        cargo_args+=(--release)
    fi
    
    if [[ -n "$TARGET" ]]; then
        cargo_args+=(--target "$TARGET")
    fi
    
    if [[ -n "$FEATURES" ]]; then
        cargo_args+=(--features "$FEATURES")
    fi
    
    cargo_args+=(--bin arcus-g3)
    
    log_info "Running: cargo build ${cargo_args[*]}"
    
    if cargo build "${cargo_args[@]}"; then
        log_success "Rust build completed successfully"
    else
        log_error "Rust build failed"
        exit 1
    fi
}

# Build Docker image
build_docker() {
    log_info "Building Docker image..."
    
    local image_tag="${REGISTRY}/${IMAGE_NAME}:${TAG}"
    
    log_info "Image: $image_tag"
    
    if docker build -t "$image_tag" .; then
        log_success "Docker image built successfully: $image_tag"
        
        if [[ "$PUSH_IMAGE" == true ]]; then
            log_info "Pushing image to registry..."
            if docker push "$image_tag"; then
                log_success "Image pushed successfully: $image_tag"
            else
                log_error "Failed to push image"
                exit 1
            fi
        fi
    else
        log_error "Docker build failed"
        exit 1
    fi
}

# Main execution
main() {
    log_info "Starting Arcus-G3 SWG build process..."
    log_info "Build type: $BUILD_TYPE"
    
    if [[ -n "$TARGET" ]]; then
        log_info "Target: $TARGET"
    fi
    
    if [[ -n "$FEATURES" ]]; then
        log_info "Features: $FEATURES"
    fi
    
    # Build Rust binary
    build_rust
    
    # Build Docker image if requested
    if [[ "$DOCKER_BUILD" == true ]]; then
        build_docker
    fi
    
    log_success "Build process completed successfully!"
}

# Run main function
main "$@"
