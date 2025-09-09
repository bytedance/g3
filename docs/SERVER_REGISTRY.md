# Server Registry for Arcus-G3 Multi-Tenant SWG

This document describes the Server Registry system for managing multi-tenant servers in the Arcus-G3 Multi-Tenant Secure Web Gateway.

## Overview

The Server Registry provides a centralized system for managing multiple types of proxy servers across different tenants. It supports:

- **Multi-tenant server isolation**
- **Server lifecycle management** (start/stop/restart/reload)
- **Health monitoring and statistics**
- **Flexible server configuration**
- **High-performance server management**

## Architecture

### Core Components

1. **ServerRegistry**: Core registry for server storage and management
2. **ServerManager**: High-level server management operations
3. **ServerConfigBuilder**: Builder pattern for server configuration
4. **PredefinedConfigs**: Common server configuration templates

### Server Types

The registry supports multiple server types:

- **HTTP Proxy**: Standard HTTP proxy server
- **HTTPS Proxy**: TLS-enabled HTTP proxy server
- **SOCKS Proxy**: SOCKS4/5 proxy server
- **TCP Proxy**: Raw TCP proxy server
- **TLS Proxy**: TLS termination proxy server
- **SNI Proxy**: Server Name Indication proxy server

## Usage

### Basic Server Registration

```rust
use arcus_g3_core::{
    TenantId,
    server_registry::{ServerRegistry, ServerType},
    server_manager::ServerManager,
    server_builder::ServerConfigBuilder,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create server manager
    let server_manager = ServerManager::new();
    server_manager.initialize().await?;

    // Create tenant
    let tenant_id = TenantId::new();

    // Create server configuration
    let config = ServerConfigBuilder::http_proxy(
        "my-http-proxy".to_string(),
        tenant_id.clone(),
        8080,
    )
    .max_connections(1000)
    .connection_timeout(Duration::from_secs(30))
    .build();

    // Register server
    let server_id = server_manager.register_tenant_server(tenant_id, config).await?;
    
    // Server is automatically started (auto-start enabled by default)
    println!("Server {} registered and started", server_id);

    Ok(())
}
```

### Server Lifecycle Management

```rust
// Start a server
server_manager.start_server(&server_id).await?;

// Stop a server
server_manager.stop_server(&server_id).await?;

// Restart a server
server_manager.restart_server(&server_id).await?;

// Reload server configuration
let new_config = ServerConfigBuilder::http_proxy(
    "updated-proxy".to_string(),
    tenant_id,
    8080,
).max_connections(2000).build();

server_manager.reload_server(&server_id, new_config).await?;
```

### Tenant Operations

```rust
// Start all servers for a tenant
server_manager.start_tenant_servers(&tenant_id).await?;

// Stop all servers for a tenant
server_manager.stop_tenant_servers(&tenant_id).await?;

// Restart all servers for a tenant
server_manager.restart_tenant_servers(&tenant_id).await?;

// Get all servers for a tenant
let tenant_servers = server_manager.get_tenant_servers(&tenant_id).await;

// Remove all servers for a tenant
server_manager.remove_tenant_servers(&tenant_id).await?;
```

### Server Queries

```rust
// Get server by ID
if let Some(server) = server_manager.get_server(&server_id).await {
    println!("Server status: {:?}", server.status);
}

// Get servers by type
let http_servers = server_manager.get_servers_by_type(&ServerType::Http).await;

// Get all servers
let all_servers = server_manager.get_all_servers().await;
```

## Configuration

### Server Configuration

```rust
pub struct ServerConfig {
    pub name: String,                    // Server name
    pub server_type: ServerType,         // Type of server
    pub listen_addr: String,             // Listen address
    pub listen_port: u16,                // Listen port
    pub tenant_id: TenantId,             // Tenant ID
    pub config: serde_json::Value,       // Custom configuration
    pub max_connections: Option<u32>,    // Max connections
    pub connection_timeout: Option<Duration>, // Connection timeout
    pub enable_tls: bool,                // Enable TLS
    pub tls_cert_path: Option<String>,   // TLS certificate path
    pub tls_key_path: Option<String>,    // TLS private key path
}
```

### Builder Pattern

```rust
let config = ServerConfigBuilder::https_proxy(
    "secure-proxy".to_string(),
    tenant_id,
    8443,
)
.tls_config("cert.pem".to_string(), "key.pem".to_string())
.max_connections(500)
.connection_timeout(Duration::from_secs(15))
.config_field("tls_version", Value::String("TLS1.3".to_string()))
.build();
```

### Predefined Configurations

```rust
// Standard HTTP proxy
let config = PredefinedConfigs::standard_http_proxy(tenant_id);

// Standard HTTPS proxy
let config = PredefinedConfigs::standard_https_proxy(
    tenant_id,
    "cert.pem".to_string(),
    "key.pem".to_string(),
);

// High-performance HTTP proxy
let config = PredefinedConfigs::high_performance_http_proxy(tenant_id);

// Load balancer
let config = PredefinedConfigs::load_balancer(
    tenant_id,
    vec!["backend1:8080".to_string(), "backend2:8080".to_string()],
);
```

