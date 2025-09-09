# Multi-stage Dockerfile for Arcus-G3 SWG
# Stage 1: Build stage
FROM rust:1.89-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libc-ares-dev \
    libcapnp-dev \
    liblua5.3-dev \
    python3-dev \
    build-essential \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY arcus-g3-core/ ./arcus-g3-core/
COPY arcus-g3-proxy/ ./arcus-g3-proxy/
COPY arcus-g3-security/ ./arcus-g3-security/
COPY arcus-g3-metrics/ ./arcus-g3-metrics/
COPY arcus-g3-config/ ./arcus-g3-config/
COPY arcus-g3-cli/ ./arcus-g3-cli/

# Build dependencies first (for better caching)
RUN cargo build --release --bin arcus-g3

# Copy source code
COPY . .

# Build the application
RUN cargo build --release --bin arcus-g3

# Stage 2: Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libc-ares4 \
    libcapnp-0.10.4 \
    liblua5.3-0 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r arcus && useradd -r -g arcus arcus

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/arcus-g3 /usr/local/bin/arcus-g3

# Create necessary directories
RUN mkdir -p /app/config /app/logs /app/certs && \
    chown -R arcus:arcus /app

# Switch to non-root user
USER arcus

# Expose ports
EXPOSE 8080 8443 1080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD arcus-g3 --version || exit 1

# Default command
CMD ["arcus-g3", "start", "--config", "/app/config/config.yaml"]
