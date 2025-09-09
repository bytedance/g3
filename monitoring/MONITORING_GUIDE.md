# Arcus-G3 Monitoring and Observability Guide

## Overview

This guide provides comprehensive information about the monitoring and observability infrastructure for the Arcus-G3 Multi-Tenant Secure Web Gateway project.

## Architecture

### Monitoring Stack

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   G3Proxy       │    │   G3StatsD      │    │   Arcus-G3      │
│   (Metrics)     │    │   (Metrics)     │    │   (Metrics)     │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼─────────────┐
                    │      Prometheus           │
                    │   (Metrics Collection)    │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │       Grafana             │
                    │   (Dashboards & Alerts)   │
                    └───────────────────────────┘

┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Application   │    │   System        │    │   Docker        │
│   Logs          │    │   Logs          │    │   Logs          │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼─────────────┐
                    │      Filebeat             │
                    │   (Log Collection)        │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │      Logstash             │
                    │   (Log Processing)        │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │    Elasticsearch          │
                    │   (Log Storage)           │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │       Kibana              │
                    │   (Log Visualization)     │
                    └───────────────────────────┘

┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Application   │    │   G3Proxy       │    │   Arcus-G3      │
│   Traces        │    │   Traces        │    │   Traces        │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼─────────────┐
                    │      Jaeger               │
                    │   (Distributed Tracing)   │
                    └───────────────────────────┘