## Server Status

Servers can have the following statuses:

- **Stopped**: Server is not running
- **Starting**: Server is in the process of starting
- **Running**: Server is active and accepting connections
- **Stopping**: Server is in the process of stopping
- **Error**: Server encountered an error

## Statistics

### Registry Statistics

```rust
let stats = server_manager.get_stats().await;
println!("Total servers: {}", stats.total_servers);
println!("Running servers: {}", stats.running_servers);
println!("Stopped servers: {}", stats.stopped_servers);
println!("Error servers: {}", stats.error_servers);
println!("Total tenants: {}", stats.total_tenants);
```

### Tenant Statistics

```rust
let tenant_stats = server_manager.get_tenant_stats(&tenant_id).await;
println!("Tenant servers: {}", tenant_stats.total_servers);
println!("Running servers: {}", tenant_stats.running_servers);
println!("Total connections: {}", tenant_stats.total_connections);
println!("Active connections: {}", tenant_stats.active_connections);
```

### Server Statistics

```rust
if let Some(server) = server_manager.get_server(&server_id).await {
    println!("Total requests: {}", server.stats.total_requests);
    println!("Bytes transferred: {}", server.stats.total_bytes_transferred);
    println!("Average response time: {}ms", server.stats.avg_response_time_ms);
    println!("Error count: {}", server.stats.error_count);
}
```

## Health Monitoring

The server registry includes built-in health monitoring:

```rust
// Create server manager with health check
let server_manager = ServerManager::with_config(
    Duration::from_secs(30), // Health check interval
    true,  // Auto-start enabled
    true,  // Health check enabled
);

// Health checks run automatically in the background
// Servers are monitored and statistics are updated
```

## Multi-Tenant Features

### Tenant Isolation

- Each tenant has isolated server configurations
- Servers are mapped to specific tenants
- Tenant-specific operations (start/stop all servers)
- Tenant-specific statistics and monitoring

### Resource Management

- Configurable connection limits per server
- Connection timeout management
- Memory and CPU usage monitoring
- Automatic cleanup of stopped servers

### Security

- TLS configuration per server
- Certificate management per tenant
- Access control and authentication
- Audit logging for server operations

## Performance Considerations

### Connection Management

- Configurable maximum connections per server
- Connection pooling and reuse
- Timeout handling and cleanup
- Load balancing across multiple servers

### Memory Usage

- Efficient server instance storage
- Configurable cache sizes
- Automatic cleanup of unused resources
- Memory usage monitoring

### Scalability

- Horizontal scaling support
- Load balancing capabilities
- Health check and failover
- Performance monitoring and alerting

## Error Handling

The server registry provides comprehensive error handling:

```rust
// Server registration errors
match server_manager.register_tenant_server(tenant_id, config).await {
    Ok(server_id) => println!("Server registered: {}", server_id),
    Err(e) => eprintln!("Failed to register server: {}", e),
}

// Server operation errors
match server_manager.start_server(&server_id).await {
    Ok(()) => println!("Server started successfully"),
    Err(e) => eprintln!("Failed to start server: {}", e),
}
```

## Integration with Arcus-G3

The server registry integrates seamlessly with other Arcus-G3 components:

- **Tenant Management**: Automatic tenant detection and mapping
- **Configuration Management**: Dynamic configuration reloading
- **Metrics Collection**: Server statistics and monitoring
- **Security**: TLS certificate management and validation
- **Logging**: Comprehensive audit and debug logging

## Best Practices

### Server Configuration

1. **Use descriptive server names** that include tenant information
2. **Set appropriate connection limits** based on expected load
3. **Configure timeouts** to prevent resource exhaustion
4. **Enable TLS** for secure communications
5. **Use predefined configurations** for common server types

### Tenant Management

1. **Isolate servers by tenant** to prevent cross-tenant access
2. **Monitor tenant resource usage** to prevent abuse
3. **Implement tenant-specific policies** for server management
4. **Use consistent naming conventions** for tenant servers

### Performance Optimization

1. **Monitor server statistics** regularly
2. **Adjust connection limits** based on usage patterns
3. **Use health checks** to detect and handle failures
4. **Implement load balancing** for high-traffic scenarios
5. **Clean up unused servers** to free resources

## Troubleshooting

### Common Issues

1. **Server fails to start**: Check configuration and port availability
2. **High memory usage**: Review connection limits and cleanup policies
3. **Connection timeouts**: Adjust timeout settings and check network
4. **TLS errors**: Verify certificate paths and permissions

### Debug Information

Enable debug logging to troubleshoot issues:

```rust
tracing_subscriber::fmt()
    .with_env_filter("debug")
    .init();
```

### Health Check Monitoring

Monitor health check logs to identify server issues:

```rust
// Health checks run automatically and log server status
// Check logs for server health information
```

## Future Enhancements

- **Server clustering** for high availability
- **Dynamic scaling** based on load
- **Advanced load balancing** algorithms
- **Server migration** between tenants
- **Performance analytics** and reporting
