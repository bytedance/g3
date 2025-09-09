#!/bin/bash

# Arcus-G3 Monitoring Test Script
# This script tests the monitoring and observability infrastructure

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Test function
test_service() {
    local service_name=$1
    local url=$2
    local expected_status=$3
    
    print_status "Testing $service_name at $url"
    
    if curl -s -o /dev/null -w "%{http_code}" "$url" | grep -q "$expected_status"; then
        print_success "$service_name is responding correctly"
        return 0
    else
        print_error "$service_name is not responding correctly"
        return 1
    fi
}

# Test Prometheus
test_prometheus() {
    print_status "Testing Prometheus..."
    
    # Test health endpoint
    test_service "Prometheus Health" "http://localhost:9090/-/healthy" "200"
    
    # Test metrics endpoint
    test_service "Prometheus Metrics" "http://localhost:9090/metrics" "200"
    
    # Test targets
    print_status "Checking Prometheus targets..."
    local targets=$(curl -s "http://localhost:9090/api/v1/targets" | jq -r '.data.activeTargets[] | select(.health == "up") | .labels.job' | wc -l)
    print_status "Active targets: $targets"
    
    # Test rules
    print_status "Checking Prometheus rules..."
    local rules=$(curl -s "http://localhost:9090/api/v1/rules" | jq -r '.data.groups[].rules[].name' | wc -l)
    print_status "Configured rules: $rules"
}

# Test Grafana
test_grafana() {
    print_status "Testing Grafana..."
    
    # Test health endpoint
    test_service "Grafana Health" "http://localhost:3000/api/health" "200"
    
    # Test login
    print_status "Testing Grafana authentication..."
    local login_response=$(curl -s -X POST "http://localhost:3000/api/auth/login" \
        -H "Content-Type: application/json" \
        -d '{"user":"admin","password":"admin"}' | jq -r '.message')
    
    if [[ "$login_response" == "Logged in" ]]; then
        print_success "Grafana authentication successful"
    else
        print_warning "Grafana authentication failed: $login_response"
    fi
    
    # Test datasources
    print_status "Checking Grafana datasources..."
    local datasources=$(curl -s "http://localhost:3000/api/datasources" | jq -r '.[].name' | wc -l)
    print_status "Configured datasources: $datasources"
    
    # Test dashboards
    print_status "Checking Grafana dashboards..."
    local dashboards=$(curl -s "http://localhost:3000/api/search?type=dash-db" | jq -r '.[].title' | wc -l)
    print_status "Available dashboards: $dashboards"
}

# Test Jaeger
test_jaeger() {
    print_status "Testing Jaeger..."
    
    # Test UI
    test_service "Jaeger UI" "http://localhost:16686" "200"
    
    # Test API
    test_service "Jaeger API" "http://localhost:16686/api/services" "200"
    
    # Test collector
    print_status "Testing Jaeger collector..."
    local collector_response=$(curl -s "http://localhost:14268/api/services" | jq -r '.data[]' | wc -l)
    print_status "Available services: $collector_response"
}

# Test Elasticsearch
test_elasticsearch() {
    print_status "Testing Elasticsearch..."
    
    # Test cluster health
    test_service "Elasticsearch Health" "http://localhost:9200/_cluster/health" "200"
    
    # Test cluster status
    print_status "Checking Elasticsearch cluster status..."
    local cluster_status=$(curl -s "http://localhost:9200/_cluster/health" | jq -r '.status')
    print_status "Cluster status: $cluster_status"
    
    # Test indices
    print_status "Checking Elasticsearch indices..."
    local indices=$(curl -s "http://localhost:9200/_cat/indices?v" | wc -l)
    print_status "Available indices: $((indices - 1))"
}

# Test Kibana
test_kibana() {
    print_status "Testing Kibana..."
    
    # Test status
    test_service "Kibana Status" "http://localhost:5601/api/status" "200"
    
    # Test saved objects
    print_status "Checking Kibana saved objects..."
    local saved_objects=$(curl -s "http://localhost:5601/api/saved_objects/_find?type=index-pattern" | jq -r '.total' 2>/dev/null || echo "0")
    print_status "Saved objects: $saved_objects"
}

