//! Example demonstrating tenant isolation mechanisms
//!
//! This example shows how to use the tenant isolation system including
//! resource limits, monitoring, and enforcement.

use std::time::Duration;
use std::collections::HashMap;

use arcus_g3_core::{
    TenantId, TenantIsolationManager, TenantResourceMonitor, ResourceMonitorConfig,
    ResourceMonitorEvent, ViolationSeverity, ResourceType,
    tenant_isolation::{TenantConfig, TenantResourceUsage, TenantResourceLimits},
    tenant_isolation_builder::{TenantIsolationBuilder, PredefinedTenantConfigs},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🏗️  Arcus-G3 Tenant Isolation Demo");
    println!("=====================================");

    // Create tenant isolation manager
    let isolation_manager = std::sync::Arc::new(TenantIsolationManager::new(Duration::from_secs(30)));

    // Create different types of tenants
    let tenants = create_sample_tenants().await?;

    // Add tenants to isolation manager
    for (tenant_id, config) in tenants {
        isolation_manager.add_tenant(config).await?;
        println!("✅ Added tenant: {}", tenant_id);
    }

    // Create resource monitor
    let mut monitor = create_resource_monitor(isolation_manager.clone()).await?;

    // Start monitoring
    monitor.start().await?;
    println!("🔍 Started resource monitoring");

    // Simulate resource usage
    simulate_resource_usage(&isolation_manager).await?;

    // Wait for monitoring to detect violations
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check for violations
    check_tenant_violations(&isolation_manager).await?;

    // Demonstrate tenant management
    demonstrate_tenant_management(&isolation_manager).await?;

    // Stop monitoring
    monitor.stop().await?;
    println!("🛑 Stopped resource monitoring");

    println!("\n🎉 Tenant isolation demo completed successfully!");

    Ok(())
}

/// Create sample tenants with different configurations
async fn create_sample_tenants() -> Result<Vec<(TenantId, TenantConfig)>, Box<dyn std::error::Error>> {
    let mut tenants = Vec::new();

    // Development tenant
    let dev_tenant_id = TenantId::new();
    let dev_config = PredefinedTenantConfigs::development_tenant(
        dev_tenant_id.clone(),
        "Development Tenant".to_string(),
    );
    tenants.push((dev_tenant_id, dev_config));

    // Production tenant
    let prod_tenant_id = TenantId::new();
    let prod_config = PredefinedTenantConfigs::production_tenant(
        prod_tenant_id.clone(),
        "Production Tenant".to_string(),
    );
    tenants.push((prod_tenant_id, prod_config));

    // Enterprise tenant
    let ent_tenant_id = TenantId::new();
    let ent_config = PredefinedTenantConfigs::enterprise_tenant(
        ent_tenant_id.clone(),
        "Enterprise Tenant".to_string(),
    );
    tenants.push((ent_tenant_id, ent_config));

    // High security tenant
    let sec_tenant_id = TenantId::new();
    let sec_config = PredefinedTenantConfigs::high_security_tenant(
        sec_tenant_id.clone(),
        "High Security Tenant".to_string(),
    );
    tenants.push((sec_tenant_id, sec_config));

    // Resource constrained tenant
    let constrained_tenant_id = TenantId::new();
    let constrained_config = PredefinedTenantConfigs::resource_constrained_tenant(
        constrained_tenant_id.clone(),
        "Resource Constrained Tenant".to_string(),
    );
    tenants.push((constrained_tenant_id, constrained_config));

    // Custom tenant with specific limits
    let custom_tenant_id = TenantId::new();
    let custom_config = TenantIsolationBuilder::new(custom_tenant_id.clone(), "Custom Tenant".to_string())
        .description("Custom tenant with specific resource limits".to_string())
        .max_connections(200)
        .max_bandwidth_bps(25_000_000) // 25 MB/s
        .max_requests_per_second(200)
        .max_memory_bytes(250_000_000) // 250 MB
        .max_cpu_percentage(25.0)
        .max_servers(5)
        .max_certificates(25)
        .max_log_retention_days(14)
        .max_audit_log_size(25_000_000) // 25 MB
        .setting("custom_feature".to_string(), serde_json::Value::Bool(true))
        .setting("priority".to_string(), serde_json::Value::String("medium".to_string()))
        .build();
    tenants.push((custom_tenant_id, custom_config));

    Ok(tenants)
}

