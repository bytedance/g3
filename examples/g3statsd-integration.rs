//! G3StatsD integration example for Arcus-G3 SWG
//!
//! This example demonstrates how to use the G3StatsD integration
//! for multi-tenant metrics collection in Arcus-G3.

use std::time::Duration;
use tokio::time::sleep;
use anyhow::Result;

use arcus_g3_core::TenantId;
use arcus_g3_metrics::g3statsd_service::G3StatsDService;
use arcus_g3_metrics::g3statsd_config::{G3StatsDConfig, G3StatsDTenantConfig, G3StatsDImporterConfig, G3StatsDCollectorConfig, G3StatsDExporterConfig, G3StatsDExporterDestination};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create G3StatsD service
    let mut g3statsd_service = G3StatsDService::new();

    // Load configuration
    g3statsd_service.load_config("monitoring/g3statsd-arcus-g3.yaml").await?;

    // Start the service
    g3statsd_service.start().await?;

    // Create some example tenants
    let tenant1 = TenantId::new_v4();
    let tenant2 = TenantId::new_v4();

    println!("Created tenants:");
    println!("  Tenant 1: {}", tenant1);
    println!("  Tenant 2: {}", tenant2);

    // Create collectors for tenants
    let collector1 = g3statsd_service.create_tenant_collector(tenant1).await?;
    let collector2 = g3statsd_service.create_tenant_collector(tenant2).await?;

    println!("Created collectors for both tenants");

    // Simulate some metrics collection
    for i in 0..10 {
        // Add metrics for tenant 1
        g3statsd_service.add_counter(&tenant1, "http_requests", 1.0).await?;
        g3statsd_service.add_gauge(&tenant1, "active_connections", (i * 10) as f64).await?;
        g3statsd_service.add_histogram(&tenant1, "response_time", (i * 10.5) as f64).await?;
        g3statsd_service.add_timer(&tenant1, "processing_time", (i * 5.2) as f64).await?;

        // Add metrics for tenant 2
        g3statsd_service.add_counter(&tenant2, "http_requests", 2.0).await?;
        g3statsd_service.add_gauge(&tenant2, "active_connections", (i * 15) as f64).await?;
        g3statsd_service.add_histogram(&tenant2, "response_time", (i * 12.3) as f64).await?;
        g3statsd_service.add_timer(&tenant2, "processing_time", (i * 7.8) as f64).await?;

        println!("Added metrics batch {}", i + 1);

        // Wait a bit between batches
        sleep(Duration::from_secs(1)).await;
    }

    // Get service statistics
    let stats = g3statsd_service.get_service_stats().await;
    println!("Service statistics:");
    println!("  Running: {}", stats.is_running);
    println!("  Tenant count: {}", stats.tenant_count);
    println!("  Total collectors: {}", stats.total_collectors);
    println!("  Export task running: {}", stats.export_task_running);

    // Get metrics for tenant 1
    if let Some(metrics) = g3statsd_service.get_tenant_metrics(&tenant1).await? {
        println!("Metrics for tenant 1:");
        println!("  Counters: {:?}", metrics.counters);
        println!("  Gauges: {:?}", metrics.gauges);
        println!("  Histograms: {:?}", metrics.histograms);
        println!("  Timers: {:?}", metrics.timers);
    }

    // Wait for some exports to happen
    println!("Waiting for metrics export...");
    sleep(Duration::from_secs(15)).await;

    // Stop the service
    g3statsd_service.stop().await?;
    println!("G3StatsD service stopped");

    Ok(())
}
