//! G3FCGen integration example for Arcus-G3 SWG
//!
//! This example demonstrates how to use the G3FCGen integration
//! for multi-tenant fake certificate generation in Arcus-G3.

use std::time::Duration;
use tokio::time::sleep;
use anyhow::Result;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;

use arcus_g3_core::TenantId;
use arcus_g3_security::g3fcgen_service::G3FCGenService;
use arcus_g3_security::g3fcgen_config::{G3FCGenConfigFile, G3FCGenGlobalConfig, G3FCGenCAConfig, G3FCGenBackendConfig, G3FCGenStatsConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create G3FCGen service
    let mut g3fcgen_service = G3FCGenService::new();

    // Load configuration
    g3fcgen_service.load_config("security/g3fcgen-arcus-g3.yaml").await?;

    // Start the service
    g3fcgen_service.start().await?;

    // Create some example tenants
    let tenant1 = TenantId::new();
    let tenant2 = TenantId::new();

    println!("Created tenants:");
    println!("  Tenant 1: {}", tenant1);
    println!("  Tenant 2: {}", tenant2);

    // Create a sample CA certificate and key
    // In a real implementation, these would be loaded from files
    let ca_cert = create_sample_ca_cert()?;
    let ca_key = create_sample_ca_key()?;

    // Create generators for tenants
    let generator1 = g3fcgen_service.create_tenant_generator(tenant1.clone(), ca_cert.clone(), ca_key.clone()).await?;
    let generator2 = g3fcgen_service.create_tenant_generator(tenant2.clone(), ca_cert, ca_key).await?;

    println!("Created generators for both tenants");

    // Simulate some certificate generation
    let hostnames = vec![
        "example.com",
        "test.example.com",
        "api.example.com",
        "www.example.com",
        "secure.example.com",
    ];

    for i in 0..5 {
        // Generate certificates for tenant 1
        for hostname in &hostnames {
            let cert = g3fcgen_service.get_certificate(&tenant1, hostname).await?;
            println!("Tenant 1 - Generated certificate for {}: TTL={}s", hostname, cert.ttl);
        }

        // Generate certificates for tenant 2
        for hostname in &hostnames {
            let cert = g3fcgen_service.get_certificate(&tenant2, hostname).await?;
            println!("Tenant 2 - Generated certificate for {}: TTL={}s", hostname, cert.ttl);
        }

        println!("Generated certificates batch {}", i + 1);

        // Wait a bit between batches
        sleep(Duration::from_secs(1)).await;
    }

    // Get service statistics
    let stats = g3fcgen_service.get_service_stats().await;
    println!("Service statistics:");
    println!("  Running: {}", stats.is_running);
    println!("  Tenant count: {}", stats.tenant_count);
    println!("  Total generators: {}", stats.total_generators);
    println!("  Cleanup task running: {}", stats.cleanup_task_running);

    // Validate hostnames
    let test_hostnames = vec![
        "example.com",
        "malicious.com",
        "test.example.com",
    ];

    for hostname in &test_hostnames {
        let valid1 = g3fcgen_service.validate_hostname(&tenant1, hostname).await?;
        let valid2 = g3fcgen_service.validate_hostname(&tenant2, hostname).await?;
        println!("Hostname {} - Tenant 1: {}, Tenant 2: {}", hostname, valid1, valid2);
    }

    // Wait for some cleanup operations
    println!("Waiting for certificate cleanup...");
    sleep(Duration::from_secs(10)).await;

    // Stop the service
    g3fcgen_service.stop().await?;
    println!("G3FCGen service stopped");

    Ok(())
}

/// Create a sample CA certificate for demonstration
fn create_sample_ca_cert() -> Result<X509> {
    // This is a simplified implementation
    // In a real implementation, this would load from a file or generate a proper CA
    tracing::debug!("Creating sample CA certificate");
    
    // For demonstration purposes, we'll create a placeholder
    // In a real implementation, this would be a proper X509 certificate
    Err(anyhow::anyhow!("Sample CA certificate creation not implemented"))
}

/// Create a sample CA private key for demonstration
fn create_sample_ca_key() -> Result<PKey<Private>> {
    // This is a simplified implementation
    // In a real implementation, this would load from a file or generate a proper key
    tracing::debug!("Creating sample CA private key");
    
    // For demonstration purposes, we'll create a placeholder
    // In a real implementation, this would be a proper private key
    Err(anyhow::anyhow!("Sample CA private key creation not implemented"))
}