/// Create resource monitor with event callbacks
async fn create_resource_monitor(
    isolation_manager: std::sync::Arc<TenantIsolationManager>,
) -> Result<TenantResourceMonitor, Box<dyn std::error::Error>> {
    let config = ResourceMonitorConfig {
        monitoring_interval: Duration::from_secs(2), // Check every 2 seconds for demo
        auto_enforcement: true,
        alerting_enabled: true,
        max_violations_before_action: 2, // Disable after 2 violations for demo
        ..Default::default()
    };

    let mut monitor = TenantResourceMonitor::new(isolation_manager, config);

    // Add event callback
    monitor.add_callback(|event| {
        match event {
            ResourceMonitorEvent::ThresholdExceeded { tenant_id, resource_type, current_usage, threshold, severity } => {
                println!("⚠️  Threshold exceeded for tenant {}: {} at {:.1}% (threshold: {:.1}%) - Severity: {:?}",
                    tenant_id, format_resource_type(&resource_type), current_usage * 100.0, threshold * 100.0, severity);
            },
            ResourceMonitorEvent::LimitViolated { tenant_id, resource_type, current_value, limit_value, severity } => {
                println!("🚨 Limit violated for tenant {}: {} {} > {} - Severity: {:?}",
                    tenant_id, format_resource_type(&resource_type), current_value, limit_value, severity);
            },
            ResourceMonitorEvent::TenantDisabled { tenant_id, reason, violation_count } => {
                println!("🔴 Tenant {} disabled: {} ({} violations)",
                    tenant_id, reason, violation_count);
            },
            ResourceMonitorEvent::TenantReEnabled { tenant_id } => {
                println!("🟢 Tenant {} re-enabled", tenant_id);
            },
            ResourceMonitorEvent::StatsUpdated { stats } => {
                println!("📊 Stats updated: {} total tenants, {} active, {} with violations, {} total violations",
                    stats.total_tenants, stats.active_tenants, stats.tenants_with_violations, stats.total_violations);
            },
        }
    });

    Ok(monitor)
}

/// Simulate resource usage for different tenants
async fn simulate_resource_usage(
    isolation_manager: &TenantIsolationManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📈 Simulating resource usage...");

    let tenant_configs = isolation_manager.get_all_tenant_configs().await;

    for (tenant_id, config) in tenant_configs {
        println!("\n🔧 Simulating usage for tenant: {} ({})", tenant_id, config.name);

        // Create usage that will trigger different types of violations
        let mut usage = TenantResourceUsage::default();
        usage.last_updated = std::time::SystemTime::now();

        // Simulate different scenarios based on tenant type
        if config.name.contains("Development") {
            // Development tenant - moderate usage
            usage.current_connections = config.resource_limits.max_connections / 2;
            usage.current_bandwidth_bps = config.resource_limits.max_bandwidth_bps / 2;
            usage.current_requests_per_second = config.resource_limits.max_requests_per_second / 2;
            usage.current_memory_bytes = config.resource_limits.max_memory_bytes / 2;
            usage.current_cpu_percentage = config.resource_limits.max_cpu_percentage / 2.0;
            usage.current_servers = config.resource_limits.max_servers / 2;
            usage.current_certificates = config.resource_limits.max_certificates / 2;
            usage.current_audit_log_size = config.resource_limits.max_audit_log_size / 2;
        } else if config.name.contains("Production") {
            // Production tenant - high usage but within limits
            usage.current_connections = (config.resource_limits.max_connections as f32 * 0.8) as u32;
            usage.current_bandwidth_bps = (config.resource_limits.max_bandwidth_bps as f32 * 0.8) as u64;
            usage.current_requests_per_second = (config.resource_limits.max_requests_per_second as f32 * 0.8) as u32;
            usage.current_memory_bytes = (config.resource_limits.max_memory_bytes as f32 * 0.8) as u64;
            usage.current_cpu_percentage = config.resource_limits.max_cpu_percentage * 0.8;
            usage.current_servers = (config.resource_limits.max_servers as f32 * 0.8) as u32;
            usage.current_certificates = (config.resource_limits.max_certificates as f32 * 0.8) as u32;
            usage.current_audit_log_size = (config.resource_limits.max_audit_log_size as f32 * 0.8) as u64;
        } else if config.name.contains("Enterprise") {
            // Enterprise tenant - very high usage
            usage.current_connections = (config.resource_limits.max_connections as f32 * 0.9) as u32;
            usage.current_bandwidth_bps = (config.resource_limits.max_bandwidth_bps as f32 * 0.9) as u64;
            usage.current_requests_per_second = (config.resource_limits.max_requests_per_second as f32 * 0.9) as u32;
            usage.current_memory_bytes = (config.resource_limits.max_memory_bytes as f32 * 0.9) as u64;
            usage.current_cpu_percentage = config.resource_limits.max_cpu_percentage * 0.9;
            usage.current_servers = (config.resource_limits.max_servers as f32 * 0.9) as u32;
            usage.current_certificates = (config.resource_limits.max_certificates as f32 * 0.9) as u32;
            usage.current_audit_log_size = (config.resource_limits.max_audit_log_size as f32 * 0.9) as u64;
        } else if config.name.contains("High Security") {
            // High security tenant - moderate usage
            usage.current_connections = (config.resource_limits.max_connections as f32 * 0.7) as u32;
            usage.current_bandwidth_bps = (config.resource_limits.max_bandwidth_bps as f32 * 0.7) as u64;
            usage.current_requests_per_second = (config.resource_limits.max_requests_per_second as f32 * 0.7) as u32;
            usage.current_memory_bytes = (config.resource_limits.max_memory_bytes as f32 * 0.7) as u64;
            usage.current_cpu_percentage = config.resource_limits.max_cpu_percentage * 0.7;
            usage.current_servers = (config.resource_limits.max_servers as f32 * 0.7) as u32;
            usage.current_certificates = (config.resource_limits.max_certificates as f32 * 0.7) as u32;
            usage.current_audit_log_size = (config.resource_limits.max_audit_log_size as f32 * 0.7) as u64;
        } else if config.name.contains("Resource Constrained") {
            // Resource constrained tenant - exceed limits to trigger violations
            usage.current_connections = config.resource_limits.max_connections + 10; // Exceed limit
            usage.current_bandwidth_bps = config.resource_limits.max_bandwidth_bps + 100_000; // Exceed limit
            usage.current_requests_per_second = config.resource_limits.max_requests_per_second + 5; // Exceed limit
            usage.current_memory_bytes = config.resource_limits.max_memory_bytes + 10_000_000; // Exceed limit
            usage.current_cpu_percentage = config.resource_limits.max_cpu_percentage + 10.0; // Exceed limit
            usage.current_servers = config.resource_limits.max_servers + 1; // Exceed limit
            usage.current_certificates = config.resource_limits.max_certificates + 2; // Exceed limit
            usage.current_audit_log_size = config.resource_limits.max_audit_log_size + 1_000_000; // Exceed limit
        } else if config.name.contains("Custom") {
            // Custom tenant - exceed some limits
            usage.current_connections = config.resource_limits.max_connections + 50; // Exceed limit
            usage.current_bandwidth_bps = (config.resource_limits.max_bandwidth_bps as f32 * 0.8) as u64; // Within limit
            usage.current_requests_per_second = config.resource_limits.max_requests_per_second + 25; // Exceed limit
            usage.current_memory_bytes = (config.resource_limits.max_memory_bytes as f32 * 0.8) as u64; // Within limit
            usage.current_cpu_percentage = config.resource_limits.max_cpu_percentage * 0.8; // Within limit
            usage.current_servers = (config.resource_limits.max_servers as f32 * 0.8) as u32; // Within limit
            usage.current_certificates = (config.resource_limits.max_certificates as f32 * 0.8) as u32; // Within limit
            usage.current_audit_log_size = (config.resource_limits.max_audit_log_size as f32 * 0.8) as u64; // Within limit
        }

        // Update resource usage
        isolation_manager.update_resource_usage(&tenant_id, usage).await?;

        println!("   📊 Updated resource usage for tenant {}", tenant_id);
    }

    Ok(())
}

