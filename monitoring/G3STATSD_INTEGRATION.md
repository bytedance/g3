# G3StatsD Integration for Arcus-G3 SWG

This document describes the G3StatsD integration for multi-tenant metrics collection in the Arcus-G3 Multi-Tenant Secure Web Gateway.

## Overview

G3StatsD is a high-performance metrics collection and aggregation system that provides:
- Multi-tenant metrics isolation
- Multiple export destinations (Prometheus, Graphite, InfluxDB, OpenTSDB)
- Configurable aggregation and regulation
- High-performance UDP and Unix socket input
- Real-time metrics processing

## Architecture

### Components

1. **MultiTenantG3StatsDCollector**: Core collector for tenant-specific metrics
2. **G3StatsDConfigManager**: Configuration management for multi-tenant setup
3. **G3StatsDService**: Service manager for running and managing collectors
4. **G3StatsDExportConfig**: Export configuration for different destinations

### Multi-Tenant Support

- **Tenant Isolation**: Each tenant has its own metrics namespace
- **Configurable Prefixes**: Custom metric prefixes per tenant
- **Selective Export**: Different export destinations per tenant
- **Resource Limits**: Configurable limits per tenant

## Configuration

### Global Configuration

```yaml
global:
  default_export_interval: 10  # seconds
  default_metric_prefix: "arcus_g3"
  include_tenant_id_by_default: true
  buffer_size: 10000
  worker_threads: 4
```

### Tenant Configuration

```yaml
tenants:
  tenant-1:
    tenant_id: "550e8400-e29b-41d4-a716-446655440000"
    metric_prefix: "tenant1"
    include_tenant_id: true
    export_interval: 5
    enabled_collectors: ["aggregate_5s"]
    enabled_exporters: ["prometheus", "console"]
    custom_tags:
      environment: "production"
      region: "us-west-2"
```

### Importers

G3StatsD supports multiple input methods:

#### UDP StatsD
```yaml
importers:
  - name: statsd_udp
    type: statsd_udp
    collector: aggregate_10s
    listen: "0.0.0.0:8125"
    config:
      buffer_size: 8192
      max_packet_size: 65536
```

#### Unix Socket StatsD
```yaml
importers:
  - name: statsd_unix
    type: statsd_unix
    collector: aggregate_10s
    listen: "/tmp/g3statsd-arcus-g3.sock"
    config:
      buffer_size: 8192
```

### Collectors

Collectors aggregate and process metrics:

#### Aggregate Collector
```yaml
collectors:
  - name: aggregate_10s
    type: aggregate
    next: regulate_10s
    emit_interval: "10s"
    join_tags:
      - "tenant_id"
      - "service"
    config:
      buffer_size: 1000
      flush_interval: "10s"
```

#### Regulate Collector
```yaml
collectors:
  - name: regulate_10s
    type: regulate
    exporter: prometheus
    prefix: "arcus_g3"
    config:
      max_metrics_per_second: 1000
      max_tags_per_metric: 50
```

### Exporters

Exporters send metrics to various destinations:

#### Prometheus
```yaml
exporters:
  - name: prometheus
    type: prometheus
    destination:
      type: prometheus
      port: 9090
      path: "/metrics"
    config:
      namespace: "arcus_g3"
      include_tenant_labels: true
```

#### Graphite
```yaml
exporters:
  - name: graphite
    type: graphite
    destination:
      type: graphite
      host: "graphite.example.com"
      port: 2003
      prefix: "arcus_g3"
    config:
      protocol: "tcp"
      batch_size: 100
      flush_interval: "10s"
```

#### InfluxDB
```yaml
exporters:
  - name: influxdb
    type: influxdb
    destination:
      type: influxdb
      url: "http://influxdb.example.com:8086"
      database: "arcus_g3"
      username: "arcus_g3"
      password: "secret"
    config:
      precision: "s"
      batch_size: 1000
      flush_interval: "10s"
```

#### OpenTSDB
```yaml
exporters:
  - name: opentsdb
    type: opentsdb
    destination:
      type: opentsdb
      host: "opentsdb.example.com"
      port: 4242
      prefix: "arcus_g3"
    config:
      protocol: "tcp"
      batch_size: 100
      flush_interval: "10s"
```

## Usage

### Basic Usage

