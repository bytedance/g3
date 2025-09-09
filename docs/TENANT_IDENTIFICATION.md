# Tenant Identification System for Arcus-G3 Multi-Tenant SWG

This document describes the Tenant Identification System for managing multi-tenant identification in the Arcus-G3 Multi-Tenant Secure Web Gateway.

## Overview

The Tenant Identification System provides multiple methods for identifying tenants in the Arcus-G3 SWG, including:

- **IP Range-based Identification**: Identify tenants based on client IP addresses
- **SSO Header-based Identification**: Identify tenants using SSO headers
- **Domain/SNI-based Identification**: Identify tenants using domain names or SNI
- **Custom Header-based Identification**: Identify tenants using custom headers
- **Certificate-based Identification**: Identify tenants using client certificates
- **Query Parameter-based Identification**: Identify tenants using query parameters

## Architecture

### Core Components

1. **TenantIdentificationManager**: High-level management of tenant identification
2. **TenantRouter**: Core routing logic for tenant identification
3. **TenantIdentificationBuilder**: Builder pattern for configuration
4. **PredefinedIdentificationConfigs**: Common identification configurations

### Identification Methods

The system supports multiple identification methods with configurable priorities:

- **IP Range**: Match client IP addresses against configured ranges
- **SSO Header**: Match SSO headers against configured patterns
- **Domain**: Match domain names using wildcard or regex patterns
- **Custom Header**: Match custom headers against configured patterns
- **Certificate**: Match certificate subject/issuer against patterns
- **Query Parameter**: Match query parameters against configured patterns

## Usage

### Basic Tenant Identification

```rust
use arcus_g3_core::{
    TenantId,
    tenant_identification::TenantIdentificationRequest,
    tenant_identification_manager::TenantIdentificationManager,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create identification manager
    let manager = TenantIdentificationManager::new();
    manager.initialize().await?;

    // Create tenant
    let tenant_id = TenantId::new();

    // Add IP range identification
    manager.add_ip_range_identification(
        tenant_id.clone(),
        vec![IpRange::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 255)),
        )],
        100,
    ).await?;

    // Create identification request
    let request = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))),
        headers: HashMap::new(),
        domain: None,
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    // Identify tenant
    if let Some(result) = manager.identify_tenant(&request).await? {
        println!("Identified tenant: {}", result.tenant_id);
    }

    Ok(())
}
```

### Multiple Identification Methods

```rust
// Add multiple identification methods for a tenant
manager.add_ip_range_identification(tenant_id.clone(), ip_ranges, 100).await?;
manager.add_sso_header_identification(
    tenant_id.clone(),
    "X-Tenant-ID".to_string(),
    vec![tenant_id.to_string()],
    false,
    90,
).await?;
manager.add_domain_identification(
    tenant_id.clone(),
    vec!["tenant.example.com".to_string()],
    true, // Use wildcard matching
    false,
    80,
).await?;
```

### Predefined Configurations

```rust
use arcus_g3_core::tenant_identification_builder::PredefinedIdentificationConfigs;

// Corporate IP range identification
let config = PredefinedIdentificationConfigs::corporate_ip_range(tenant_id);

// Web application domain identification
let config = PredefinedIdentificationConfigs::web_application_domain(
    tenant_id,
    vec!["app.example.com".to_string()],
);

// Enterprise SSO header identification
let config = PredefinedIdentificationConfigs::enterprise_sso_header(
    tenant_id,
    "X-Enterprise-Tenant".to_string(),
    vec![tenant_id.to_string()],
);
```

### Builder Pattern

```rust
use arcus_g3_core::tenant_identification_builder::TenantIdentificationBuilder;

let config = TenantIdentificationBuilder::new(tenant_id)
    .add_ip_range_addresses(
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 255)),
        100,
    )
    .add_sso_header(
        "X-Tenant-ID".to_string(),
        vec![tenant_id.to_string()],
        false,
        90,
    )
    .add_domain(
        vec!["tenant.example.com".to_string()],
        true, // Use wildcard matching
        false,
        80,
    )
    .build();
```

