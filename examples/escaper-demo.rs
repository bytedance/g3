//! Example demonstrating escaper functionality
//!
//! This example shows how to use the escaper system including
//! direct escapers, proxy escapers, and escaper chains.

use std::time::Duration;

use arcus_g3_core::{
    TenantId, EscaperManager, EscapeContext, ProtocolType,
    direct_escaper::DirectEscaper,
    proxy_escaper::ProxyEscaper,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🏗️  Arcus-G3 Escaper Demo");
    println!("==========================");

    // Create escaper manager
    let manager = std::sync::Arc::new(EscaperManager::new());

    // Create different types of escapers
    let direct_escaper = std::sync::Arc::new(DirectEscaper::new(
        "direct-1".to_string(),
        100,
    ));

    let proxy_escaper = std::sync::Arc::new(ProxyEscaper::new(
        "proxy-1".to_string(),
        90,
        "proxy.example.com:8080".to_string(),
    ));

    let proxy_escaper_with_auth = std::sync::Arc::new(ProxyEscaper::with_auth(
        "proxy-auth".to_string(),
        80,
        "auth-proxy.example.com:8080".to_string(),
        "username".to_string(),
        "password".to_string(),
    ));

    // Add escapers to global chain
    manager.add_global_escaper(direct_escaper).await?;
    manager.add_global_escaper(proxy_escaper).await?;
    manager.add_global_escaper(proxy_escaper_with_auth).await?;

    println!("✅ Added escapers to global chain");

    // Create tenant-specific escapers
    let tenant_id = TenantId::new();
    let tenant_direct_escaper = std::sync::Arc::new(DirectEscaper::new(
        "tenant-direct".to_string(),
        150,
    ));

    let tenant_proxy_escaper = std::sync::Arc::new(ProxyEscaper::new(
        "tenant-proxy".to_string(),
        140,
        "tenant-proxy.example.com:8080".to_string(),
    ));

    // Add escapers to tenant chain
    manager.add_tenant_escaper(&tenant_id, tenant_direct_escaper).await?;
    manager.add_tenant_escaper(&tenant_id, tenant_proxy_escaper).await?;

    println!("✅ Added escapers to tenant {} chain", tenant_id);

    // Demonstrate different escape scenarios
    demonstrate_escape_scenarios(&manager, &tenant_id).await?;

    // Demonstrate escaper management
    demonstrate_escaper_management(&manager).await?;

    // Demonstrate statistics
    demonstrate_statistics(&manager, &tenant_id).await?;

    println!("\n🎉 Escaper demo completed successfully!");

    Ok(())
}

/// Demonstrate different escape scenarios
async fn demonstrate_escape_scenarios(
    manager: &EscaperManager,
    tenant_id: &TenantId,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📡 Demonstrating escape scenarios...");

    // Test HTTP escape
    let http_context = EscapeContext::new(
        tenant_id.clone(),
        "req-http-1".to_string(),
        "127.0.0.1:8080".to_string(),
        "httpbin.org:80".to_string(),
    ).with_protocol(ProtocolType::Http)
     .with_header("User-Agent".to_string(), "Arcus-G3-Demo/1.0".to_string())
     .with_timeout(Duration::from_secs(10));

    let result = manager.escape(&http_context).await?;
    println!("🌐 HTTP Escape: Status={}, Success={}, Duration={}ms",
        result.status_code, result.success, result.duration.as_millis());

    // Test HTTPS escape
    let https_context = EscapeContext::new(
        tenant_id.clone(),
        "req-https-1".to_string(),
        "127.0.0.1:8080".to_string(),
        "httpbin.org:443".to_string(),
    ).with_protocol(ProtocolType::Https)
     .with_header("User-Agent".to_string(), "Arcus-G3-Demo/1.0".to_string())
     .with_timeout(Duration::from_secs(15));

    let result = manager.escape(&https_context).await?;
    println!("🔒 HTTPS Escape: Status={}, Success={}, Duration={}ms",
        result.status_code, result.success, result.duration.as_millis());

    // Test SOCKS escape
    let socks_context = EscapeContext::new(
        tenant_id.clone(),
        "req-socks-1".to_string(),
        "127.0.0.1:8080".to_string(),
        "example.com:22".to_string(),
    ).with_protocol(ProtocolType::Socks5)
     .with_timeout(Duration::from_secs(5));

    let result = manager.escape(&socks_context).await?;
    println!("🧦 SOCKS5 Escape: Status={}, Success={}, Duration={}ms",
        result.status_code, result.success, result.duration.as_millis());

    // Test TCP escape
    let tcp_context = EscapeContext::new(
        tenant_id.clone(),
        "req-tcp-1".to_string(),
        "127.0.0.1:8080".to_string(),
        "example.com:22".to_string(),
    ).with_protocol(ProtocolType::Tcp)
     .with_timeout(Duration::from_secs(5));

    let result = manager.escape(&tcp_context).await?;
    println!("🔌 TCP Escape: Status={}, Success={}, Duration={}ms",
        result.status_code, result.success, result.duration.as_millis());

    // Test WebSocket escape
    let ws_context = EscapeContext::new(
        tenant_id.clone(),
        "req-ws-1".to_string(),
        "127.0.0.1:8080".to_string(),
        "echo.websocket.org:80".to_string(),
    ).with_protocol(ProtocolType::WebSocket)
     .with_header("Upgrade".to_string(), "websocket".to_string())
     .with_header("Connection".to_string(), "Upgrade".to_string())
     .with_timeout(Duration::from_secs(10));

    let result = manager.escape(&ws_context).await?;
    println!("🔌 WebSocket Escape: Status={}, Success={}, Duration={}ms",
        result.status_code, result.success, result.duration.as_millis());

    Ok(())
}