```rust
use arcus_g3_core::TenantId;
use arcus_g3_metrics::g3statsd_service::G3StatsDService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create G3StatsD service
    let mut g3statsd_service = G3StatsDService::new();

    // Load configuration
    g3statsd_service.load_config("g3statsd-arcus-g3.yaml").await?;

    // Start the service
    g3statsd_service.start().await?;

    // Create a tenant
    let tenant_id = TenantId::new_v4();

    // Create collector for tenant
    g3statsd_service.create_tenant_collector(tenant_id.clone()).await?;

    // Add metrics
    g3statsd_service.add_counter(&tenant_id, "http_requests", 1.0).await?;
    g3statsd_service.add_gauge(&tenant_id, "active_connections", 42.0).await?;
    g3statsd_service.add_histogram(&tenant_id, "response_time", 150.0).await?;
    g3statsd_service.add_timer(&tenant_id, "processing_time", 25.0).await?;

    // Stop the service
    g3statsd_service.stop().await?;

    Ok(())
}
```

### Advanced Usage

```rust
use arcus_g3_metrics::g3statsd_config::{G3StatsDConfig, G3StatsDTenantConfig, G3StatsDExporterConfig, G3StatsDExporterDestination};

// Create custom configuration
let mut config = G3StatsDConfig::default();

// Add tenant configuration
let tenant_config = G3StatsDTenantConfig {
    tenant_id: tenant_id.clone(),
    metric_prefix: Some("custom_prefix".to_string()),
    include_tenant_id: Some(true),
    export_interval: Some(5),
    enabled_collectors: vec!["aggregate_5s".to_string()],
    enabled_exporters: vec!["prometheus".to_string()],
    custom_tags: [("environment".to_string(), "production".to_string())].into(),
};

config.tenants.insert(tenant_id.clone(), tenant_config);

// Add custom exporter
let prometheus_exporter = G3StatsDExporterConfig {
    name: "prometheus".to_string(),
    r#type: "prometheus".to_string(),
    destination: G3StatsDExporterDestination::Prometheus {
        port: 9090,
        path: Some("/metrics".to_string()),
    },
    config: [("namespace".to_string(), serde_yaml::Value::String("arcus_g3".to_string()))].into(),
};

config.exporters.push(prometheus_exporter);
```

## Metrics Types

### Counters
Counters are monotonically increasing values:
```rust
g3statsd_service.add_counter(&tenant_id, "http_requests_total", 1.0).await?;
```

### Gauges
Gauges are point-in-time values:
```rust
g3statsd_service.add_gauge(&tenant_id, "active_connections", 42.0).await?;
```

### Histograms
Histograms track value distributions:
```rust
g3statsd_service.add_histogram(&tenant_id, "response_time_ms", 150.0).await?;
```

### Timers
Timers are specialized histograms for time measurements:
```rust
g3statsd_service.add_timer(&tenant_id, "processing_time_ms", 25.0).await?;
```

## Multi-Tenant Features

### Tenant Isolation
- Each tenant has its own metrics namespace
- Metrics are isolated by tenant ID
- Configurable prefixes per tenant

### Resource Management
- Configurable export intervals per tenant
- Selective exporter enablement per tenant
- Custom tags per tenant

### Monitoring
- Service statistics per tenant
- Metrics collection status per tenant
- Export status per tenant

## Performance Considerations

### Buffer Sizes
- Configure appropriate buffer sizes for your workload
- Consider memory usage vs. performance trade-offs
- Monitor buffer utilization

### Export Intervals
- Balance between real-time metrics and performance
- Consider downstream system capacity
- Use different intervals for different metric types

### Worker Threads
- Configure based on CPU cores available
- Consider I/O bound vs. CPU bound workloads
- Monitor worker utilization

## Troubleshooting

### Common Issues

1. **Metrics not appearing**: Check tenant configuration and enabled exporters
2. **High memory usage**: Reduce buffer sizes or increase export frequency
3. **Export failures**: Check destination connectivity and configuration
4. **Performance issues**: Adjust worker threads and buffer sizes

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
let stats = g3statsd_service.get_service_stats().await;
println!("Service running: {}", stats.is_running);
println!("Tenant count: {}", stats.tenant_count);
```

## Integration with Arcus-G3

The G3StatsD integration is designed to work seamlessly with the Arcus-G3 SWG:

1. **Automatic Tenant Detection**: Metrics are automatically tagged with tenant ID
2. **Configuration Management**: Integrated with Arcus-G3 configuration system
3. **Service Management**: Managed as part of the Arcus-G3 service lifecycle
4. **Monitoring Integration**: Exports to Arcus-G3 monitoring stack

## Security Considerations

- **Tenant Isolation**: Ensure metrics are properly isolated between tenants
- **Access Control**: Implement appropriate access controls for metrics endpoints
- **Data Retention**: Configure appropriate data retention policies
- **Encryption**: Use encrypted connections for sensitive metrics data

## Future Enhancements

- **Real-time Dashboards**: Integration with Grafana for real-time visualization
- **Alerting**: Integration with alerting systems for threshold-based alerts
- **Machine Learning**: Integration with ML systems for anomaly detection
- **Custom Aggregations**: Support for custom aggregation functions