/// Check for tenant violations
async fn check_tenant_violations(
    isolation_manager: &TenantIsolationManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Checking for tenant violations...");

    let tenant_configs = isolation_manager.get_all_tenant_configs().await;

    for (tenant_id, config) in tenant_configs {
        let violations = isolation_manager.check_resource_violations(&tenant_id).await?;
        
        if violations.is_empty() {
            println!("✅ Tenant {} ({}) - No violations", tenant_id, config.name);
        } else {
            println!("❌ Tenant {} ({}) - {} violations:", tenant_id, config.name, violations.len());
            for violation in &violations {
                println!("   - {}: {} > {} (Severity: {:?})",
                    format_resource_type(&violation.resource_type),
                    violation.current_value,
                    violation.limit_value,
                    violation.severity
                );
            }
        }
    }

    Ok(())
}

/// Demonstrate tenant management operations
async fn demonstrate_tenant_management(
    isolation_manager: &TenantIsolationManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔧 Demonstrating tenant management operations...");

    // Get all tenant configurations
    let tenant_configs = isolation_manager.get_all_tenant_configs().await;
    println!("📋 Total tenants: {}", tenant_configs.len());

    // Get all tenant usage
    let tenant_usage = isolation_manager.get_all_tenant_usage().await;
    println!("📊 Total tenants with usage data: {}", tenant_usage.len());

    // Get isolation statistics
    let stats = isolation_manager.get_stats().await;
    println!("📈 Isolation statistics:");
    println!("   - Total tenants: {}", stats.total_tenants);
    println!("   - Active tenants: {}", stats.active_tenants);
    println!("   - Tenants with violations: {}", stats.tenants_with_violations);
    println!("   - Total violations: {}", stats.total_violations);

    // Demonstrate tenant update
    if let Some((tenant_id, _)) = tenant_configs.iter().next() {
        println!("\n🔄 Updating tenant configuration for: {}", tenant_id);
        
        let updates = arcus_g3_core::tenant_isolation::TenantConfigUpdate {
            name: Some("Updated Tenant Name".to_string()),
            description: Some("Updated description".to_string()),
            resource_limits: None,
            enabled: Some(true),
            settings: None,
        };
        
        isolation_manager.update_tenant(tenant_id, updates).await?;
        println!("✅ Updated tenant configuration");
    }

    Ok(())
}

/// Format resource type for display
fn format_resource_type(resource_type: &ResourceType) -> &'static str {
    match resource_type {
        ResourceType::Connections => "Connections",
        ResourceType::Bandwidth => "Bandwidth",
        ResourceType::RequestsPerSecond => "Requests/sec",
        ResourceType::Memory => "Memory",
        ResourceType::Cpu => "CPU",
        ResourceType::Servers => "Servers",
        ResourceType::Certificates => "Certificates",
        ResourceType::AuditLogSize => "Audit Log Size",
    }
}
