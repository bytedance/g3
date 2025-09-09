//! Server Registry Demo for Arcus-G3 Multi-Tenant SWG
//!
//! This example demonstrates how to use the ServerRegistry and ServerManager
//! for managing multi-tenant servers in the Arcus-G3 Secure Web Gateway.

use std::time::Duration;
use tokio::time::sleep;

use arcus_g3_core::{
    TenantId,
    server_registry::{ServerRegistry, ServerType, ServerStatus},
    server_manager::ServerManager,
    server_builder::{ServerConfigBuilder, PredefinedConfigs},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Arcus-G3 Server Registry Demo");
    println!("=================================");

    // Create server manager
    let mut server_manager = ServerManager::with_config(
        Duration::from_secs(10), // Health check interval
        true,  // Auto-start enabled
        true,  // Health check enabled
    );

    // Initialize the server manager
    server_manager.initialize().await?;

    // Create some example tenants
    let tenant1 = TenantId::new();
    let tenant2 = TenantId::new();
    let tenant3 = TenantId::new();

    println!("\n📋 Created tenants:");
    println!("  Tenant 1: {}", tenant1);
    println!("  Tenant 2: {}", tenant2);
    println!("  Tenant 3: {}", tenant3);

    // Register servers for tenant 1
    println!("\n🔧 Registering servers for Tenant 1...");
    
    let http_config = ServerConfigBuilder::http_proxy(
        "tenant1-http-proxy".to_string(),
        tenant1.clone(),
        8080,
    )
    .max_connections(1000)
    .connection_timeout(Duration::from_secs(30))
    .build();

    let https_config = ServerConfigBuilder::https_proxy(
        "tenant1-https-proxy".to_string(),
        tenant1.clone(),
        8443,
    )
    .tls_config("tenant1-cert.pem".to_string(), "tenant1-key.pem".to_string())
    .max_connections(500)
    .build();

    let socks_config = ServerConfigBuilder::socks_proxy(
        "tenant1-socks-proxy".to_string(),
        tenant1.clone(),
        1080,
    )
    .max_connections(200)
    .build();

    let server1_id = server_manager.register_tenant_server(tenant1.clone(), http_config).await?;
    let server2_id = server_manager.register_tenant_server(tenant1.clone(), https_config).await?;
    let server3_id = server_manager.register_tenant_server(tenant1.clone(), socks_config).await?;

    println!("  ✅ Registered HTTP proxy: {}", server1_id);
    println!("  ✅ Registered HTTPS proxy: {}", server2_id);
    println!("  ✅ Registered SOCKS proxy: {}", server3_id);

    // Register servers for tenant 2 using predefined configurations
    println!("\n🔧 Registering servers for Tenant 2 using predefined configs...");
    
    let standard_http = PredefinedConfigs::standard_http_proxy(tenant2.clone());
    let standard_https = PredefinedConfigs::standard_https_proxy(
        tenant2.clone(),
        "tenant2-cert.pem".to_string(),
        "tenant2-key.pem".to_string(),
    );
    let standard_socks = PredefinedConfigs::standard_socks_proxy(tenant2.clone());

    let server4_id = server_manager.register_tenant_server(tenant2.clone(), standard_http).await?;
    let server5_id = server_manager.register_tenant_server(tenant2.clone(), standard_https).await?;
    let server6_id = server_manager.register_tenant_server(tenant2.clone(), standard_socks).await?;

    println!("  ✅ Registered standard HTTP proxy: {}", server4_id);
    println!("  ✅ Registered standard HTTPS proxy: {}", server5_id);
    println!("  ✅ Registered standard SOCKS proxy: {}", server6_id);

    // Register high-performance server for tenant 3
    println!("\n🔧 Registering high-performance server for Tenant 3...");
    
    let hp_config = PredefinedConfigs::high_performance_http_proxy(tenant3.clone());
    let server7_id = server_manager.register_tenant_server(tenant3.clone(), hp_config).await?;
    println!("  ✅ Registered high-performance HTTP proxy: {}", server7_id);

    // Wait for servers to start (auto-start is enabled)
    println!("\n⏳ Waiting for servers to start...");
    sleep(Duration::from_secs(2)).await;

    // Display server status
    println!("\n📊 Server Status:");
    let all_servers = server_manager.get_all_servers().await;
    for server in all_servers {
        println!("  {} - {}:{} - Status: {:?} - Connections: {}/{}", 
            server.config.name,
            server.config.listen_addr,
            server.config.listen_port,
            server.status,
            server.active_connections,
            server.total_connections
        );
    }

    // Display tenant-specific servers
    println!("\n🏢 Tenant 1 Servers:");
    let tenant1_servers = server_manager.get_tenant_servers(&tenant1).await;
    for server in tenant1_servers {
        println!("  {} - {}:{} - {:?}", 
            server.config.name,
            server.config.listen_addr,
            server.config.listen_port,
            server.status
        );
    }

    // Display servers by type
    println!("\n🌐 HTTP Servers:");
    let http_servers = server_manager.get_servers_by_type(&ServerType::Http).await;
    for server in http_servers {
        println!("  {} - Tenant: {} - Status: {:?}", 
            server.config.name,
            server.config.tenant_id,
            server.status
        );
    }

    // Test server operations
    println!("\n🔄 Testing server operations...");
    
    // Stop a server
    println!("  Stopping server: {}", server1_id);
    server_manager.stop_server(&server1_id).await?;
    sleep(Duration::from_millis(500)).await;
    
    // Check status
    if let Some(server) = server_manager.get_server(&server1_id).await {
        println!("  Server {} status: {:?}", server1_id, server.status);
    }

    // Restart the server
    println!("  Restarting server: {}", server1_id);
    server_manager.restart_server(&server1_id).await?;
    sleep(Duration::from_millis(500)).await;
    
    // Check status again
    if let Some(server) = server_manager.get_server(&server1_id).await {
        println!("  Server {} status: {:?}", server1_id, server.status);
    }

    // Test tenant operations
    println!("\n🏢 Testing tenant operations...");
    
    // Stop all servers for tenant 2
    println!("  Stopping all servers for Tenant 2");
    server_manager.stop_tenant_servers(&tenant2).await?;
    sleep(Duration::from_millis(500)).await;
    
    // Check tenant 2 server statuses
    let tenant2_servers = server_manager.get_tenant_servers(&tenant2).await;
    for server in tenant2_servers {
        println!("  Tenant 2 server {} status: {:?}", server.config.name, server.status);
    }

    // Restart all servers for tenant 2
    println!("  Restarting all servers for Tenant 2");
    server_manager.restart_tenant_servers(&tenant2).await?;
    sleep(Duration::from_millis(500)).await;

    // Display final statistics
    println!("\n📈 Final Statistics:");
    let stats = server_manager.get_stats().await;
    println!("  Total servers: {}", stats.total_servers);
    println!("  Running servers: {}", stats.running_servers);
    println!("  Stopped servers: {}", stats.stopped_servers);
    println!("  Error servers: {}", stats.error_servers);
    println!("  Total tenants: {}", stats.total_tenants);

    // Display tenant statistics
    for tenant_id in [&tenant1, &tenant2, &tenant3] {
        let tenant_stats = server_manager.get_tenant_stats(tenant_id).await;
        println!("\n  Tenant {} Statistics:", tenant_id);
        println!("    Total servers: {}", tenant_stats.total_servers);
        println!("    Running servers: {}", tenant_stats.running_servers);
        println!("    Stopped servers: {}", tenant_stats.stopped_servers);
        println!("    Error servers: {}", tenant_stats.error_servers);
        println!("    Total connections: {}", tenant_stats.total_connections);
        println!("    Active connections: {}", tenant_stats.active_connections);
    }

    // Test server removal
    println!("\n🗑️  Testing server removal...");
    println!("  Removing server: {}", server7_id);
    server_manager.remove_server(&server7_id).await?;
    
    // Check if server was removed
    if server_manager.get_server(&server7_id).await.is_none() {
        println!("  ✅ Server {} successfully removed", server7_id);
    } else {
        println!("  ❌ Server {} still exists", server7_id);
    }

    // Test tenant server removal
    println!("  Removing all servers for Tenant 3");
    server_manager.remove_tenant_servers(&tenant3).await?;
    
    // Check if tenant 3 servers were removed
    let tenant3_servers = server_manager.get_tenant_servers(&tenant3).await;
    if tenant3_servers.is_empty() {
        println!("  ✅ All Tenant 3 servers successfully removed");
    } else {
        println!("  ❌ Tenant 3 still has {} servers", tenant3_servers.len());
    }

    println!("\n🎉 Server Registry Demo completed successfully!");
    println!("\nKey Features Demonstrated:");
    println!("  ✅ Multi-tenant server management");
    println!("  ✅ Server lifecycle operations (start/stop/restart)");
    println!("  ✅ Tenant-specific server operations");
    println!("  ✅ Server configuration with builder pattern");
    println!("  ✅ Predefined server configurations");
    println!("  ✅ Server statistics and monitoring");
    println!("  ✅ Health check and auto-start functionality");

    Ok(())
}
