# Escaper System Documentation

## Overview

The Escaper System is a core component of Arcus-G3 that provides flexible traffic routing and forwarding capabilities. It enables the secure web gateway to route client requests through various paths including direct connections, proxy chains, and specialized routing strategies.

## Architecture

The escaper system is built around a trait-based architecture that allows for different types of escapers to be implemented and managed uniformly.

### Core Components

1. **Escaper Trait** - Defines the interface for all escaper implementations
2. **EscapeContext** - Contains contextual information for escape operations
3. **EscapeResult** - Represents the outcome of escape operations
4. **EscaperChain** - Manages multiple escapers in priority order
5. **EscaperManager** - Centralized management of all escaper instances

## Escaper Types

### 1. Direct Escaper
Routes traffic directly to the destination without any intermediate proxies.

**Features:**
- Direct connection establishment
- Support for HTTP, HTTPS, SOCKS, TCP, TLS, UDP, and WebSocket protocols
- Built-in health monitoring
- Performance statistics tracking

**Use Cases:**
- Simple direct connections
- High-performance scenarios
- When no proxy is required

### 2. Proxy Escaper
Routes traffic through another proxy server.

**Features:**
- HTTP/HTTPS proxy support
- SOCKS proxy support
- Authentication support (Basic, Digest, Bearer)
- Connection pooling
- Failover capabilities

**Use Cases:**
- Corporate proxy environments
- Geographic routing
- Security compliance requirements

## Configuration

### Escaper Configuration

```rust
pub struct EscaperConfig {
    pub name: String,
    pub escaper_type: EscaperType,
    pub max_connections: u32,
    pub timeout: Duration,
    pub retry_attempts: u32,
    pub health_check_interval: Duration,
    pub enabled: bool,
}
```

### Escape Context

```rust
pub struct EscapeContext {
    pub tenant_id: TenantId,
    pub protocol: ProtocolType,
    pub destination: String,
    pub source_address: Option<String>,
    pub headers: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

## Usage Examples

### Creating a Direct Escaper

```rust
use arcus_g3_core::escaper::DirectEscaper;

let escaper = DirectEscaper::new("direct-1".to_string(), 100);
```

### Creating a Proxy Escaper

```rust
use arcus_g3_core::escaper::ProxyEscaper;

let escaper = ProxyEscaper::new(
    "proxy-1".to_string(),
    "proxy.example.com:8080".to_string(),
    50,
);
```

### Using the Escaper Manager

```rust
use arcus_g3_core::escaper_manager::EscaperManager;

let manager = EscaperManager::new();
manager.add_escaper(Box::new(escaper)).await?;
```

## Performance Monitoring

The escaper system includes comprehensive performance monitoring:

- **Connection Statistics** - Total connections, active connections, failed connections
- **Performance Metrics** - Response times, throughput, error rates
- **Health Status** - Real-time health monitoring and reporting
- **Resource Usage** - Memory and CPU usage tracking

## Error Handling

The system provides robust error handling with:

- **Retry Logic** - Configurable retry attempts for failed operations
- **Circuit Breaker** - Automatic disabling of failing escapers
- **Fallback Mechanisms** - Automatic failover to alternative escapers
- **Detailed Error Reporting** - Comprehensive error information and logging

## Security Features

- **Authentication Support** - Multiple authentication methods
- **Encryption** - TLS/SSL support for secure connections
- **Access Control** - Tenant-based access restrictions
- **Audit Logging** - Comprehensive audit trails

## Testing

The escaper system includes extensive test coverage:

- **Unit Tests** - Individual component testing
- **Integration Tests** - End-to-end functionality testing
- **Performance Tests** - Load and stress testing
- **Security Tests** - Security vulnerability testing

## Future Enhancements

- **Load Balancing** - Advanced load balancing algorithms
- **Geographic Routing** - Location-based routing decisions
- **Machine Learning** - AI-powered routing optimization
- **Advanced Monitoring** - Enhanced observability and alerting