## Configuration

### IP Range Configuration

```rust
pub struct IpRangeConfig {
    pub ranges: Vec<IpRange>,
    pub priority: u32,
    pub match_ipv4: bool,
    pub match_ipv6: bool,
}

// Create IP range from CIDR notation
let range = IpRange::from_cidr("192.168.1.0/24")?;

// Create IP range from addresses
let range = IpRange::new(
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)),
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 255)),
);
```

### SSO Header Configuration

```rust
pub struct SsoHeaderConfig {
    pub header_name: String,
    pub patterns: Vec<String>,
    pub use_regex: bool,
    pub priority: u32,
    pub case_sensitive: bool,
}

// Add SSO header identification
manager.add_sso_header_identification(
    tenant_id,
    "X-Tenant-ID".to_string(),
    vec!["tenant-123".to_string()],
    false, // Don't use regex
    100,
).await?;
```

### Domain Configuration

```rust
pub struct DomainConfig {
    pub domains: Vec<String>,
    pub use_wildcard: bool,
    pub use_regex: bool,
    pub priority: u32,
    pub case_sensitive: bool,
}

// Add domain identification with wildcard support
manager.add_domain_identification(
    tenant_id,
    vec!["*.example.com".to_string(), "api.example.com".to_string()],
    true, // Use wildcard matching
    false, // Don't use regex
    100,
).await?;
```

### Custom Header Configuration

```rust
pub struct CustomHeaderConfig {
    pub header_name: String,
    pub patterns: Vec<String>,
    pub use_regex: bool,
    pub priority: u32,
    pub case_sensitive: bool,
}

// Add custom header identification
manager.add_custom_header_identification(
    tenant_id,
    "X-User-Tenant".to_string(),
    vec!["tenant-123".to_string()],
    false,
    100,
).await?;
```

### Certificate Configuration

```rust
pub struct CertificateConfig {
    pub subject_patterns: Vec<String>,
    pub issuer_patterns: Vec<String>,
    pub use_regex: bool,
    pub priority: u32,
    pub case_sensitive: bool,
}

// Add certificate identification
manager.add_certificate_identification(
    tenant_id,
    vec!["CN=tenant-123".to_string()],
    vec!["CN=Internal-CA".to_string()],
    false,
    100,
).await?;
```

### Query Parameter Configuration

```rust
pub struct QueryParamConfig {
    pub param_name: String,
    pub patterns: Vec<String>,
    pub use_regex: bool,
    pub priority: u32,
    pub case_sensitive: bool,
}

// Add query parameter identification
manager.add_query_param_identification(
    tenant_id,
    "tenant_id".to_string(),
    vec!["tenant-123".to_string()],
    false,
    100,
).await?;
```

## Identification Process

### Request Structure

```rust
pub struct TenantIdentificationRequest {
    pub client_ip: Option<IpAddr>,
    pub headers: HashMap<String, String>,
    pub domain: Option<String>,
    pub query_params: HashMap<String, String>,
    pub cert_subject: Option<String>,
    pub cert_issuer: Option<String>,
}
```

### Result Structure

```rust
pub struct TenantIdentificationResult {
    pub tenant_id: TenantId,
    pub method: TenantIdentificationMethod,
    pub confidence: f64,
    pub metadata: HashMap<String, String>,
}
```

### Identification Flow

1. **Tenant-specific Methods**: Try identification methods configured for specific tenants
2. **Global Methods**: Try global identification methods
3. **Fallback**: Use default tenant if fallback is enabled
4. **Statistics**: Update identification statistics

## Priority System

Identification methods are processed in priority order:

1. **Higher Priority First**: Methods with higher priority values are tried first
2. **Tenant-specific First**: Tenant-specific methods are tried before global methods
3. **First Match Wins**: The first successful identification method determines the tenant

## Statistics and Monitoring

### Identification Statistics