/// Demonstrate escaper management operations
async fn demonstrate_escaper_management(
    manager: &EscaperManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔧 Demonstrating escaper management...");

    // Get global escapers
    let global_escapers = manager.get_global_escapers().await;
    println!("📋 Global escapers: {}", global_escapers.len());
    for escaper in &global_escapers {
        println!("   - {} ({}): Priority={}, Type={:?}",
            escaper.name(),
            if escaper.is_healthy().await { "Healthy" } else { "Unhealthy" },
            escaper.priority(),
            escaper.escaper_type()
        );
    }

    // Get health status
    let health_status = manager.get_health_status().await;
    println!("🏥 Health status:");
    for (name, healthy) in &health_status {
        println!("   - {}: {}", name, if *healthy { "Healthy" } else { "Unhealthy" });
    }

    // Test escaper removal
    let removed = manager.remove_global_escaper("direct-1").await?;
    if removed.is_some() {
        println!("🗑️  Removed escaper: direct-1");
    }

    // Test escaper lookup
    let escaper = manager.get_escaper_by_name("proxy-1").await;
    if let Some(escaper) = escaper {
        println!("🔍 Found escaper: {} (Type: {:?})", escaper.name(), escaper.escaper_type());
    }

    // Test escaper filtering by type
    let direct_escapers = manager.get_escapers_by_type(&arcus_g3_core::escaper::EscaperType::Direct).await;
    println!("🎯 Direct escapers: {}", direct_escapers.len());

    let proxy_escapers = manager.get_escapers_by_type(&arcus_g3_core::escaper::EscaperType::Proxy).await;
    println!("🎯 Proxy escapers: {}", proxy_escapers.len());

    Ok(())
}

/// Demonstrate statistics and monitoring
async fn demonstrate_statistics(
    manager: &EscaperManager,
    tenant_id: &TenantId,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Demonstrating statistics and monitoring...");

    // Get manager statistics
    let manager_stats = manager.get_stats().await;
    println!("📈 Manager Statistics:");
    println!("   - Total attempts: {}", manager_stats.total_attempts);
    println!("   - Successful escapes: {}", manager_stats.successful_escapes);
    println!("   - Failed escapes: {}", manager_stats.failed_escapes);
    println!("   - Average escape time: {:.2}ms", manager_stats.avg_escape_time_ms);
    println!("   - Tenant count: {}", manager_stats.tenant_count);
    println!("   - Total escapers: {}", manager_stats.total_escapers);

    // Get global chain statistics
    let global_stats = manager.get_global_stats().await;
    println!("🌐 Global Chain Statistics:");
    println!("   - Total attempts: {}", global_stats.total_attempts);
    println!("   - Successful escapes: {}", global_stats.successful_escapes);
    println!("   - Failed escapes: {}", global_stats.failed_escapes);
    println!("   - Success rate: {:.2}%", global_stats.success_rate());
    println!("   - Average escape time: {:.2}ms", global_stats.avg_escape_time_ms);

    // Get tenant statistics
    if let Some(tenant_stats) = manager.get_tenant_stats(tenant_id).await {
        println!("🏢 Tenant {} Statistics:", tenant_id);
        println!("   - Total attempts: {}", tenant_stats.total_attempts);
        println!("   - Successful escapes: {}", tenant_stats.successful_escapes);
        println!("   - Failed escapes: {}", tenant_stats.failed_escapes);
        println!("   - Success rate: {:.2}%", tenant_stats.success_rate());
        println!("   - Average escape time: {:.2}ms", tenant_stats.avg_escape_time_ms);
    }

    // Demonstrate escaper-specific statistics
    let global_escapers = manager.get_global_escapers().await;
    for escaper in &global_escapers {
        let stats = escaper.get_stats().await;
        println!("🔧 {} Statistics:", escaper.name());
        println!("   - Total attempts: {}", stats.total_attempts);
        println!("   - Successful escapes: {}", stats.successful_escapes);
        println!("   - Failed escapes: {}", stats.failed_escapes);
        println!("   - Success rate: {:.2}%", stats.success_rate());
        println!("   - Average escape time: {:.2}ms", stats.avg_escape_time_ms);
        println!("   - Total bytes processed: {}", stats.total_bytes_processed);
    }

    // Demonstrate statistics reset
    println!("🔄 Resetting statistics...");
    manager.reset_all_stats().await?;
    
    let reset_stats = manager.get_stats().await;
    println!("📊 After reset - Total attempts: {}", reset_stats.total_attempts);

    Ok(())
}
