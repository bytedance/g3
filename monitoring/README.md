# Arcus-G3 Monitoring and Observability Infrastructure

This directory contains the complete monitoring and observability setup for the Arcus-G3 Multi-Tenant Secure Web Gateway project.

## Overview

The monitoring infrastructure includes:
- **Prometheus** - Metrics collection and alerting
- **Grafana** - Dashboards and visualization
- **Jaeger** - Distributed tracing
- **ELK Stack** - Log aggregation and analysis
  - **Elasticsearch** - Log storage and search
  - **Logstash** - Log processing and transformation
  - **Kibana** - Log visualization and analysis
  - **Filebeat** - Log collection and shipping
- **Redis** - Caching and session storage
- **Node Exporter** - System metrics collection

## Quick Start

### Prerequisites

- Docker and Docker Compose
- kubectl (optional, for Kubernetes deployment)
- curl (for health checks)

### Setup

1. **Run the setup script:**
   ```bash
   ./monitoring/setup.sh
   ```

2. **Or manually start services:**
   ```bash
   cd monitoring
   docker-compose up -d
   ```

### Access Services

| Service | URL | Credentials |
|---------|-----|-------------|
| Prometheus | http://localhost:9090 | - |
| Grafana | http://localhost:3000 | admin/admin |
| Jaeger | http://localhost:16686 | - |
| Elasticsearch | http://localhost:9200 | - |
| Kibana | http://localhost:5601 | - |

## Architecture

### Metrics Flow

```
G3Proxy/G3StatsD → Prometheus → Grafana
     ↓
Node Exporter → Prometheus → Grafana
```

### Logs Flow

```
Application Logs → Filebeat → Logstash → Elasticsearch → Kibana
```

### Tracing Flow

```
Application → Jaeger Collector → Jaeger Storage → Jaeger UI
```

## Configuration

### Prometheus

- **Config**: `prometheus/prometheus.yml`
- **Rules**: `prometheus/rules/arcus-g3-alerts.yml`
- **Scrape Targets**: G3Proxy, G3StatsD, Node Exporter, Redis, Elasticsearch

### Grafana

- **Datasources**: `grafana/provisioning/datasources/`
- **Dashboards**: `grafana/dashboards/`
- **Provisioning**: `grafana/provisioning/dashboards/dashboard.yml`

### Logstash

- **Pipeline**: `logstash/pipeline/arcus-g3.conf`
- **Config**: `logstash/config/logstash.yml`
- **Inputs**: Beats, TCP, UDP
- **Outputs**: Elasticsearch, stdout

### Filebeat

- **Config**: `filebeat/filebeat.yml`
- **Inputs**: Docker containers, system logs, application logs
- **Output**: Logstash

## Kubernetes Deployment

### Prerequisites

- Kubernetes cluster
- kubectl configured

### Deploy

```bash
kubectl apply -f monitoring/k8s/
```

### Verify

```bash
kubectl get pods -n monitoring
kubectl get svc -n monitoring
```

### Port Forward

```bash
# Grafana
kubectl port-forward -n monitoring svc/grafana 3000:3000

# Prometheus
kubectl port-forward -n monitoring svc/prometheus 9090:9090

# Jaeger
kubectl port-forward -n monitoring svc/jaeger 16686:16686

# Kibana
kubectl port-forward -n monitoring svc/kibana 5601:5601
```

## Monitoring Features

### Metrics

- **System Metrics**: CPU, Memory, Disk, Network
- **Application Metrics**: Request rate, Response time, Error rate
- **Business Metrics**: Tenant-specific metrics, User activity
- **Custom Metrics**: G3Proxy-specific metrics

### Alerts

- High CPU usage (>80%)
- High memory usage (>85%)
- Low disk space (>90%)
- Service down
- High error rate (>10%)
- High response time (>1s)
- Redis connection issues
- Elasticsearch cluster health
- Jaeger collector errors
- G3Proxy connection issues
- Tenant isolation violations

### Dashboards

- **Arcus-G3 Overview**: System health, performance metrics
- **G3Proxy Metrics**: Proxy-specific metrics and performance
- **Tenant Metrics**: Per-tenant usage and performance
- **Security Metrics**: Security events and violations
- **Infrastructure Metrics**: System resources and health

### Logs

- **Structured Logging**: JSON format with tenant context
- **Log Levels**: DEBUG, INFO, WARN, ERROR, FATAL
- **Log Categories**: Application, Security, Audit, Performance
- **Log Context**: Tenant ID, User ID, Request ID, Session ID

### Tracing

- **Distributed Tracing**: Request flow across services
- **Span Attributes**: Tenant context, user context, performance data
- **Trace Correlation**: Link traces with logs and metrics
- **Performance Analysis**: Identify bottlenecks and issues

## Customization

### Adding New Metrics

1. **Update Prometheus config** (`prometheus/prometheus.yml`)
2. **Add scrape targets** for new services
3. **Create alert rules** (`prometheus/rules/`)
4. **Update Grafana dashboards** (`grafana/dashboards/`)

### Adding New Log Sources

1. **Update Filebeat config** (`filebeat/filebeat.yml`)
2. **Add new input sources**
3. **Update Logstash pipeline** (`logstash/pipeline/arcus-g3.conf`)
4. **Add log parsing rules**

### Adding New Dashboards

1. **Create dashboard JSON** in `grafana/dashboards/`
2. **Update dashboard provisioning** (`grafana/provisioning/dashboards/`)
3. **Restart Grafana** to load new dashboards

## Troubleshooting

### Common Issues

1. **Services not starting:**
   ```bash
   docker-compose logs -f [service-name]
   ```

2. **Port conflicts:**
   ```bash
   # Check port usage
   lsof -i :9090
   lsof -i :3000
   ```

3. **Memory issues:**
   ```bash
   # Increase Docker memory limit
   # Or reduce service memory requirements
   ```

4. **Elasticsearch cluster issues:**
   ```bash
   # Check cluster health
   curl http://localhost:9200/_cluster/health
   ```

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

## Maintenance

### Backup

```bash
# Backup Prometheus data
docker cp monitoring_prometheus_1:/prometheus ./backup/prometheus

# Backup Grafana data
docker cp monitoring_grafana_1:/var/lib/grafana ./backup/grafana

# Backup Elasticsearch data
docker cp monitoring_elasticsearch_1:/usr/share/elasticsearch/data ./backup/elasticsearch
```

### Updates

```bash
# Update services
cd monitoring
docker-compose pull
docker-compose up -d
```

### Cleanup

```bash
# Stop and remove services
cd monitoring
docker-compose down -v

# Remove volumes
docker volume prune
```

## Security Considerations

- Change default passwords
- Enable TLS/SSL for production
- Configure firewall rules
- Use secrets management
- Enable authentication
- Regular security updates

## Performance Tuning

### Prometheus

- Adjust scrape intervals
- Configure retention policies
- Optimize query performance
- Use recording rules

### Elasticsearch

- Tune JVM heap size
- Configure shard allocation
- Optimize index settings
- Use index templates

### Grafana

- Configure caching
- Optimize dashboard queries
- Use data source proxies
- Enable query caching

## Contributing

1. Follow the existing configuration patterns
2. Test changes in development environment
3. Update documentation
4. Submit pull requests

## Support

For issues and questions:
- Check the troubleshooting section
- Review service logs
- Create GitHub issues
- Contact the development team