```rust
pub struct IdentificationStats {
    pub total_attempts: u64,
    pub successful_identifications: u64,
    pub failed_identifications: u64,
    pub method_counts: HashMap<String, u64>,
    pub avg_identification_time_us: f64,
    pub last_updated: SystemTime,
}
```

### Statistics Usage

```rust
// Get current statistics
let stats = manager.get_stats().await;
println!("Total attempts: {}", stats.total_attempts);
println!("Success rate: {:.2}%", 
    (stats.successful_identifications as f64 / stats.total_attempts as f64) * 100.0);

// Reset statistics
manager.reset_stats().await?;
```

## Advanced Features

### Configuration Validation

```rust
// Validate configuration for duplicate priorities
manager.validate_configuration().await?;
```

### Method Management

```rust
// Get methods for a tenant
if let Some(methods) = manager.get_tenant_methods(&tenant_id).await {
    println!("Tenant has {} identification methods", methods.len());
}

// Remove specific method
manager.remove_tenant_method(&tenant_id, &method).await?;

// Clear all methods for a tenant
manager.clear_tenant_methods(&tenant_id).await?;
```

### Fallback Configuration

```rust
// Set default tenant
manager.set_default_tenant(tenant_id).await?;

// Enable/disable fallback
manager.set_fallback_enabled(true).await?;
```

## IP Range Helpers

### Common IP Ranges

```rust
use arcus_g3_core::tenant_identification_builder::IpRangeHelpers;

// Private IPv4 ranges
let private_ranges = IpRangeHelpers::private_ipv4_range();

// Localhost ranges
let localhost_ranges = IpRangeHelpers::localhost_range();

// CIDR notation parsing
let ranges = IpRangeHelpers::from_cidr_list(vec![
    "192.168.1.0/24",
    "10.0.0.0/8",
    "172.16.0.0/12",
])?;
```

## Best Practices

### Configuration Management

1. **Use Appropriate Priorities**: Set higher priorities for more specific methods
2. **Avoid Duplicate Priorities**: Ensure unique priorities within the same tenant
3. **Use Fallback Wisely**: Enable fallback only when appropriate
4. **Validate Configurations**: Always validate configurations before deployment

### Performance Optimization

1. **Order Methods by Likelihood**: Place most likely methods first
2. **Use Specific Patterns**: Use specific patterns over generic ones
3. **Monitor Statistics**: Regularly check identification statistics
4. **Cache Configurations**: Use configuration caching for better performance

### Security Considerations

1. **Validate Input**: Always validate identification input
2. **Use Secure Headers**: Use secure headers for sensitive identification
3. **Log Identification Events**: Log identification events for auditing
4. **Regular Updates**: Regularly update identification patterns

## Troubleshooting

### Common Issues

1. **No Tenant Identified**: Check if fallback is enabled and default tenant is set
2. **Wrong Tenant Identified**: Check method priorities and patterns
3. **Performance Issues**: Check method order and pattern complexity
4. **Configuration Errors**: Validate configuration for duplicate priorities

### Debug Information

Enable debug logging to troubleshoot identification issues:

```rust
tracing_subscriber::fmt()
    .with_env_filter("debug")
    .init();
```

### Statistics Analysis

Use identification statistics to identify issues:

```rust
let stats = manager.get_stats().await;
if stats.failed_identifications > stats.successful_identifications {
    println!("High failure rate - check configuration");
}
```

## Integration with Arcus-G3

The tenant identification system integrates seamlessly with other Arcus-G3 components:

- **Server Registry**: Automatic tenant identification for server routing
- **Configuration Management**: Dynamic configuration updates
- **Metrics Collection**: Identification statistics and monitoring
- **Security**: Secure tenant identification and validation
- **Logging**: Comprehensive audit and debug logging

## Future Enhancements

- **Machine Learning**: ML-based tenant identification
- **Behavioral Analysis**: User behavior-based identification
- **Geolocation**: Geographic location-based identification
- **Time-based Rules**: Time-based identification rules
- **Advanced Analytics**: Advanced identification analytics and reporting
