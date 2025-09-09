# G3FCGen Integration for Arcus-G3 SWG

This document describes the G3FCGen integration for multi-tenant fake certificate generation in the Arcus-G3 Multi-Tenant Secure Web Gateway.

## Overview

G3FCGen is a high-performance fake certificate generation system that provides:
- Multi-tenant certificate isolation
- Certificate caching and validation
- Automatic certificate rotation and cleanup
- High-performance certificate generation
- Support for various certificate types (TLS, TLCP, etc.)

## Architecture

### Components

1. **MultiTenantG3FCGen**: Core generator for tenant-specific certificate generation
2. **G3FCGenConfigManager**: Configuration management for multi-tenant setup
3. **G3FCGenService**: Service manager for running and managing generators
4. **G3FCGenConfig**: Configuration for certificate generation parameters

### Multi-Tenant Support

- **Tenant Isolation**: Each tenant has its own certificate cache
- **Configurable TTL**: Custom certificate lifetimes per tenant
- **Hostname Validation**: Allowed/denied hostname patterns per tenant
- **Resource Limits**: Configurable cache sizes per tenant

## Configuration

### Global Configuration

```yaml
global:
  default_ttl: 3600  # 1 hour
  max_ttl: 86400     # 24 hours
  cleanup_interval: 300  # 5 minutes
  max_cache_size: 1000
  keep_serial: false
  append_ca_cert: true
```

### CA Configuration

```yaml
ca:
  certificate: "ca.crt"
  private_key: "ca.key"
  password: null
  additional_certs: []
```

### Tenant Configuration

```yaml
tenants:
  tenant-1:
    tenant_id: "550e8400-e29b-41d4-a716-446655440000"
    ttl: 7200  # 2 hours
    cache_size: 500
    cleanup_interval: 180  # 3 minutes
    allowed_hostnames:
      - "*.example.com"
      - "*.tenant1.com"
    denied_hostnames:
      - "*.malicious.com"
    custom_ca_cert: "tenant1-ca.crt"
    custom_ca_key: "tenant1-ca.key"
```

### Backend Configuration

```yaml
backend:
  type: "openssl"
  worker_threads: 4
  queue_size: 1024
  generation_timeout: 30
  stats:
    enabled: true
    export_interval: 60
    export_destination: "console"
```

## Usage

### Basic Usage

```rust
use arcus_g3_core::TenantId;
use arcus_g3_security::g3fcgen_service::G3FCGenService;
use openssl::pkey::PKey;
use openssl::x509::X509;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create G3FCGen service
    let mut g3fcgen_service = G3FCGenService::new();

    // Load configuration
    g3fcgen_service.load_config("g3fcgen-arcus-g3.yaml").await?;

    // Start the service
    g3fcgen_service.start().await?;

    // Create a tenant
    let tenant_id = TenantId::new();

    // Load CA certificate and key
    let ca_cert = load_ca_certificate("ca.crt")?;
    let ca_key = load_ca_private_key("ca.key")?;

    // Create generator for tenant
    g3fcgen_service.create_tenant_generator(tenant_id.clone(), ca_cert, ca_key).await?;

    // Generate certificate
    let certificate = g3fcgen_service.get_certificate(&tenant_id, "example.com").await?;
    println!("Generated certificate: {}", certificate.cert_pem);

    // Stop the service
    g3fcgen_service.stop().await?;

    Ok(())
}
```

### Advanced Usage

```rust
use arcus_g3_security::g3fcgen_config::{G3FCGenConfigFile, G3FCGenTenantConfig};

// Create custom configuration
let mut config = G3FCGenConfigFile::default();

// Add tenant configuration
let tenant_config = G3FCGenTenantConfig {
    tenant_id: tenant_id.clone(),
    ttl: Some(7200),
    cache_size: Some(500),
    cleanup_interval: Some(180),
    allowed_hostnames: Some(vec!["*.example.com".to_string()]),
    denied_hostnames: Some(vec!["*.malicious.com".to_string()]),
    custom_ca_cert: None,
    custom_ca_key: None,
};

config.tenants.insert(tenant_id.clone(), tenant_config);
```

