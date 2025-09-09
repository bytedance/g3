#!/bin/bash

# Arcus-G3 Monitoring Setup Script
# This script sets up the complete monitoring and observability infrastructure

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

# Check if Docker is running
check_docker() {
    print_status "Checking Docker status..."
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
    print_success "Docker is running"
}

# Check if kubectl is available
check_kubectl() {
    print_status "Checking kubectl availability..."
    if ! command -v kubectl &> /dev/null; then
        print_warning "kubectl not found. Kubernetes deployment will be skipped."
        return 1
    fi
    print_success "kubectl is available"
    return 0
}

# Create monitoring directories
create_directories() {
    print_status "Creating monitoring directories..."
    mkdir -p monitoring/{prometheus/rules,grafana/{provisioning/{datasources,dashboards},dashboards},logstash/{pipeline,config},filebeat,k8s}
    print_success "Directories created"
}

# Start Docker Compose services
start_docker_compose() {
    print_status "Starting Docker Compose services..."
    cd monitoring
    docker-compose up -d
    print_success "Docker Compose services started"
    cd ..
}

# Deploy to Kubernetes
deploy_k8s() {
    if check_kubectl; then
        print_status "Deploying to Kubernetes..."
        kubectl apply -f monitoring/k8s/
        print_success "Kubernetes deployment completed"
    else
        print_warning "Skipping Kubernetes deployment"
    fi
}

# Wait for services to be ready
wait_for_services() {
    print_status "Waiting for services to be ready..."
    
    # Wait for Prometheus
    print_status "Waiting for Prometheus..."
    timeout 60 bash -c 'until curl -s http://localhost:9090/-/healthy > /dev/null; do sleep 2; done'
    print_success "Prometheus is ready"
    
    # Wait for Grafana
    print_status "Waiting for Grafana..."
    timeout 60 bash -c 'until curl -s http://localhost:3000/api/health > /dev/null; do sleep 2; done'
    print_success "Grafana is ready"
    
    # Wait for Jaeger
    print_status "Waiting for Jaeger..."
    timeout 60 bash -c 'until curl -s http://localhost:16686 > /dev/null; do sleep 2; done'
    print_success "Jaeger is ready"
    
    # Wait for Elasticsearch
    print_status "Waiting for Elasticsearch..."
    timeout 60 bash -c 'until curl -s http://localhost:9200/_cluster/health > /dev/null; do sleep 2; done'
    print_success "Elasticsearch is ready"
    
    # Wait for Kibana
    print_status "Waiting for Kibana..."
    timeout 60 bash -c 'until curl -s http://localhost:5601/api/status > /dev/null; do sleep 2; done'
    print_success "Kibana is ready"
}

# Display service URLs
show_service_urls() {
    print_success "Monitoring services are ready!"
    echo ""
    echo "Service URLs:"
    echo "============="
    echo "Prometheus:     http://localhost:9090"
    echo "Grafana:        http://localhost:3000 (admin/admin)"
    echo "Jaeger:         http://localhost:16686"
    echo "Elasticsearch:  http://localhost:9200"
    echo "Kibana:         http://localhost:5601"
    echo ""
    echo "Docker Compose commands:"
    echo "========================"
    echo "Start services:  cd monitoring && docker-compose up -d"
    echo "Stop services:   cd monitoring && docker-compose down"
    echo "View logs:       cd monitoring && docker-compose logs -f"
    echo ""
    echo "Kubernetes commands:"
    echo "==================="
    echo "View pods:       kubectl get pods -n monitoring"
    echo "View services:   kubectl get svc -n monitoring"
    echo "Port forward:    kubectl port-forward -n monitoring svc/grafana 3000:3000"
    echo ""
}

# Main execution
main() {
    echo "Arcus-G3 Monitoring Setup"
    echo "========================="
    echo ""
    
    check_docker
    create_directories
    start_docker_compose
    deploy_k8s
    wait_for_services
    show_service_urls
    
    print_success "Monitoring setup completed successfully!"
}

# Run main function
main "$@"