```

## Components

### 1. Prometheus

**Purpose**: Metrics collection and alerting

**Configuration**:
- Config file: `monitoring/prometheus/prometheus.yml`
- Alert rules: `monitoring/prometheus/rules/arcus-g3-alerts.yml`
- Retention: 30 days
- Scrape interval: 15s

**Key Metrics**:
- System metrics (CPU, memory, disk)
- Application metrics (request rate, response time, error rate)
- Business metrics (tenant-specific, user activity)
- Custom metrics (G3Proxy-specific)

**Access**: http://localhost:9090

### 2. Grafana

**Purpose**: Dashboards and visualization

**Configuration**:
- Datasources: `monitoring/grafana/provisioning/datasources/`
- Dashboards: `monitoring/grafana/dashboards/`
- Admin credentials: admin/admin

**Key Dashboards**:
- Arcus-G3 Overview
- G3Proxy Metrics
- Tenant Metrics
- Security Metrics
- Infrastructure Metrics

**Access**: http://localhost:3000

### 3. Jaeger

**Purpose**: Distributed tracing

**Configuration**:
- Collector endpoint: http://jaeger:14268/api/traces
- UI port: 16686
- Sampling rate: 10%

**Key Features**:
- Request flow visualization
- Performance analysis
- Error tracking
- Service dependency mapping

**Access**: http://localhost:16686

### 4. ELK Stack

**Purpose**: Log aggregation and analysis

#### Elasticsearch
- **Purpose**: Log storage and search
- **Port**: 9200
- **Index pattern**: `arcus-g3-logs-*`
- **Retention**: 30 days

#### Logstash
- **Purpose**: Log processing and transformation
- **Inputs**: Beats, TCP, UDP
- **Outputs**: Elasticsearch, stdout
- **Pipeline**: `monitoring/logstash/pipeline/arcus-g3.conf`

#### Kibana
- **Purpose**: Log visualization and analysis
- **Port**: 5601
- **Features**: Search, filtering, dashboards, alerts

#### Filebeat
- **Purpose**: Log collection and shipping
- **Inputs**: Docker containers, system logs, application logs
- **Output**: Logstash

**Access**: 
- Elasticsearch: http://localhost:9200
- Kibana: http://localhost:5601

### 5. Redis

**Purpose**: Caching and session storage

**Configuration**:
- Port: 6379
- Persistence: RDB + AOF
- Memory limit: 512MB

**Access**: localhost:6379

## Metrics

### System Metrics

| Metric | Description | Type | Labels |
|--------|-------------|------|--------|
| `node_cpu_seconds_total` | CPU usage | Counter | mode, instance |
| `node_memory_MemTotal_bytes` | Total memory | Gauge | instance |
| `node_memory_MemAvailable_bytes` | Available memory | Gauge | instance |
| `node_filesystem_size_bytes` | Disk size | Gauge | instance, mountpoint |
| `node_filesystem_avail_bytes` | Available disk space | Gauge | instance, mountpoint |

### Application Metrics

| Metric | Description | Type | Labels |
|--------|-------------|------|--------|
| `arcus_g3_http_requests_total` | HTTP requests | Counter | method, status, tenant, endpoint |
| `arcus_g3_http_request_duration_seconds` | Request duration | Histogram | method, status, tenant, endpoint |
| `arcus_g3_active_connections` | Active connections | Gauge | - |
| `arcus_g3_tenant_requests_total` | Tenant requests | Counter | tenant_id, tenant_name, service |
| `arcus_g3_user_requests_total` | User requests | Counter | user_id, tenant_id, service |

### Security Metrics

| Metric | Description | Type | Labels |
|--------|-------------|------|--------|
| `arcus_g3_security_events_total` | Security events | Counter | event_type, severity, tenant_id, source |
| `arcus_g3_audit_events_total` | Audit events | Counter | event_type, tenant_id, user_id, action |
| `arcus_g3_error_metrics` | Error metrics | Counter | error_type, tenant_id, service, severity |

### Performance Metrics

| Metric | Description | Type | Labels |
|--------|-------------|------|--------|
| `arcus_g3_performance_metrics` | Performance metrics | Gauge | metric_name, tenant_id, service |
| `arcus_g3_response_time_seconds` | Response time | Histogram | tenant, service, endpoint |
| `arcus_g3_throughput_requests_per_second` | Throughput | Gauge | tenant, service |

## Alerts

### Critical Alerts

| Alert | Condition | Duration | Severity | Description |
|-------|-----------|----------|----------|-------------|
| ServiceDown | `up == 0` | 1m | Critical | Service is down |
| LowDiskSpace | `(1 - (node_filesystem_avail_bytes / node_filesystem_size_bytes)) * 100 > 90` | 5m | Critical | Disk space is above 90% |
| TenantIsolationViolation | `arcus_g3_tenant_isolation_violations_total > 0` | 1m | Critical | Tenant isolation violation detected |

### Warning Alerts

| Alert | Condition | Duration | Severity | Description |
|-------|-----------|----------|----------|-------------|
| HighCPUUsage | `100 - (avg by(instance) (irate(node_cpu_seconds_total{mode="idle"}[5m])) * 100) > 80` | 5m | Warning | CPU usage is above 80% |
| HighMemoryUsage | `(1 - (node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)) * 100 > 85` | 5m | Warning | Memory usage is above 85% |
| HighErrorRate | `rate(http_requests_total{status=~"5.."}[5m]) / rate(http_requests_total[5m]) > 0.1` | 5m | Warning | Error rate is above 10% |
| HighResponseTime | `histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1` | 5m | Warning | 95th percentile response time is above 1s |

## Logs

### Log Structure

```json
{
  "timestamp": "2024-01-01T12:00:00Z",
  "level": "INFO",
  "message": "Request processed",
  "service": "arcus-g3",
  "tenant_id": "tenant-123",
  "user_id": "user-456",
  "request_id": "req-789",
  "session_id": "sess-abc",
  "method": "GET",
  "url": "/api/v1/users",
  "status": 200,
  "response_time": 0.123,
  "bytes_sent": 1024,
  "bytes_received": 512,
  "client_ip": "192.168.1.100",
  "user_agent": "Mozilla/5.0...",
  "geoip": {
    "country": "US",
    "city": "New York"
  }
}
```

### Log Categories

1. **Application Logs**: General application events
2. **Security Logs**: Security-related events
3. **Audit Logs**: Compliance and audit events
4. **Performance Logs**: Performance-related events
5. **Error Logs**: Error and exception events

### Log Levels

- **DEBUG**: Detailed information for debugging
- **INFO**: General information about program execution
- **WARN**: Warning messages for potential issues
- **ERROR**: Error messages for recoverable errors
- **FATAL**: Fatal error messages for unrecoverable errors

## Tracing

### Trace Structure

```json
{
  "trace_id": "trace-123",
  "span_id": "span-456",
  "parent_span_id": "span-789",
  "operation_name": "http_request",
  "start_time": "2024-01-01T12:00:00Z",
  "duration": 123000000,
  "tags": {
    "tenant_id": "tenant-123",
    "user_id": "user-456",
    "request_id": "req-789",
    "method": "GET",
    "url": "/api/v1/users",
    "status": 200
  },
  "logs": [
    {
      "timestamp": "2024-01-01T12:00:00Z",
      "fields": {
        "event": "request_started"
      }
    }
  ]
}
```

### Span Attributes

- **tenant_id**: Tenant identifier
- **user_id**: User identifier
- **request_id**: Request identifier
- **session_id**: Session identifier
- **method**: HTTP method
- **url**: Request URL
- **status**: HTTP status code
- **response_time**: Response time in milliseconds

## Configuration

### Environment Variables

```bash
# Prometheus
PROMETHEUS_RETENTION=30d
PROMETHEUS_SCRAPE_INTERVAL=15s

