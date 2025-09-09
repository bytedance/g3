# Build and Deployment Guide

This document provides comprehensive instructions for building, testing, and deploying the Arcus-G3 Multi-Tenant Secure Web Gateway.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Local Development](#local-development)
- [Docker Build](#docker-build)
- [Kubernetes Deployment](#kubernetes-deployment)
- [CI/CD Pipeline](#cicd-pipeline)
- [Monitoring Setup](#monitoring-setup)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### Required Software

- **Rust**: 1.89+ (stable, beta, or nightly)
- **Docker**: 20.10+ with BuildKit support
- **kubectl**: 1.24+ (for Kubernetes deployment)
- **Helm**: 3.8+ (for Helm chart deployment)

### Optional Software

- **Docker Compose**: 2.0+ (for local development)
- **Kustomize**: 4.0+ (for Kubernetes customization)
- **Trivy**: 0.40+ (for security scanning)

## Local Development

### 1. Clone and Setup

```bash
git clone https://github.com/arcus-g3/swg.git
cd swg
```

### 2. Install Dependencies

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Install development tools
cargo install cargo-watch cargo-edit cargo-audit cargo-outdated cargo-tree
```

### 3. Build and Test

```bash
# Build the project
cargo build --all

# Run tests
cargo test --all

# Run with formatting and linting
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Security audit
cargo audit
```

### 4. Run Locally

```bash
# Run the CLI
cargo run --bin arcus-g3 -- --help

# Run with configuration
cargo run --bin arcus-g3 -- start --config config.yaml
```

## Docker Build

### 1. Build Docker Image

```bash
# Build using the build script
./scripts/build.sh --docker

# Or build manually
docker build -t arcus-g3-swg:latest .
```

### 2. Run with Docker Compose

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f arcus-g3

# Stop services
docker-compose down
```

### 3. Multi-Platform Build

```bash
# Build for multiple platforms
docker buildx build --platform linux/amd64,linux/arm64 -t arcus-g3-swg:latest .
```

## Kubernetes Deployment

### 1. Using Kustomize

```bash
# Deploy to development
kubectl apply -k k8s/overlays/dev

# Deploy to production
kubectl apply -k k8s/overlays/prod
```

### 2. Using Helm

```bash
# Add Helm repository (if using a chart repository)
helm repo add arcus-g3 https://charts.arcus-g3.local
helm repo update

# Install/upgrade
helm upgrade --install arcus-g3-swg ./helm/arcus-g3-swg \
  --namespace arcus-g3 \
  --create-namespace \
  --values helm/arcus-g3-swg/values.yaml

# Custom values
helm upgrade --install arcus-g3-swg ./helm/arcus-g3-swg \
  --namespace arcus-g3 \
  --set replicaCount=5 \
  --set image.tag=v1.0.0
```

### 3. Verify Deployment

```bash
# Check pods
kubectl get pods -l app=arcus-g3-swg

# Check services
kubectl get services -l app=arcus-g3-swg

# Check logs
kubectl logs -l app=arcus-g3-swg -f
```

## CI/CD Pipeline

### GitHub Actions

The project includes comprehensive GitHub Actions workflows:

- **CI Pipeline** (`.github/workflows/ci.yml`):
  - Lint and format checking
  - Security auditing
  - Multi-platform testing
  - Code coverage
  - Docker image building

- **Docker Pipeline** (`.github/workflows/docker.yml`):
  - Multi-platform Docker builds
  - Container registry publishing
  - Security scanning

- **Release Pipeline** (`.github/workflows/release.yml`):
  - Automated releases
  - Multi-platform binary builds
  - Docker image tagging

### Local CI Testing

```bash
# Run the same checks as CI
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo audit
```

## Monitoring Setup

### 1. Prometheus Configuration

```bash
# Start Prometheus
docker run -d \
  --name prometheus \
  -p 9090:9090 \
  -v $(pwd)/monitoring/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus:latest
```

### 2. Grafana Setup

```bash
# Start Grafana
docker run -d \
  --name grafana \
  -p 3000:3000 \
  -v $(pwd)/monitoring/grafana:/var/lib/grafana \
  grafana/grafana:latest
```

### 3. Access Dashboards

- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `RUST_BACKTRACE` | Backtrace on panic | `1` |
| `POD_NAME` | Kubernetes pod name | - |
| `POD_NAMESPACE` | Kubernetes namespace | - |

### Configuration Files

- **YAML**: `config.yaml` (primary configuration)
- **TOML**: `config.toml` (alternative format)
- **Environment**: `.env` (environment-specific overrides)

## Troubleshooting

### Common Issues

1. **Build Failures**:
   ```bash
   # Clean and rebuild
   cargo clean
   cargo build --all
   ```

2. **Docker Build Issues**:
   ```bash
   # Check Docker daemon
   docker info
   
   # Clean Docker cache
   docker system prune -a
   ```

3. **Kubernetes Deployment Issues**:
   ```bash
   # Check pod status
   kubectl describe pod <pod-name>
   
   # Check logs
   kubectl logs <pod-name> --previous
   ```

4. **Permission Issues**:
   ```bash
   # Fix script permissions
   chmod +x scripts/*.sh
   ```

### Debug Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run --bin arcus-g3

# Run with backtrace
RUST_BACKTRACE=full cargo run --bin arcus-g3
```

### Performance Tuning

```bash
# Build with optimizations
cargo build --release --all

# Profile build
cargo build --release --all --profile release
```

## Security Considerations

1. **Container Security**:
   - Use non-root user
   - Minimal base image
   - Regular security updates

2. **Kubernetes Security**:
   - Pod Security Standards
   - Network Policies
   - RBAC configuration

3. **Code Security**:
   - Regular dependency audits
   - Static analysis
   - Security testing

## Support

For issues and questions:

- **GitHub Issues**: https://github.com/arcus-g3/swg/issues
- **Documentation**: https://docs.arcus-g3.local
- **Community**: https://community.arcus-g3.local