# Test Redis
test_redis() {
    print_status "Testing Redis..."
    
    # Test connection
    if redis-cli -h localhost -p 6379 ping | grep -q "PONG"; then
        print_success "Redis is responding"
    else
        print_error "Redis is not responding"
        return 1
    fi
    
    # Test info
    print_status "Checking Redis info..."
    local redis_info=$(redis-cli -h localhost -p 6379 info server | grep "redis_version" | cut -d: -f2 | tr -d '\r')
    print_status "Redis version: $redis_info"
}

# Test metrics collection
test_metrics() {
    print_status "Testing metrics collection..."
    
    # Test Prometheus metrics
    local prometheus_metrics=$(curl -s "http://localhost:9090/metrics" | grep -c "arcus_g3" || echo "0")
    print_status "Arcus-G3 metrics in Prometheus: $prometheus_metrics"
    
    # Test custom metrics
    local custom_metrics=$(curl -s "http://localhost:9090/metrics" | grep -c "arcus_g3_" || echo "0")
    print_status "Custom metrics: $custom_metrics"
    
    # Test metric types
    print_status "Checking metric types..."
    local counters=$(curl -s "http://localhost:9090/metrics" | grep -c "# TYPE.*counter" || echo "0")
    local gauges=$(curl -s "http://localhost:9090/metrics" | grep -c "# TYPE.*gauge" || echo "0")
    local histograms=$(curl -s "http://localhost:9090/metrics" | grep -c "# TYPE.*histogram" || echo "0")
    
    print_status "Counters: $counters, Gauges: $gauges, Histograms: $histograms"
}

# Test log collection
test_logs() {
    print_status "Testing log collection..."
    
    # Test Elasticsearch logs
    local log_count=$(curl -s "http://localhost:9200/arcus-g3-logs-*/_count" | jq -r '.count' 2>/dev/null || echo "0")
    print_status "Logs in Elasticsearch: $log_count"
    
    # Test log indices
    local log_indices=$(curl -s "http://localhost:9200/_cat/indices/arcus-g3-logs-*?v" | wc -l)
    print_status "Log indices: $((log_indices - 1))"
}

# Test alerting
test_alerting() {
    print_status "Testing alerting..."
    
    # Test Prometheus rules
    local rules=$(curl -s "http://localhost:9090/api/v1/rules" | jq -r '.data.groups[].rules[].name' | wc -l)
    print_status "Alert rules: $rules"
    
    # Test alerting rules
    local alerting_rules=$(curl -s "http://localhost:9090/api/v1/rules" | jq -r '.data.groups[].rules[] | select(.type == "alerting") | .name' | wc -l)
    print_status "Alerting rules: $alerting_rules"
}

# Generate test data
generate_test_data() {
    print_status "Generating test data..."
    
    # Generate some test metrics
    for i in {1..10}; do
        curl -s "http://localhost:9090/metrics" > /dev/null
        sleep 1
    done
    
    # Generate some test logs
    for i in {1..5}; do
        echo "{\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"level\":\"INFO\",\"message\":\"Test log message $i\",\"service\":\"arcus-g3\",\"tenant_id\":\"tenant-$i\"}" | \
        curl -s -X POST "http://localhost:5000" -H "Content-Type: application/json" -d @- > /dev/null
    done
    
    print_success "Test data generated"
}

# Main test function
main() {
    echo "Arcus-G3 Monitoring Test Suite"
    echo "=============================="
    echo ""
    
    # Check if services are running
    print_status "Checking if services are running..."
    
    # Test each service
    test_prometheus
    echo ""
    
    test_grafana
    echo ""
    
    test_jaeger
    echo ""
    
    test_elasticsearch
    echo ""
    
    test_kibana
    echo ""
    
    test_redis
    echo ""
    
    # Generate test data
    generate_test_data
    echo ""
    
    # Test metrics and logs
    test_metrics
    echo ""
    
    test_logs
    echo ""
    
    test_alerting
    echo ""
    
    print_success "All tests completed!"
    echo ""
    echo "Service URLs:"
    echo "============="
    echo "Prometheus:     http://localhost:9090"
    echo "Grafana:        http://localhost:3000 (admin/admin)"
    echo "Jaeger:         http://localhost:16686"
    echo "Elasticsearch:  http://localhost:9200"
    echo "Kibana:         http://localhost:5601"
}

# Run main function
main "$@"