# Grafana
GRAFANA_ADMIN_PASSWORD=admin
GRAFANA_INSTALL_PLUGINS=grafana-piechart-panel,grafana-worldmap-panel

# Elasticsearch
ELASTICSEARCH_HEAP_SIZE=1g
ELASTICSEARCH_CLUSTER_NAME=arcus-g3

# Jaeger
JAEGER_SAMPLING_RATE=0.1
JAEGER_SERVICE_NAME=arcus-g3

# Redis
REDIS_MAXMEMORY=512mb
REDIS_MAXMEMORY_POLICY=allkeys-lru
```

### Configuration Files

1. **Prometheus**: `monitoring/prometheus/prometheus.yml`
2. **Grafana**: `monitoring/grafana/provisioning/`
3. **Logstash**: `monitoring/logstash/pipeline/arcus-g3.conf`
4. **Filebeat**: `monitoring/filebeat/filebeat.yml`
5. **G3Proxy**: `monitoring/g3proxy-monitoring.yaml`

## Deployment

### Docker Compose

```bash
cd monitoring
docker-compose up -d
```

### Kubernetes

```bash
kubectl apply -f monitoring/k8s/
```

### Helm

```bash
helm install arcus-g3-monitoring monitoring/helm/
```

## Monitoring Best Practices

### 1. Metrics

- Use meaningful metric names
- Include relevant labels
- Set appropriate retention periods
- Monitor metric cardinality
- Use histograms for latency metrics
- Use counters for rate metrics
- Use gauges for current values

### 2. Logs

- Use structured logging (JSON)
- Include correlation IDs
- Set appropriate log levels
- Rotate logs regularly
- Compress old logs
- Monitor log volume

### 3. Alerts

- Set appropriate thresholds
- Use different severity levels
- Include runbook information
- Test alerts regularly
- Avoid alert fatigue
- Use alert grouping

### 4. Dashboards

- Keep dashboards focused
- Use appropriate visualizations
- Include time ranges
- Add drill-down capabilities
- Update dashboards regularly
- Share dashboards with teams

## Troubleshooting

### Common Issues

1. **Services not starting**
   - Check Docker logs
   - Verify port conflicts
   - Check resource limits

2. **Metrics not appearing**
   - Verify scrape targets
   - Check metric names
   - Verify label values

3. **Logs not appearing**
   - Check Filebeat status
   - Verify Logstash pipeline
   - Check Elasticsearch indices

4. **Alerts not firing**
   - Check alert rules
   - Verify metric values
   - Check alert manager

### Health Checks

```bash
# Prometheus
curl http://localhost:9090/-/healthy

# Grafana
curl http://localhost:3000/api/health

# Jaeger
curl http://localhost:16686

# Elasticsearch
curl http://localhost:9200/_cluster/health

# Kibana
curl http://localhost:5601/api/status
```

### Performance Tuning

1. **Prometheus**
   - Adjust scrape intervals
   - Configure retention policies
   - Use recording rules
   - Optimize queries

2. **Elasticsearch**
   - Tune JVM heap size
   - Configure shard allocation
   - Optimize index settings
   - Use index templates

3. **Grafana**
   - Configure caching
   - Optimize dashboard queries
   - Use data source proxies
   - Enable query caching

## Security

### Access Control

- Use strong passwords
- Enable authentication
- Configure authorization
- Use TLS/SSL
- Regular security updates

### Data Protection

- Encrypt sensitive data
- Use secure communication
- Implement data retention
- Regular backups
- Audit access

## Maintenance

### Regular Tasks

1. **Daily**
   - Check service health
   - Review alerts
   - Monitor disk usage

2. **Weekly**
   - Review dashboards
   - Check log retention
   - Update documentation

3. **Monthly**
   - Review metrics
   - Optimize queries
   - Update configurations

### Backup

```bash
# Backup Prometheus data
docker cp monitoring_prometheus_1:/prometheus ./backup/prometheus

# Backup Grafana data
docker cp monitoring_grafana_1:/var/lib/grafana ./backup/grafana

# Backup Elasticsearch data
docker cp monitoring_elasticsearch_1:/usr/share/elasticsearch/data ./backup/elasticsearch
```

## Support

For issues and questions:
- Check the troubleshooting section
- Review service logs
- Create GitHub issues
- Contact the development team

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Jaeger Documentation](https://www.jaegertracing.io/docs/)
- [Elasticsearch Documentation](https://www.elastic.co/guide/en/elasticsearch/reference/current/)
- [Kibana Documentation](https://www.elastic.co/guide/en/kibana/current/)
- [Redis Documentation](https://redis.io/documentation)
