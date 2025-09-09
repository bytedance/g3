# Tenant Isolation System

## Overview

The Tenant Isolation System provides comprehensive resource management and isolation for multi-tenant environments in the Arcus-G3 Multi-Tenant Secure Web Gateway. It ensures that each tenant operates within defined resource limits and provides monitoring, alerting, and enforcement capabilities.

## Architecture

### Core Components

1. **TenantIsolationManager** - Central management of tenant configurations and resource limits
2. **TenantResourceMonitor** - Real-time monitoring and enforcement of resource usage
3. **TenantIsolationBuilder** - Builder pattern for creating tenant configurations
4. **PredefinedTenantConfigs** - Pre-configured templates for common tenant types

### Resource Types

The system monitors and enforces limits on the following resources:

- **Connections** - Maximum concurrent connections per tenant
- **Bandwidth** - Maximum bandwidth usage in bytes per second
- **Requests per Second** - Maximum request rate
- **Memory** - Maximum memory usage in bytes
- **CPU** - Maximum CPU usage percentage
- **Servers** - Maximum number of servers per tenant
- **Certificates** - Maximum number of certificates per tenant
- **Audit Log Size** - Maximum audit log size in bytes

## Usage

### Basic Setup

```rust
use arcus_g3_core::{
    TenantId, TenantIsolationManager, TenantResourceMonitor,
    ResourceMonitorConfig, tenant_isolation::TenantConfig,
    tenant_isolation_builder::TenantIsolationBuilder,
};

// Create isolation manager
let isolation_manager = std::sync::Arc::new(
    TenantIsolationManager::new(Duration::from_secs(30))
);

// Create tenant configuration
let tenant_id = TenantId::new();
let config = TenantIsolationBuilder::new(tenant_id.clone(), "My Tenant".to_string())
    .description("Production tenant".to_string())
    .max_connections(1000)
    .max_bandwidth_bps(100_000_000) // 100 MB/s
    .max_requests_per_second(1000)
    .max_memory_bytes(1_073_741_824) // 1 GB
    .max_cpu_percentage(50.0)
    .max_servers(10)
    .max_certificates(100)
    .max_log_retention_days(30)
    .max_audit_log_size(100_000_000) // 100 MB
    .setting("monitoring_enabled".to_string(), serde_json::Value::Bool(true))
    .build();

// Add tenant to isolation manager
isolation_manager.add_tenant(config).await?;
```

### Resource Monitoring

```rust
// Create resource monitor
let config = ResourceMonitorConfig {
    monitoring_interval: Duration::from_secs(30),
    auto_enforcement: true,
    alerting_enabled: true,
    max_violations_before_action: 3,
    ..Default::default()
};

let mut monitor = TenantResourceMonitor::new(isolation_manager.clone(), config);

// Add event callback
monitor.add_callback(|event| {
    match event {
        ResourceMonitorEvent::LimitViolated { tenant_id, resource_type, current_value, limit_value, severity } => {
            println!("Limit violated for tenant {}: {} {} > {}", 
                tenant_id, resource_type, current_value, limit_value);
        },
        ResourceMonitorEvent::TenantDisabled { tenant_id, reason, violation_count } => {
            println!("Tenant {} disabled: {} ({} violations)", 
                tenant_id, reason, violation_count);
        },
        _ => {}
    }
});

// Start monitoring
monitor.start().await?;
```

### Predefined Configurations

The system provides several predefined tenant configurations:

#### Development Tenant
```rust
let config = PredefinedTenantConfigs::development_tenant(
    tenant_id, "Dev Tenant".to_string()
);
```
- Relaxed resource limits
- Debug mode enabled
- 7-day log retention

#### Production Tenant
```rust
let config = PredefinedTenantConfigs::production_tenant(
    tenant_id, "Prod Tenant".to_string()
);
```
- Standard resource limits
- Monitoring and alerting enabled
- 30-day log retention

