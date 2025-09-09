//! Tenant Identification Demo for Arcus-G3 Multi-Tenant SWG
//!
//! This example demonstrates how to use the TenantIdentificationManager
//! for identifying tenants in the Arcus-G3 Multi-Tenant Secure Web Gateway.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use tokio::time::sleep;

use arcus_g3_core::{
    TenantId,
    tenant_identification::{
        TenantIdentificationRequest, TenantIdentificationMethod,
        IpRange, IpRangeConfig, SsoHeaderConfig, DomainConfig,
    },
    tenant_identification_manager::TenantIdentificationManager,
    tenant_identification_builder::{
        TenantIdentificationBuilder, PredefinedIdentificationConfigs, IpRangeHelpers,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Arcus-G3 Tenant Identification Demo");
    println!("=====================================");

    // Create tenant identification manager
    let manager = TenantIdentificationManager::new();
    manager.initialize().await?;

    // Create some example tenants
    let tenant1 = TenantId::new();
    let tenant2 = TenantId::new();
    let tenant3 = TenantId::new();

    println!("\n📋 Created tenants:");
    println!("  Tenant 1: {}", tenant1);
    println!("  Tenant 2: {}", tenant2);
    println!("  Tenant 3: {}", tenant3);

    // Configure tenant identification methods
    println!("\n🔧 Configuring tenant identification methods...");

    // Tenant 1: IP range identification
    let tenant1_ranges = vec![
        IpRange::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 255)),
        ),
        IpRange::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(10, 255, 255, 255)),
        ),
    ];
    manager.add_ip_range_identification(tenant1.clone(), tenant1_ranges, 100).await?;
    println!("  ✅ Tenant 1: IP range identification configured");

    // Tenant 2: SSO header identification
    manager.add_sso_header_identification(
        tenant2.clone(),
        "X-Tenant-ID".to_string(),
        vec![tenant2.to_string()],
        false,
        100,
    ).await?;
    println!("  ✅ Tenant 2: SSO header identification configured");

    // Tenant 3: Domain identification
    manager.add_domain_identification(
        tenant3.clone(),
        vec!["tenant3.example.com".to_string(), "*.tenant3.example.com".to_string()],
        true, // Use wildcard matching
        false,
        100,
    ).await?;
    println!("  ✅ Tenant 3: Domain identification configured");

    // Set default tenant
    manager.set_default_tenant(tenant1.clone()).await?;
    println!("  ✅ Default tenant set to Tenant 1");

    // Test tenant identification
    println!("\n🧪 Testing tenant identification...");

    // Test 1: IP range identification
    println!("\n📍 Test 1: IP range identification");
    let mut headers = HashMap::new();
    let request1 = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))),
        headers: headers.clone(),
        domain: None,
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    if let Some(result) = manager.identify_tenant(&request1).await? {
        println!("  ✅ Identified tenant: {} (method: {:?}, confidence: {:.2})", 
            result.tenant_id, result.method, result.confidence);
    } else {
        println!("  ❌ Failed to identify tenant");
    }

    // Test 2: SSO header identification
    println!("\n🔐 Test 2: SSO header identification");
    headers.insert("X-Tenant-ID".to_string(), tenant2.to_string());
    let request2 = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))), // External IP
        headers,
        domain: None,
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    if let Some(result) = manager.identify_tenant(&request2).await? {
        println!("  ✅ Identified tenant: {} (method: {:?}, confidence: {:.2})", 
            result.tenant_id, result.method, result.confidence);
    } else {
        println!("  ❌ Failed to identify tenant");
    }

    // Test 3: Domain identification
    println!("\n🌐 Test 3: Domain identification");
    let mut headers = HashMap::new();
    let request3 = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))), // External IP
        headers,
        domain: Some("api.tenant3.example.com".to_string()),
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    if let Some(result) = manager.identify_tenant(&request3).await? {
        println!("  ✅ Identified tenant: {} (method: {:?}, confidence: {:.2})", 
            result.tenant_id, result.method, result.confidence);
    } else {
        println!("  ❌ Failed to identify tenant");
    }

    // Test 4: Fallback to default tenant
    println!("\n🔄 Test 4: Fallback to default tenant");
    let mut headers = HashMap::new();
    let request4 = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))), // External IP
        headers,
        domain: Some("unknown.example.com".to_string()),
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    if let Some(result) = manager.identify_tenant(&request4).await? {
        println!("  ✅ Identified tenant: {} (method: {:?}, confidence: {:.2})", 
            result.tenant_id, result.method, result.confidence);
    } else {
        println!("  ❌ Failed to identify tenant");
    }

    // Test 5: Multiple identification methods for same tenant
    println!("\n🔀 Test 5: Multiple identification methods");
    
    // Add additional methods for tenant 1
    manager.add_sso_header_identification(
        tenant1.clone(),
        "X-User-Tenant".to_string(),
        vec![tenant1.to_string()],
        false,
        90,
    ).await?;
    
    manager.add_domain_identification(
        tenant1.clone(),
        vec!["tenant1.example.com".to_string()],
        false,
        false,
        80,
    ).await?;

    // Test with SSO header
    let mut headers = HashMap::new();
    headers.insert("X-User-Tenant".to_string(), tenant1.to_string());
    let request5a = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))), // External IP
        headers,
        domain: None,
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    if let Some(result) = manager.identify_tenant(&request5a).await? {
        println!("  ✅ Identified tenant: {} (method: {:?}, confidence: {:.2})", 
            result.tenant_id, result.method, result.confidence);
    } else {
        println!("  ❌ Failed to identify tenant");
    }

    // Test with domain
    let mut headers = HashMap::new();
    let request5b = TenantIdentificationRequest {
        client_ip: Some(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))), // External IP
        headers,
        domain: Some("tenant1.example.com".to_string()),
        query_params: HashMap::new(),
        cert_subject: None,
        cert_issuer: None,
    };

    if let Some(result) = manager.identify_tenant(&request5b).await? {
        println!("  ✅ Identified tenant: {} (method: {:?}, confidence: {:.2})", 
            result.tenant_id, result.method, result.confidence);
    } else {
        println!("  ❌ Failed to identify tenant");
    }

    // Test predefined configurations
    println!("\n📦 Testing predefined configurations...");
    
    let tenant4 = TenantId::new();
    let corporate_config = PredefinedIdentificationConfigs::corporate_ip_range(tenant4.clone());
    println!("  ✅ Corporate IP range config created for tenant {}", tenant4);
    
    let tenant5 = TenantId::new();
    let web_config = PredefinedIdentificationConfigs::web_application_domain(
        tenant5.clone(),
        vec!["app.example.com".to_string(), "*.app.example.com".to_string()],
    );
    println!("  ✅ Web application domain config created for tenant {}", tenant5);
    
    let tenant6 = TenantId::new();
    let sso_config = PredefinedIdentificationConfigs::enterprise_sso_header(
        tenant6.clone(),
        "X-Enterprise-Tenant".to_string(),
        vec![tenant6.to_string()],
    );
    println!("  ✅ Enterprise SSO config created for tenant {}", tenant6);

    // Test IP range helpers
    println!("\n🛠️  Testing IP range helpers...");
    
    let private_ranges = IpRangeHelpers::private_ipv4_range();
    println!("  ✅ Private IPv4 ranges: {} ranges", private_ranges.len());
    
    let localhost_ranges = IpRangeHelpers::localhost_range();
    println!("  ✅ Localhost ranges: {} ranges", localhost_ranges.len());
    
    let cidr_ranges = IpRangeHelpers::from_cidr_list(vec!["192.168.1.0/24", "10.0.0.0/8"])?;
    println!("  ✅ CIDR ranges: {} ranges", cidr_ranges.len());

    // Test builder pattern
    println!("\n🔨 Testing builder pattern...");
    
    let tenant7 = TenantId::new();
    let custom_config = TenantIdentificationBuilder::new(tenant7.clone())
        .add_ip_range_addresses(
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(172, 16, 255, 255)),
            100,
        )
        .add_sso_header(
            "X-Custom-Tenant".to_string(),
            vec![tenant7.to_string()],
            false,
            90,
        )
        .add_domain(
            vec!["custom.example.com".to_string()],
            true,
            false,
            80,
        )
        .build();
    
    println!("  ✅ Custom config created with {} methods for tenant {}", 
        custom_config.methods.len(), tenant7);

    // Display identification statistics
    println!("\n📊 Identification Statistics:");
    let stats = manager.get_stats().await;
    println!("  Total attempts: {}", stats.total_attempts);
    println!("  Successful identifications: {}", stats.successful_identifications);
    println!("  Failed identifications: {}", stats.failed_identifications);
    println!("  Average identification time: {:.2}μs", stats.avg_identification_time_us);
    
    println!("\n  Method counts:");
    for (method, count) in &stats.method_counts {
        println!("    {}: {}", method, count);
    }

    // Test configuration validation
    println!("\n✅ Testing configuration validation...");
    manager.validate_configuration().await?;
    println!("  ✅ Configuration validation passed");

    // Test tenant method retrieval
    println!("\n🔍 Testing tenant method retrieval...");
    
    if let Some(methods) = manager.get_tenant_methods(&tenant1).await {
        println!("  ✅ Tenant 1 has {} identification methods", methods.len());
    } else {
        println!("  ❌ No methods found for Tenant 1");
    }

    let global_methods = manager.get_global_methods().await;
    println!("  ✅ Global methods: {} methods", global_methods.len());

    // Test method removal
    println!("\n🗑️  Testing method removal...");
    
    if let Some(methods) = manager.get_tenant_methods(&tenant1).await {
        if let Some(method) = methods.first() {
            manager.remove_tenant_method(&tenant1, method).await?;
            println!("  ✅ Removed method from Tenant 1");
        }
    }

    // Test tenant method clearing
    manager.clear_tenant_methods(&tenant2).await?;
    println!("  ✅ Cleared all methods for Tenant 2");

    // Test statistics reset
    manager.reset_stats().await?;
    println!("  ✅ Statistics reset");

    println!("\n🎉 Tenant Identification Demo completed successfully!");
    println!("\nKey Features Demonstrated:");
    println!("  ✅ Multiple identification methods (IP range, SSO header, domain)");
    println!("  ✅ Fallback to default tenant");
    println!("  ✅ Multiple methods per tenant");
    println!("  ✅ Predefined configurations");
    println!("  ✅ Builder pattern for custom configurations");
    println!("  ✅ IP range helpers and CIDR parsing");
    println!("  ✅ Configuration validation");
    println!("  ✅ Statistics and monitoring");
    println!("  ✅ Method management (add/remove/clear)");

    Ok(())
}