## Certificate Types

### TLS Server Certificates
- Standard TLS server certificates
- Compatible with most TLS implementations
- Support for SNI (Server Name Indication)

### TLCP Certificates
- Chinese TLCP (Tongsuo) certificates
- Support for encryption and signature certificates
- Compliance with Chinese cryptographic standards

### Mimic Certificates
- Certificates that mimic existing certificates
- Preserve serial numbers if configured
- Support for various certificate usages

## Multi-Tenant Features

### Tenant Isolation
- Separate certificate caches per tenant
- Tenant-specific CA certificates
- Isolated certificate generation

### Configuration Management
- Global default settings
- Tenant-specific overrides
- Dynamic configuration reloading
- YAML-based configuration

### Resource Management
- Configurable cache sizes per tenant
- Automatic cleanup of expired certificates
- Memory usage optimization
- Performance monitoring

### Security Features
- Hostname validation per tenant
- Allowed/denied hostname patterns
- Custom CA certificates per tenant
- Certificate rotation and cleanup

## Certificate Lifecycle

### Generation
1. **Request Validation**: Validate hostname against tenant rules
2. **Cache Check**: Check if valid certificate exists in cache
3. **Certificate Generation**: Generate new certificate if needed
4. **Caching**: Store certificate in tenant-specific cache
5. **Return**: Return certificate to requester

### Caching
- **TTL Management**: Certificates expire based on configured TTL
- **Cache Size Limits**: Automatic cleanup when cache is full
- **Cleanup Scheduling**: Regular cleanup of expired certificates
- **Memory Optimization**: Efficient memory usage for large caches

### Cleanup
- **Automatic Cleanup**: Scheduled cleanup of expired certificates
- **Manual Cleanup**: On-demand cleanup for specific tenants
- **Statistics Tracking**: Track cleanup operations and statistics
- **Performance Monitoring**: Monitor cleanup performance

## Performance Considerations

### Cache Management
- Configure appropriate cache sizes for your workload
- Consider memory usage vs. performance trade-offs
- Monitor cache hit rates and cleanup frequency

### Certificate Generation
- Use appropriate worker thread counts
- Consider certificate generation timeout settings
- Monitor generation performance and queue sizes

### Resource Usage
- Monitor memory usage per tenant
- Consider certificate TTL vs. cache size trade-offs
- Optimize cleanup intervals for your workload

## Troubleshooting

### Common Issues

1. **Certificate Generation Fails**: Check CA certificate and key configuration
2. **High Memory Usage**: Reduce cache sizes or increase cleanup frequency
3. **Slow Certificate Generation**: Increase worker threads or check CA key size
4. **Certificate Validation Fails**: Check hostname validation rules

### Debugging

Enable debug logging:
```rust
tracing_subscriber::fmt()
    .with_env_filter("debug")
    .init();
```

### Monitoring

Check service statistics:
```rust
let stats = g3fcgen_service.get_service_stats().await;
println!("Service running: {}", stats.is_running);
println!("Tenant count: {}", stats.tenant_count);
```

## Integration with Arcus-G3

The G3FCGen integration is designed to work seamlessly with the Arcus-G3 SWG:

1. **Automatic Tenant Detection**: Certificates are automatically tagged with tenant ID
2. **Configuration Management**: Integrated with Arcus-G3 configuration system
3. **Service Management**: Managed as part of the Arcus-G3 service lifecycle
4. **Security Integration**: Integrated with Arcus-G3 security policies

## Security Considerations

- **Certificate Validation**: Ensure proper certificate validation
- **CA Security**: Protect CA private keys with appropriate security measures
- **Hostname Validation**: Implement proper hostname validation rules
- **Certificate Rotation**: Regular rotation of CA certificates
- **Access Control**: Implement appropriate access controls for certificate generation

## Future Enhancements

- **Certificate Transparency**: Integration with certificate transparency logs
- **OCSP Support**: Online Certificate Status Protocol support
- **Certificate Revocation**: Certificate revocation list support
- **Advanced Validation**: More sophisticated hostname validation
- **Performance Optimization**: Further performance optimizations