#### Enterprise Tenant
```rust
let config = PredefinedTenantConfigs::enterprise_tenant(
    tenant_id, "Enterprise Tenant".to_string()
);
```
- High resource limits
- Enterprise features enabled
- 90-day log retention

#### High Security Tenant
```rust
let config = PredefinedTenantConfigs::high_security_tenant(
    tenant_id, "High Security Tenant".to_string()
);
```
- Strict resource limits
- Security mode enabled
- 365-day log retention

#### Resource Constrained Tenant
```rust
let config = PredefinedTenantConfigs::resource_constrained_tenant(
    tenant_id, "Resource Constrained Tenant".to_string()
);
```
- Minimal resource limits
- Resource optimization enabled
- 1-day log retention

## Resource Violations

### Violation Severity Levels

- **Critical** - Immediate action required (e.g., memory, connections)
- **High** - High priority action required (e.g., bandwidth, servers)
- **Medium** - Medium priority action required (e.g., CPU)
- **Low** - Low priority action required (e.g., audit log size)

### Violation Detection

The system detects violations in two ways:

1. **Threshold Exceeded** - Resource usage exceeds alert threshold (e.g., 95% of limit)
2. **Limit Violated** - Resource usage exceeds hard limit

### Automatic Enforcement

When `auto_enforcement` is enabled:

1. Violations are tracked per tenant
2. After exceeding `max_violations_before_action`, the tenant is automatically disabled
3. Disabled tenants can be manually re-enabled after issues are resolved

## Monitoring Events

The system emits the following events:

### ThresholdExceeded
```rust
ResourceMonitorEvent::ThresholdExceeded {
    tenant_id: TenantId,
    resource_type: ResourceType,
    current_usage: f32,    // Usage ratio (0.0 to 1.0)
    threshold: f32,        // Threshold ratio
    severity: ViolationSeverity,
}
```

### LimitViolated
```rust
ResourceMonitorEvent::LimitViolated {
    tenant_id: TenantId,
    resource_type: ResourceType,
    current_value: u64,    // Current usage value
    limit_value: u64,      // Limit value
    severity: ViolationSeverity,
}
```

### TenantDisabled
```rust
ResourceMonitorEvent::TenantDisabled {
    tenant_id: TenantId,
    reason: String,        // Reason for disabling
    violation_count: u32,  // Number of violations
}
```

### TenantReEnabled
```rust
ResourceMonitorEvent::TenantReEnabled {
    tenant_id: TenantId,
}
```

### StatsUpdated
```rust
ResourceMonitorEvent::StatsUpdated {
    stats: TenantIsolationStats,
}
```

## Configuration

### ResourceMonitorConfig

```rust
pub struct ResourceMonitorConfig {
    pub monitoring_interval: Duration,           // How often to check resources
    pub auto_enforcement: bool,                 // Enable automatic enforcement
    pub alerting_enabled: bool,                 // Enable alerting
    pub alert_thresholds: AlertThresholds,      // Alert thresholds
    pub max_violations_before_action: u32,      // Max violations before action
}
```

### AlertThresholds

```rust
pub struct AlertThresholds {
    pub critical_threshold: f32,  // 0.0 to 1.0 (e.g., 0.95 = 95%)
    pub high_threshold: f32,      // 0.0 to 1.0 (e.g., 0.85 = 85%)
    pub medium_threshold: f32,    // 0.0 to 1.0 (e.g., 0.75 = 75%)
    pub low_threshold: f32,       // 0.0 to 1.0 (e.g., 0.65 = 65%)
}
```

## API Reference

### TenantIsolationManager

#### Methods

- `new(monitoring_interval: Duration)` - Create new manager
- `add_tenant(config: TenantConfig)` - Add tenant configuration
- `update_tenant(tenant_id: &TenantId, updates: TenantConfigUpdate)` - Update tenant
- `remove_tenant(tenant_id: &TenantId)` - Remove tenant
- `get_tenant_config(tenant_id: &TenantId)` - Get tenant configuration
- `get_tenant_usage(tenant_id: &TenantId)` - Get tenant resource usage
- `update_resource_usage(tenant_id: &TenantId, usage: TenantResourceUsage)` - Update usage
- `check_resource_violations(tenant_id: &TenantId)` - Check for violations
- `get_all_tenant_configs()` - Get all tenant configurations
- `get_all_tenant_usage()` - Get all tenant usage
- `get_stats()` - Get isolation statistics

### TenantResourceMonitor

#### Methods

- `new(isolation_manager: Arc<TenantIsolationManager>, config: ResourceMonitorConfig)` - Create monitor
- `add_callback<F>(callback: F)` - Add event callback
- `start()` - Start monitoring
- `stop()` - Stop monitoring
- `is_running()` - Check if monitoring is running
- `get_violation_count(tenant_id: &TenantId)` - Get violation count
- `get_disabled_tenants()` - Get disabled tenants
- `re_enable_tenant(tenant_id: &TenantId)` - Re-enable disabled tenant

### TenantIsolationBuilder

#### Methods

- `new(tenant_id: TenantId, name: String)` - Create builder
- `description(description: String)` - Set description
- `resource_limits(limits: TenantResourceLimits)` - Set resource limits
- `max_connections(max: u32)` - Set max connections
- `max_bandwidth_bps(max: u64)` - Set max bandwidth
- `max_requests_per_second(max: u32)` - Set max requests/sec
- `max_memory_bytes(max: u64)` - Set max memory
- `max_cpu_percentage(max: f32)` - Set max CPU percentage
- `max_servers(max: u32)` - Set max servers
- `max_certificates(max: u32)` - Set max certificates
- `max_log_retention_days(max: u32)` - Set max log retention
- `max_audit_log_size(max: u64)` - Set max audit log size
- `enabled(enabled: bool)` - Set enabled status
- `setting(key: String, value: serde_json::Value)` - Add custom setting
- `settings(settings: HashMap<String, serde_json::Value>)` - Add multiple settings
- `build()` - Build tenant configuration

## Best Practices

### Resource Limit Sizing

1. **Start Conservative** - Begin with lower limits and increase as needed
2. **Monitor Usage** - Use monitoring to understand actual usage patterns
3. **Plan for Growth** - Set limits that allow for reasonable growth
4. **Consider Burst Capacity** - Allow for temporary spikes in usage

### Monitoring Configuration

1. **Appropriate Intervals** - Balance between responsiveness and performance
2. **Reasonable Thresholds** - Set thresholds that provide early warning
3. **Action Limits** - Set violation limits that prevent abuse while allowing recovery
4. **Event Handling** - Implement proper event handling for alerts and actions

### Tenant Management

1. **Regular Review** - Periodically review tenant configurations and usage
2. **Proactive Monitoring** - Monitor trends and adjust limits before violations
3. **Graceful Degradation** - Implement fallback mechanisms for disabled tenants
4. **Documentation** - Document tenant configurations and policies

## Examples

See `examples/tenant-isolation-demo.rs` for a comprehensive example demonstrating:

- Creating different types of tenants
- Setting up resource monitoring
- Simulating resource usage
- Handling violations and events
- Managing tenant configurations

## Security Considerations

1. **Resource Isolation** - Ensure tenants cannot access other tenants' resources
2. **Limit Enforcement** - Strictly enforce resource limits to prevent abuse
3. **Audit Logging** - Log all resource violations and enforcement actions
4. **Access Control** - Implement proper access control for tenant management
5. **Monitoring Security** - Secure monitoring endpoints and data

## Performance Considerations

1. **Efficient Monitoring** - Use appropriate monitoring intervals
2. **Resource Overhead** - Consider the overhead of monitoring and enforcement
3. **Scalability** - Design for horizontal scaling of tenant management
4. **Caching** - Cache frequently accessed tenant configurations
5. **Async Operations** - Use async operations for non-blocking resource checks
