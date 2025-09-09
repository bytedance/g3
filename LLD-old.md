# Low-Level Design (LLD): Multi-Tenant Secure Web Gateway using G3 Project

**[Codename: Arcus-G3 LLD]**

## Table of Contents

1. [System Architecture Overview](#1-system-architecture-overview)
2. [Component Design](#2-component-design)
3. [Data Flow Design](#3-data-flow-design)
4. [Multi-Tenancy Implementation](#4-multi-tenancy-implementation)
5. [Security Architecture](#5-security-architecture)
6. [Performance & Scalability Design](#6-performance--scalability-design)
7. [Configuration Management](#7-configuration-management)
8. [Monitoring & Logging](#8-monitoring--logging)
9. [API Design](#9-api-design)
10. [Database Design](#10-database-design)
11. [Deployment Architecture](#11-deployment-architecture)
12. [Error Handling & Recovery](#12-error-handling--recovery)

---

## 1. System Architecture Overview

### 1.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           INGRESS LAYER                                     │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐               │
│  │  Load Balancer  │ │   API Gateway   │ │  SSL Terminator │               │
│  │   (Nginx/F5)    │ │    (Istio)      │ │   (Optional)    │               │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
┌─────────────────────────────────────────────────────────────────────────────┐
│                       G3 PROXY LAYER (TENANT-AWARE)                        │
│                                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐      │
│  │   g3proxy    │ │   g3proxy    │ │   g3proxy    │ │   g3proxy    │      │
│  │  (Tenant A)  │ │  (Tenant B)  │ │  (Tenant C)  │ │   (Shared)   │      │
│  │              │ │              │ │              │ │              │      │
│  │ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │      │
│  │ │HTTP Proxy│ │ │ │SOCKS Proxy│ │ │ │SNI Proxy │ │ │ │TCP TPROXY│ │      │
│  │ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ROUTING & POLICY LAYER                              │
│                                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐      │
│  │Route-Upstream│ │Route-GeoIP   │ │Route-Client  │ │Route-Failover│      │
│  │   Escaper    │ │   Escaper    │ │   Escaper    │ │   Escaper    │      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘      │
│                                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐      │
│  │Proxy-HTTP    │ │Proxy-HTTPS   │ │Proxy-SOCKS5  │ │Direct-Fixed  │      │
│  │  Escaper     │ │   Escaper    │ │   Escaper    │ │   Escaper    │      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
┌─────────────────────────────────────────────────────────────────────────────┐
│                      SECURITY & INSPECTION LAYER                           │
│                                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐      │
│  │   Auditor    │ │   g3fcgen    │ │   ICAP       │ │   Protocol   │      │
│  │  (Traffic    │ │ (Certificate │ │ Integration  │ │  Inspection  │      │
│  │ Inspection)  │ │ Generation)  │ │ (ClamAV etc) │ │  (HTTP/TLS)  │      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
┌─────────────────────────────────────────────────────────────────────────────┐
│                       INFRASTRUCTURE LAYER                                 │
│                                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐      │
│  │   g3statsd   │ │   DNS        │ │  Config      │ │   Storage    │      │
│  │ (Metrics)    │ │ (c-ares/     │ │ Management   │ │  (Redis/     │      │
│  │              │ │  hickory)    │ │ (etcd/K8s)   │ │   etcd)      │      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Component Interaction Matrix

| Component | g3proxy | g3statsd | g3fcgen | ICAP | DNS | Config | Storage |
|-----------|---------|----------|---------|------|-----|---------|---------|
| g3proxy   | -       | Push     | Request | Call | Query| Read   | R/W     |
| g3statsd  | Receive | -        | -       | -    | -   | Read   | Write   |
| g3fcgen   | Serve   | Push     | -       | -    | -   | Read   | Write   |
| ICAP      | Serve   | Push     | -       | -    | -   | Read   | -       |
| DNS       | Serve   | Push     | -       | -    | -   | Read   | Cache   |
| Config    | Serve   | Serve    | Serve   | Serve| Serve| -     | R/W     |
| Storage   | Serve   | Serve    | Serve   | Serve| Serve| Serve | -       |

---

## 2. Component Design

### 2.1 G3Proxy Core Component

#### 2.1.1 Class Diagram

```rust
// Core G3Proxy Architecture
pub struct G3ProxyInstance {
    config: Arc<G3ProxyConfig>,
    servers: Vec<Arc<dyn Server>>,
    escapers: HashMap<String, Arc<dyn Escaper>>,
    resolvers: HashMap<String, Arc<dyn Resolver>>,
    user_groups: HashMap<String, Arc<UserGroup>>,
    auditors: HashMap<String, Arc<Auditor>>,
    stats_client: Arc<StatsClient>,
}

// Server Trait Implementation
pub trait Server: Send + Sync {
    fn name(&self) -> &str;
    fn server_type(&self) -> ServerType;
    fn start(&self) -> Result<(), ServerError>;
    fn stop(&self) -> Result<(), ServerError>;
    fn handle_connection(&self, conn: Connection) -> BoxFuture<'_, Result<(), ConnectionError>>;
    fn get_stats(&self) -> ServerStats;
}

// Escaper Trait Implementation
pub trait Escaper: Send + Sync {
    fn name(&self) -> &str;
    fn escaper_type(&self) -> EscaperType;
    fn escape(&self, task: &EscapeTask) -> BoxFuture<'_, Result<EscapeResult, EscapeError>>;
    fn get_stats(&self) -> EscaperStats;
}
```

#### 2.1.2 Server Types Implementation

```rust
// HTTP Proxy Server
pub struct HttpProxyServer {
    config: HttpProxyConfig,
    escaper: Arc<dyn Escaper>,
    user_group: Option<Arc<UserGroup>>,
    auditor: Option<Arc<Auditor>>,
    listener: TcpListener,
    tls_acceptor: Option<TlsAcceptor>,
}

impl HttpProxyServer {
    async fn handle_http_request(&self, req: HttpRequest, stream: TcpStream) -> Result<(), Error> {
        // 1. Extract tenant context
        let tenant_ctx = self.extract_tenant_context(&req)?;
        
        // 2. Authenticate user if required
        let user_ctx = self.authenticate_user(&req, &tenant_ctx).await?;
        
        // 3. Apply ingress filters
        self.apply_ingress_filters(&req, &tenant_ctx, &user_ctx)?;
        
        // 4. Route through escaper
        let escape_result = self.escaper.escape(&EscapeTask {
            target: req.target(),
            tenant_ctx: tenant_ctx.clone(),
            user_ctx: user_ctx.clone(),
        }).await?;
        
        // 5. Apply audit if configured
        if let Some(auditor) = &self.auditor {
            auditor.audit_request(&req, &tenant_ctx, &user_ctx).await?;
        }
        
        // 6. Proxy the connection
        self.proxy_connection(stream, escape_result).await
    }
}

// SNI Proxy Server
pub struct SniProxyServer {
    config: SniProxyConfig,
    escaper: Arc<dyn Escaper>,
    listener: TcpListener,
}

impl SniProxyServer {
    async fn handle_tls_connection(&self, mut stream: TcpStream) -> Result<(), Error> {
        // 1. Read TLS Client Hello to extract SNI
        let sni = self.extract_sni_from_client_hello(&mut stream).await?;
        
        // 2. Determine tenant from SNI
        let tenant_ctx = self.determine_tenant_from_sni(&sni)?;
        
        // 3. Route through escaper
        let target = format!("{}:443", sni);
        let escape_result = self.escaper.escape(&EscapeTask {
            target: target.parse()?,
            tenant_ctx,
            user_ctx: None,
        }).await?;
        
        // 4. Establish upstream connection and relay
        self.relay_tls_connection(stream, escape_result).await
    }
}
```

### 2.2 Escaper Component Design

#### 2.2.1 Route Escaper Implementation

```rust
// Route-Upstream Escaper
pub struct RouteUpstreamEscaper {
    config: RouteUpstreamConfig,
    rules: Vec<UpstreamRule>,
    fallback_escaper: Arc<dyn Escaper>,
    stats: Arc<EscaperStats>,
}

#[derive(Debug, Clone)]
pub struct UpstreamRule {
    match_type: MatchType,
    pattern: String,
    next_escaper: String,
    tenant_specific: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MatchType {
    ExactMatch,
    ChildMatch,
    SubnetMatch,
    RegexMatch,
}

impl Escaper for RouteUpstreamEscaper {
    async fn escape(&self, task: &EscapeTask) -> Result<EscapeResult, EscapeError> {
        // 1. Extract target hostname/IP
        let target = &task.target;
        
        // 2. Check tenant-specific rules first
        if let Some(tenant_id) = &task.tenant_ctx.as_ref().map(|t| &t.tenant_id) {
            for rule in &self.rules {
                if rule.tenant_specific.as_ref() == Some(tenant_id) {
                    if self.matches_rule(rule, target)? {
                        return self.forward_to_escaper(&rule.next_escaper, task).await;
                    }
                }
            }
        }
        
        // 3. Check global rules
        for rule in &self.rules {
            if rule.tenant_specific.is_none() && self.matches_rule(rule, target)? {
                return self.forward_to_escaper(&rule.next_escaper, task).await;
            }
        }
        
        // 4. Use fallback escaper
        self.fallback_escaper.escape(task).await
    }
}

// GeoIP Route Escaper
pub struct RouteGeoipEscaper {
    config: RouteGeoipConfig,
    geoip_db: Arc<GeoIpDb>,
    rules: Vec<GeoIpRule>,
    default_escaper: Arc<dyn Escaper>,
}

impl Escaper for RouteGeoipEscaper {
    async fn escape(&self, task: &EscapeTask) -> Result<EscapeResult, EscapeError> {
        // 1. Resolve target to IP if needed
        let ip = self.resolve_target_ip(&task.target).await?;
        
        // 2. Lookup country/location
        let location = self.geoip_db.lookup(ip)?;
        
        // 3. Find matching rule
        for rule in &self.rules {
            if rule.matches(&location, &task.tenant_ctx) {
                return self.forward_to_escaper(&rule.next_escaper, task).await;
            }
        }
        
        // 4. Use default escaper
        self.default_escaper.escape(task).await
    }
}
```

### 2.3 User Management Component

#### 2.3.1 User Group Implementation

```rust
pub struct UserGroup {
    name: String,
    config: UserGroupConfig,
    static_users: HashMap<String, Arc<User>>,
    dynamic_source: Option<Box<dyn UserSource>>,
    cache: Arc<RwLock<HashMap<String, Arc<User>>>>,
}

#[derive(Debug, Clone)]
pub struct User {
    name: String,
    tenant_id: String,
    auth_token: AuthToken,
    permissions: UserPermissions,
    rate_limits: RateLimits,
    explicit_sites: Vec<ExplicitSite>,
    blocked: Option<BlockConfig>,
}

#[derive(Debug, Clone)]
pub struct UserPermissions {
    dst_port_filter: Option<Vec<u16>>,
    dst_host_filter_set: Option<HostFilterSet>,
    source_ip_filter: Option<IpFilterSet>,
    time_restrictions: Option<TimeRestrictions>,
}

pub trait UserSource: Send + Sync {
    fn load_users(&self) -> BoxFuture<'_, Result<HashMap<String, User>, UserSourceError>>;
    fn watch_changes(&self) -> BoxStream<'_, UserChangeEvent>;
}

// File-based User Source
pub struct FileUserSource {
    path: PathBuf,
    watcher: Option<RecommendedWatcher>,
}

impl UserSource for FileUserSource {
    async fn load_users(&self) -> Result<HashMap<String, User>, UserSourceError> {
        let content = tokio::fs::read_to_string(&self.path).await?;
        let users_data: Vec<UserData> = serde_json::from_str(&content)?;
        
        let mut users = HashMap::new();
        for user_data in users_data {
            let user = User::from_user_data(user_data)?;
            users.insert(user.name.clone(), Arc::new(user));
        }
        
        Ok(users)
    }
}
```

### 2.4 Auditor Component Design

#### 2.4.1 Traffic Auditor Implementation

```rust
pub struct TrafficAuditor {
    config: AuditorConfig,
    protocol_inspector: Option<ProtocolInspector>,
    tls_interceptor: Option<TlsInterceptor>,
    icap_clients: HashMap<String, Arc<IcapClient>>,
    stats: Arc<AuditorStats>,
}

pub struct TlsInterceptor {
    cert_generator: Arc<CertificateGenerator>,
    ca_cert: X509,
    ca_key: PKey<Private>,
    cert_cache: Arc<RwLock<HashMap<String, (X509, PKey<Private>)>>>,
}

impl TrafficAuditor {
    pub async fn audit_connection(&self, 
        conn: &mut Connection, 
        tenant_ctx: &TenantContext,
        user_ctx: Option<&UserContext>
    ) -> Result<AuditResult, AuditError> {
        let mut audit_result = AuditResult::default();
        
        // 1. Protocol inspection
        if let Some(inspector) = &self.protocol_inspector {
            let protocol_info = inspector.inspect_connection(conn).await?;
            audit_result.protocol_info = Some(protocol_info);
            
            // Log protocol detection
            self.log_protocol_detection(&protocol_info, tenant_ctx, user_ctx).await?;
        }
        
        // 2. TLS interception if HTTPS
        if conn.is_tls() {
            if let Some(interceptor) = &self.tls_interceptor {
                let intercept_result = interceptor.intercept_tls(conn, tenant_ctx).await?;
                audit_result.tls_info = Some(intercept_result);
            }
        }
        
        // 3. ICAP processing for HTTP content
        if conn.is_http() {
            for (name, icap_client) in &self.icap_clients {
                let icap_result = icap_client.process_http_content(conn, tenant_ctx).await?;
                audit_result.icap_results.insert(name.clone(), icap_result);
            }
        }
        
        Ok(audit_result)
    }
}

pub struct IcapClient {
    config: IcapConfig,
    server_pool: Vec<IcapServer>,
    current_server: AtomicUsize,
}

impl IcapClient {
    pub async fn process_http_content(&self, 
        conn: &Connection, 
        tenant_ctx: &TenantContext
    ) -> Result<IcapResult, IcapError> {
        let server = self.get_next_server();
        
        // Build ICAP request
        let icap_request = IcapRequest {
            method: IcapMethod::Respmod,
            uri: server.uri.clone(),
            headers: self.build_icap_headers(tenant_ctx),
            http_request: conn.get_http_request(),
            http_response: conn.get_http_response(),
        };
        
        // Send to ICAP server
        let icap_response = server.send_request(icap_request).await?;
        
        // Process response
        match icap_response.status {
            200 => Ok(IcapResult::Modified(icap_response.http_response)),
            204 => Ok(IcapResult::Unmodified),
            _ => Ok(IcapResult::Error(icap_response.status)),
        }
    }
}
```

---

## 3. Data Flow Design

### 3.1 HTTP Proxy Request Flow

```
┌──────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Client  │    │  g3proxy    │    │   Escaper   │    │  Upstream   │
│          │    │ (HTTP Srv)  │    │   Chain     │    │   Server    │
└─────┬────┘    └──────┬──────┘    └──────┬──────┘    └──────┬──────┘
      │                │                  │                  │
      │ HTTP Request   │                  │                  │
      ├──────────────→ │                  │                  │
      │                │                  │                  │
      │                │ 1. Extract       │                  │
      │                │    Tenant Context │                 │
      │                │ 2. Authenticate   │                 │
      │                │    User           │                 │
      │                │ 3. Apply Filters  │                 │
      │                │                  │                  │
      │                │ Escape Request   │                  │
      │                ├─────────────────→│                  │
      │                │                  │                  │
      │                │                  │ Route Decision   │
      │                │                  │ (GeoIP/Rules)    │
      │                │                  │                  │
      │                │                  │ Upstream Request │
      │                │                  ├─────────────────→│
      │                │                  │                  │
      │                │                  │ Response         │
      │                │                  │←─────────────────┤
      │                │                  │                  │
      │                │ Escape Response  │                  │
      │                │←─────────────────┤                  │
      │                │                  │                  │
      │                │ 4. Audit Traffic │                  │
      │                │ 5. Apply Policies│                  │
      │                │                  │                  │
      │ HTTP Response  │                  │                  │
      │←───────────────┤                  │                  │
      │                │                  │                  │
```

### 3.2 HTTPS with MITM Flow

```
┌──────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Client  │    │  g3proxy    │    │   g3fcgen   │    │   Auditor   │    │  Upstream   │
│          │    │             │    │  (Cert Gen) │    │ (Inspection)│    │   Server    │
└─────┬────┘    └──────┬──────┘    └──────┬──────┘    └──────┬──────┘    └──────┬──────┘
      │                │                  │                  │                  │
      │ TLS ClientHello│                  │                  │                  │
      ├──────────────→ │                  │                  │                  │
      │                │                  │                  │                  │
      │                │ Request Fake Cert│                  │                  │
      │                ├─────────────────→│                  │                  │
      │                │                  │                  │                  │
      │                │ Return Fake Cert │                  │                  │
      │                │←─────────────────┤                  │                  │
      │                │                  │                  │                  │
      │ TLS ServerHello│                  │                  │                  │
      │ (Fake Cert)    │                  │                  │                  │
      │←───────────────┤                  │                  │                  │
      │                │                  │                  │                  │
      │ TLS Handshake  │                  │                  │                  │
      │ Complete       │                  │                  │                  │
      │←──────────────→│                  │                  │                  │
      │                │                  │                  │                  │
      │ HTTP Request   │                  │                  │                  │
      │ (Decrypted)    │                  │                  │                  │
      ├──────────────→ │                  │                  │                  │
      │                │                  │                  │                  │
      │                │ Traffic Audit    │                  │                  │
      │                ├─────────────────────────────────────→│                  │
      │                │                  │                  │                  │
      │                │                  │                  │ TLS to Upstream  │
      │                │                  │                  │ (Real Cert)      │
      │                ├─────────────────────────────────────────────────────────→│
      │                │                  │                  │                  │
      │                │                  │                  │ HTTP Response    │
      │                │←─────────────────────────────────────────────────────────┤
      │                │                  │                  │                  │
      │                │ Audit Response   │                  │                  │
      │                ├─────────────────────────────────────→│                  │
      │                │                  │                  │                  │
      │ HTTP Response  │                  │                  │                  │
      │ (Re-encrypted) │                  │                  │                  │
      │←───────────────┤                  │                  │                  │
      │                │                  │                  │                  │
```

### 3.3 Multi-Tenant Request Routing

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TENANT IDENTIFICATION                              │
└─────────────────────────┬───────────────────────────────────────────────────┘
                          │
    ┌─────────────────────────────────────────────────────────────┐
    │                     Input Methods                           │
    │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │
    │  │Source IP    │ │HTTP Headers │ │SNI Domain   │           │
    │  │Range        │ │(X-Tenant-ID)│ │Pattern      │           │
    │  └─────────────┘ └─────────────┘ └─────────────┘           │
    └─────────────────────────────────────────────────────────────┘
                          │
    ┌─────────────────────────────────────────────────────────────┐
    │                 Tenant Resolution                           │
    │                                                             │
    │  if source_ip in tenant_a_range:                           │
    │      tenant_id = "tenant-a"                                │
    │  elif "X-Tenant-ID" in headers:                            │
    │      tenant_id = headers["X-Tenant-ID"]                    │
    │  elif sni_domain.endswith(".tenant-b.com"):               │
    │      tenant_id = "tenant-b"                                │
    │  else:                                                     │
    │      tenant_id = "default"                                 │
    └─────────────────────────────────────────────────────────────┘
                          │
    ┌─────────────────────────────────────────────────────────────┐
    │                 Configuration Loading                       │
    │                                                             │
    │  tenant_config = config_manager.get_tenant_config(         │
    │      tenant_id                                              │
    │  )                                                          │
    │                                                             │
    │  user_group = user_manager.get_user_group(                 │
    │      tenant_config.user_group                              │
    │  )                                                          │
    │                                                             │
    │  escaper = escaper_manager.get_escaper(                    │
    │      tenant_config.default_escaper                         │
    │  )                                                          │
    └─────────────────────────────────────────────────────────────┘
```

---

## 4. Multi-Tenancy Implementation

### 4.1 Tenant Context Structure

```rust
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub tenant_name: String,
    pub config: Arc<TenantConfig>,
    pub resource_limits: ResourceLimits,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct TenantConfig {
    pub servers: HashMap<String, ServerConfig>,
    pub escapers: HashMap<String, EscaperConfig>,
    pub user_groups: HashMap<String, UserGroupConfig>,
    pub resolvers: HashMap<String, ResolverConfig>,
    pub auditors: HashMap<String, AuditorConfig>,
    pub policies: TenantPolicies,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_concurrent_connections: usize,
    pub max_requests_per_second: f64,
    pub max_bandwidth_mbps: f64,
    pub max_memory_mb: usize,
    pub max_cpu_cores: f32,
}

pub struct TenantManager {
    tenants: Arc<RwLock<HashMap<String, Arc<TenantContext>>>>,
    config_store: Arc<dyn ConfigStore>,
    resource_monitor: Arc<ResourceMonitor>,
}

impl TenantManager {
    pub async fn load_tenant(&self, tenant_id: &str) -> Result<Arc<TenantContext>, TenantError> {
        // 1. Check cache first
        if let Some(tenant) = self.tenants.read().await.get(tenant_id) {
            return Ok(tenant.clone());
        }
        
        // 2. Load from config store
        let tenant_config = self.config_store.get_tenant_config(tenant_id).await?;
        
        // 3. Create tenant context
        let tenant_ctx = Arc::new(TenantContext {
            tenant_id: tenant_id.to_string(),
            tenant_name: tenant_config.name.clone(),
            config: Arc::new(tenant_config),
            resource_limits: self.get_resource_limits(tenant_id).await?,
            created_at: SystemTime::now(),
        });
        
        // 4. Cache and return
        self.tenants.write().await.insert(tenant_id.to_string(), tenant_ctx.clone());
        Ok(tenant_ctx)
    }
    
    pub async fn reload_tenant(&self, tenant_id: &str) -> Result<(), TenantError> {
        // Remove from cache to force reload
        self.tenants.write().await.remove(tenant_id);
        
        // Preload new configuration
        self.load_tenant(tenant_id).await?;
        
        // Notify all g3proxy instances
        self.notify_tenant_reload(tenant_id).await?;
        
        Ok(())
    }
}
```

### 4.2 Resource Isolation Implementation

```rust
pub struct ResourceMonitor {
    tenant_resources: Arc<RwLock<HashMap<String, TenantResourceUsage>>>,
    global_limits: GlobalResourceLimits,
}

#[derive(Debug, Clone)]
pub struct TenantResourceUsage {
    pub current_connections: AtomicUsize,
    pub requests_per_second: AtomicF64,
    pub bandwidth_usage_mbps: AtomicF64,
    pub memory_usage_mb: AtomicUsize,
    pub cpu_usage_percent: AtomicF32,
    pub last_updated: AtomicU64,
}

impl ResourceMonitor {
    pub async fn check_tenant_limits(&self, 
        tenant_id: &str, 
        resource_type: ResourceType,
        requested_amount: f64
    ) -> Result<bool, ResourceError> {
        let tenant_usage = self.get_tenant_usage(tenant_id).await?;
        let tenant_limits = self.get_tenant_limits(tenant_id).await?;
        
        match resource_type {
            ResourceType::Connection => {
                let current = tenant_usage.current_connections.load(Ordering::Relaxed);
                Ok(current < tenant_limits.max_concurrent_connections)
            }
            ResourceType::Bandwidth => {
                let current = tenant_usage.bandwidth_usage_mbps.load(Ordering::Relaxed);
                Ok(current + requested_amount <= tenant_limits.max_bandwidth_mbps)
            }
            ResourceType::RequestRate => {
                let current = tenant_usage.requests_per_second.load(Ordering::Relaxed);
                Ok(current + requested_amount <= tenant_limits.max_requests_per_second)
            }
            _ => Ok(true),
        }
    }
    
    pub async fn track_resource_usage(&self, 
        tenant_id: &str,
        resource_type: ResourceType,
        amount: f64,
        operation: ResourceOperation
    ) -> Result<(), ResourceError> {
        let tenant_usage = self.get_tenant_usage(tenant_id).await?;
        
        match (resource_type, operation) {
            (ResourceType::Connection, ResourceOperation::Allocate) => {
                tenant_usage.current_connections.fetch_add(amount as usize, Ordering::Relaxed);
            }
            (ResourceType::Connection, ResourceOperation::Release) => {
                tenant_usage.current_connections.fetch_sub(amount as usize, Ordering::Relaxed);
            }
            (ResourceType::Bandwidth, ResourceOperation::Update) => {
                tenant_usage.bandwidth_usage_mbps.store(amount, Ordering::Relaxed);
            }
            _ => {}
        }
        
        tenant_usage.last_updated.store(
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            Ordering::Relaxed
        );
        
        Ok(())
    }
}
```

### 4.3 Configuration Isolation

```rust
pub trait ConfigStore: Send + Sync {
    fn get_tenant_config(&self, tenant_id: &str) -> BoxFuture<'_, Result<TenantConfig, ConfigError>>;
    fn set_tenant_config(&self, tenant_id: &str, config: &TenantConfig) -> BoxFuture<'_, Result<(), ConfigError>>;
    fn list_tenants(&self) -> BoxFuture<'_, Result<Vec<String>, ConfigError>>;
    fn delete_tenant(&self, tenant_id: &str) -> BoxFuture<'_, Result<(), ConfigError>>;
    fn watch_config_changes(&self) -> BoxStream<'_, ConfigChangeEvent>;
}

pub struct EtcdConfigStore {
    client: etcd_client::Client,
    base_key: String,
}

impl ConfigStore for EtcdConfigStore {
    async fn get_tenant_config(&self, tenant_id: &str) -> Result<TenantConfig, ConfigError> {
        let key = format!("{}/tenants/{}/config", self.base_key, tenant_id);
        let response = self.client.get(key, None).await?;
        
        if let Some(kv) = response.kvs().first() {
            let config_data = kv.value_str()?;
            let config: TenantConfig = serde_yaml::from_str(config_data)?;
            Ok(config)
        } else {
            Err(ConfigError::TenantNotFound(tenant_id.to_string()))
        }
    }
    
    async fn set_tenant_config(&self, tenant_id: &str, config: &TenantConfig) -> Result<(), ConfigError> {
        let key = format!("{}/tenants/{}/config", self.base_key, tenant_id);
        let config_data = serde_yaml::to_string(config)?;
        
        self.client.put(key, config_data, None).await?;
        Ok(())
    }
}

pub struct KubernetesConfigStore {
    client: kube::Client,
    namespace: String,
}

impl ConfigStore for KubernetesConfigStore {
    async fn get_tenant_config(&self, tenant_id: &str) -> Result<TenantConfig, ConfigError> {
        let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), &self.namespace);
        let configmap_name = format!("g3proxy-tenant-{}", tenant_id);
        
        match api.get(&configmap_name).await {
            Ok(configmap) => {
                let config_data = configmap.data
                    .and_then(|data| data.get("config.yaml").cloned())
                    .ok_or(ConfigError::InvalidConfig)?;
                
                let config: TenantConfig = serde_yaml::from_str(&config_data)?;
                Ok(config)
            }
            Err(kube::Error::Api(api_error)) if api_error.code == 404 => {
                Err(ConfigError::TenantNotFound(tenant_id.to_string()))
            }
            Err(e) => Err(ConfigError::StorageError(e.to_string())),
        }
    }
}
```

---

## 5. Security Architecture

### 5.1 TLS Implementation

```rust
pub struct TlsManager {
    configs: HashMap<TlsEngine, TlsEngineConfig>,
    certificate_store: Arc<CertificateStore>,
    session_cache: Arc<TlsSessionCache>,
}

#[derive(Debug, Clone, Copy)]
pub enum TlsEngine {
    OpenSSL,
    BoringSSL,
    AwsLc,
    AwsLcFips,
    Tongsuo,
    Rustls,
}

pub struct TlsInterceptionManager {
    ca_certificate: X509,
    ca_private_key: PKey<Private>,
    certificate_generator: Arc<CertificateGenerator>,
    certificate_cache: Arc<RwLock<HashMap<String, (X509, PKey<Private>)>>>,
}

impl TlsInterceptionManager {
    pub async fn intercept_connection(&self, 
        client_stream: TcpStream, 
        target_host: &str
    ) -> Result<InterceptedConnection, TlsError> {
        // 1. Generate or retrieve fake certificate for target host
        let (fake_cert, fake_key) = self.get_fake_certificate(target_host).await?;
        
        // 2. Create TLS acceptor with fake certificate
        let acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
        acceptor.set_certificate(&fake_cert)?;
        acceptor.set_private_key(&fake_key)?;
        
        // 3. Perform TLS handshake with client
        let ssl = Ssl::new(acceptor.context())?;
        let mut client_tls_stream = SslStream::new(ssl, client_stream)?;
        client_tls_stream.accept().await?;
        
        // 4. Establish real TLS connection to upstream
        let upstream_stream = TcpStream::connect(format!("{}:443", target_host)).await?;
        let connector = SslConnector::builder(SslMethod::tls())?;
        let ssl = connector.build().configure()?.into_ssl(target_host)?;
        let mut upstream_tls_stream = SslStream::new(ssl, upstream_stream)?;
        upstream_tls_stream.connect().await?;
        
        Ok(InterceptedConnection {
            client_stream: client_tls_stream,
            upstream_stream: upstream_tls_stream,
        })
    }
    
    async fn get_fake_certificate(&self, host: &str) -> Result<(X509, PKey<Private>), TlsError> {
        // Check cache first
        if let Some(cached) = self.certificate_cache.read().await.get(host) {
            return Ok(cached.clone());
        }
        
        // Generate new certificate
        let (cert, key) = self.certificate_generator.generate_for_host(
            host, 
            &self.ca_certificate, 
            &self.ca_private_key
        ).await?;
        
        // Cache the certificate
        self.certificate_cache.write().await.insert(host.to_string(), (cert.clone(), key.clone()));
        
        Ok((cert, key))
    }
}

pub struct CertificateGenerator {
    config: CertificateGeneratorConfig,
}

impl CertificateGenerator {
    pub async fn generate_for_host(&self, 
        host: &str, 
        ca_cert: &X509, 
        ca_key: &PKey<Private>
    ) -> Result<(X509, PKey<Private>), CertificateError> {
        // 1. Generate private key
        let rsa = Rsa::generate(2048)?;
        let private_key = PKey::from_rsa(rsa)?;
        
        // 2. Create certificate
        let mut cert_builder = X509::builder()?;
        cert_builder.set_version(2)?;
        cert_builder.set_pubkey(&private_key)?;
        
        // 3. Set certificate properties
        let mut name_builder = X509NameBuilder::new()?;
        name_builder.append_entry_by_text("CN", host)?;
        let name = name_builder.build();
        cert_builder.set_subject_name(&name)?;
        cert_builder.set_issuer_name(ca_cert.subject_name())?;
        
        // 4. Set validity period
        let not_before = Asn1Time::days_from_now(0)?;
        let not_after = Asn1Time::days_from_now(365)?;
        cert_builder.set_not_before(&not_before)?;
        cert_builder.set_not_after(&not_after)?;
        
        // 5. Add SAN extension for the host
        let context = cert_builder.x509v3_context(Some(ca_cert), None);
        let san = format!("DNS:{}", host);
        let extension = X509Extension::new_nid(None, Some(&context), Nid::SUBJECT_ALT_NAME, &san)?;
        cert_builder.append_extension(extension)?;
        
        // 6. Sign certificate
        cert_builder.sign(ca_key, MessageDigest::sha256())?;
        let certificate = cert_builder.build();
        
        Ok((certificate, private_key))
    }
}
```

### 5.2 Access Control Implementation

```rust
pub struct AccessControlManager {
    tenant_policies: Arc<RwLock<HashMap<String, TenantAccessPolicy>>>,
    user_policies: Arc<RwLock<HashMap<String, UserAccessPolicy>>>,
    site_policies: Arc<RwLock<HashMap<String, SiteAccessPolicy>>>,
}

#[derive(Debug, Clone)]
pub struct AccessDecision {
    pub action: AccessAction,
    pub reason: String,
    pub applied_policies: Vec<String>,
    pub redirect_url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AccessAction {
    Allow,
    Block,
    Warn,
    Redirect(String),
    RequireAuth,
}

impl AccessControlManager {
    pub async fn check_access(&self, 
        request: &AccessRequest,
        tenant_ctx: &TenantContext,
        user_ctx: Option<&UserContext>
    ) -> Result<AccessDecision, AccessError> {
        let mut decision = AccessDecision {
            action: AccessAction::Allow,
            reason: String::new(),
            applied_policies: Vec::new(),
            redirect_url: None,
        };
        
        // 1. Check tenant-level policies
        if let Some(tenant_policy) = self.tenant_policies.read().await.get(&tenant_ctx.tenant_id) {
            let tenant_decision = self.evaluate_tenant_policy(tenant_policy, request).await?;
            if !tenant_decision.action.is_allow() {
                return Ok(tenant_decision);
            }
            decision.applied_policies.extend(tenant_decision.applied_policies);
        }
        
        // 2. Check user-level policies if user context exists
        if let Some(user_ctx) = user_ctx {
            if let Some(user_policy) = self.user_policies.read().await.get(&user_ctx.user_id) {
                let user_decision = self.evaluate_user_policy(user_policy, request, user_ctx).await?;
                if !user_decision.action.is_allow() {
                    return Ok(user_decision);
                }
                decision.applied_policies.extend(user_decision.applied_policies);
            }
        }
        
        // 3. Check site-specific policies
        let site_key = format!("{}:{}", tenant_ctx.tenant_id, request.target_host);
        if let Some(site_policy) = self.site_policies.read().await.get(&site_key) {
            let site_decision = self.evaluate_site_policy(site_policy, request).await?;
            if !site_decision.action.is_allow() {
                return Ok(site_decision);
            }
            decision.applied_policies.extend(site_decision.applied_policies);
        }
        
        Ok(decision)
    }
    
    async fn evaluate_tenant_policy(&self, 
        policy: &TenantAccessPolicy, 
        request: &AccessRequest
    ) -> Result<AccessDecision, AccessError> {
        // Check time restrictions
        if let Some(time_restrictions) = &policy.time_restrictions {
            if !time_restrictions.is_allowed_now() {
                return Ok(AccessDecision {
                    action: AccessAction::Block,
                    reason: "Access denied due to time restrictions".to_string(),
                    applied_policies: vec!["tenant_time_restriction".to_string()],
                    redirect_url: None,
                });
            }
        }
        
        // Check IP restrictions
        if let Some(ip_filter) = &policy.source_ip_filter {
            if !ip_filter.is_allowed(&request.source_ip) {
                return Ok(AccessDecision {
                    action: AccessAction::Block,
                    reason: "Source IP not allowed".to_string(),
                    applied_policies: vec!["tenant_ip_filter".to_string()],
                    redirect_url: None,
                });
            }
        }
        
        // Check destination filters
        if let Some(dest_filter) = &policy.destination_filter {
            match dest_filter.check_destination(&request.target_host, request.target_port) {
                DestinationDecision::Block => {
                    return Ok(AccessDecision {
                        action: AccessAction::Block,
                        reason: "Destination blocked by policy".to_string(),
                        applied_policies: vec!["tenant_destination_filter".to_string()],
                        redirect_url: policy.block_page_url.clone(),
                    });
                }
                DestinationDecision::Allow => {}
            }
        }
        
        Ok(AccessDecision {
            action: AccessAction::Allow,
            reason: "Tenant policy allows access".to_string(),
            applied_policies: vec!["tenant_policy".to_string()],
            redirect_url: None,
        })
    }
}
```

---

## 6. Performance & Scalability Design

### 6.1 Connection Pool Management

```rust
pub struct ConnectionPoolManager {
    pools: Arc<RwLock<HashMap<String, Arc<ConnectionPool>>>>,
    config: ConnectionPoolConfig,
}

pub struct ConnectionPool {
    target: SocketAddr,
    connections: Arc<Mutex<VecDeque<PooledConnection>>>,
    active_connections: AtomicUsize,
    max_connections: usize,
    connection_timeout: Duration,
    idle_timeout: Duration,
    stats: Arc<ConnectionPoolStats>,
}

pub struct PooledConnection {
    stream: TcpStream,
    created_at: Instant,
    last_used: Instant,
    use_count: usize,
}

impl ConnectionPool {
    pub async fn get_connection(&self) -> Result<PooledConnection, ConnectionPoolError> {
        // 1. Try to get connection from pool
        if let Some(conn) = self.try_get_pooled_connection().await? {
            return Ok(conn);
        }
        
        // 2. Check if we can create new connection
        let active = self.active_connections.load(Ordering::Relaxed);
        if active >= self.max_connections {
            return Err(ConnectionPoolError::PoolExhausted);
        }
        
        // 3. Create new connection
        let stream = timeout(
            self.connection_timeout,
            TcpStream::connect(&self.target)
        ).await??;
        
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        
        Ok(PooledConnection {
            stream,
            created_at: Instant::now(),
            last_used: Instant::now(),
            use_count: 0,
        })
    }
    
    pub async fn return_connection(&self, mut conn: PooledConnection) -> Result<(), ConnectionPoolError> {
        conn.last_used = Instant::now();
        conn.use_count += 1;
        
        // Check if connection should be kept
        if conn.use_count > 100 || conn.created_at.elapsed() > Duration::from_secs(300) {
            // Connection is too old or used too much, close it
            self.active_connections.fetch_sub(1, Ordering::Relaxed);
            return Ok(());
        }
        
        // Return to pool
        self.connections.lock().await.push_back(conn);
        Ok(())
    }
    
    async fn cleanup_idle_connections(&self) {
        let mut connections = self.connections.lock().await;
        let now = Instant::now();
        
        let mut to_remove = 0;
        for conn in connections.iter() {
            if now.duration_since(conn.last_used) > self.idle_timeout {
                to_remove += 1;
            } else {
                break; // VecDeque is ordered by last_used
            }
        }
        
        for _ in 0..to_remove {
            connections.pop_front();
            self.active_connections.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
```

### 6.2 Load Balancing Implementation

```rust
pub trait LoadBalancer: Send + Sync {
    fn select_upstream(&self, upstreams: &[Upstream], context: &SelectionContext) -> Option<&Upstream>;
    fn update_upstream_stats(&self, upstream: &Upstream, result: &UpstreamResult);
}

pub struct RoundRobinBalancer {
    counter: AtomicUsize,
}

impl LoadBalancer for RoundRobinBalancer {
    fn select_upstream(&self, upstreams: &[Upstream], _context: &SelectionContext) -> Option<&Upstream> {
        if upstreams.is_empty() {
            return None;
        }
        
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % upstreams.len();
        upstreams.get(index)
    }
}

pub struct WeightedRoundRobinBalancer {
    current_weights: Arc<RwLock<HashMap<String, i32>>>,
}

impl LoadBalancer for WeightedRoundRobinBalancer {
    fn select_upstream(&self, upstreams: &[Upstream], _context: &SelectionContext) -> Option<&Upstream> {
        if upstreams.is_empty() {
            return None;
        }
        
        let mut current_weights = self.current_weights.write().unwrap();
        let mut total_weight = 0;
        let mut selected: Option<&Upstream> = None;
        let mut max_current_weight = i32::MIN;
        
        for upstream in upstreams {
            if !upstream.is_healthy() {
                continue;
            }
            
            let weight = upstream.config.weight as i32;
            total_weight += weight;
            
            let current_weight = current_weights.entry(upstream.id.clone()).or_insert(0);
            *current_weight += weight;
            
            if *current_weight > max_current_weight {
                max_current_weight = *current_weight;
                selected = Some(upstream);
            }
        }
        
        if let Some(selected_upstream) = selected {
            let current_weight = current_weights.get_mut(&selected_upstream.id).unwrap();
            *current_weight -= total_weight;
        }
        
        selected
    }
}

pub struct ConsistentHashBalancer {
    hash_ring: Arc<RwLock<HashRing<Upstream>>>,
}

impl LoadBalancer for ConsistentHashBalancer {
    fn select_upstream(&self, upstreams: &[Upstream], context: &SelectionContext) -> Option<&Upstream> {
        let hash_key = match &context.hash_key {
            Some(key) => key.clone(),
            None => context.client_ip.to_string(),
        };
        
        let ring = self.hash_ring.read().unwrap();
        ring.get(&hash_key).cloned()
    }
}
```

### 6.3 Rate Limiting Implementation

```rust
pub struct RateLimiter {
    limits: HashMap<String, Arc<TokenBucket>>,
    global_limit: Arc<TokenBucket>,
}

pub struct TokenBucket {
    tokens: Arc<AtomicU64>,
    capacity: u64,
    refill_rate: u64,
    last_refill: Arc<AtomicU64>,
}

impl TokenBucket {
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            tokens: Arc::new(AtomicU64::new(capacity)),
            capacity,
            refill_rate,
            last_refill: Arc::new(AtomicU64::new(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
            )),
        }
    }
    
    pub fn try_consume(&self, tokens: u64) -> bool {
        self.refill();
        
        let current_tokens = self.tokens.load(Ordering::Relaxed);
        if current_tokens >= tokens {
            let new_tokens = current_tokens - tokens;
            self.tokens.compare_exchange_weak(
                current_tokens,
                new_tokens,
                Ordering::Relaxed,
                Ordering::Relaxed
            ).is_ok()
        } else {
            false
        }
    }
    
    fn refill(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let last_refill = self.last_refill.load(Ordering::Relaxed);
        
        if now > last_refill {
            let time_passed = now - last_refill;
            let tokens_to_add = time_passed * self.refill_rate;
            
            let current_tokens = self.tokens.load(Ordering::Relaxed);
            let new_tokens = std::cmp::min(self.capacity, current_tokens + tokens_to_add);
            
            self.tokens.store(new_tokens, Ordering::Relaxed);
            self.last_refill.store(now, Ordering::Relaxed);
        }
    }
}

impl RateLimiter {
    pub async fn check_rate_limit(&self, 
        tenant_id: &str, 
        user_id: Option<&str>, 
        limit_type: RateLimitType
    ) -> Result<bool, RateLimitError> {
        // 1. Check global rate limit
        if !self.global_limit.try_consume(1) {
            return Ok(false);
        }
        
        // 2. Check tenant rate limit
        let tenant_key = format!("tenant:{}", tenant_id);
        if let Some(tenant_bucket) = self.limits.get(&tenant_key) {
            if !tenant_bucket.try_consume(1) {
                return Ok(false);
            }
        }
        
        // 3. Check user rate limit if applicable
        if let Some(user_id) = user_id {
            let user_key = format!("user:{}:{}", tenant_id, user_id);
            if let Some(user_bucket) = self.limits.get(&user_key) {
                if !user_bucket.try_consume(1) {
                    return Ok(false);
                }
            }
        }
        
        Ok(true)
    }
}
```

---

## 7. Configuration Management

### 7.1 Configuration Structure

```yaml
# Master Configuration Template
apiVersion: v1
kind: ConfigMap
metadata:
  name: g3proxy-config
  namespace: g3proxy
data:
  main.yaml: |
    runtime:
      thread_number: 8
    
    worker:
      thread_number: 8
      sched_affinity: true
    
    controller:
      local:
        recv_timeout: 30
        send_timeout: 1
    
    log:
      target: journal
      level: info
    
    stat:
      target:
        udp: "127.0.0.1:8125"
      prefix: "g3proxy"
      emit_interval: 30s
    
    # Include tenant-specific configurations
    server: "server.d"
    escaper: "escaper.d"
    resolver: "resolver.d"
    user_group: "user_group.d"
    auditor: "auditor.d"

---
# Tenant-specific Configuration
apiVersion: v1
kind: ConfigMap
metadata:
  name: g3proxy-tenant-a
  namespace: g3proxy
data:
  server.yaml: |
    - name: tenant-a-http
      type: http_proxy
      escaper: tenant-a-router
      user_group: tenant-a-users
      listen:
        address: "[::]:8080"
      tenant_context:
        tenant_id: "tenant-a"
        tenant_name: "Tenant A Corp"
      ingress_network_filter:
        default: allow
        deny:
          - "10.0.0.0/8"
      request_rate_limit: 1000/s
      tcp_sock_speed_limit: 100M/s

  escaper.yaml: |
    - name: tenant-a-router
      type: route_upstream
      resolver: tenant-a-dns
      rules:
        - exact_match: "internal.tenant-a.com"
          next: tenant-a-direct
        - child_match: "blocked.example.com"
          next: tenant-a-deny
        - regex_match: ".*\\.malicious\\..*"
          next: tenant-a-deny
      fallback_next: tenant-a-internet
      
    - name: tenant-a-direct
      type: direct_fixed
      resolver: tenant-a-dns
      resolve_strategy: IPv4First
      
    - name: tenant-a-internet
      type: proxy_https
      proxy_addr: "proxy.tenant-a.com:8443"
      tls_client:
        ca_certificate: "/certs/tenant-a-ca.pem"
      
    - name: tenant-a-deny
      type: dummy_deny

  resolver.yaml: |
    - name: tenant-a-dns
      type: hickory
      server: "1.1.1.1"
      encryption: dns-over-https
      bind_ipv4: "192.168.1.10"

  user_group.yaml: |
    - name: tenant-a-users
      static_users:
        - name: admin
          token:
            salt: "tenant-a-salt"
            sha1: "hashed-password"
          dst_port_filter: [80, 443]
          dst_host_filter_set:
            exact: ["allowed.example.com"]
            child: ["*.safe.example.net"]
          request_rate_limit: 100/s
          tcp_sock_speed_limit: 10M/s
      source:
        type: file
        path: "/config/tenant-a/users.json"

  auditor.yaml: |
    - name: tenant-a-auditor
      protocol_inspection: {}
      tls_cert_generator:
        peer_addr: "127.0.0.1:2999"
      icap_reqmod_service: "icap://security.tenant-a.com:1344/reqmod"
      icap_respmod_service: "icap://security.tenant-a.com:1344/respmod"
      application_audit_ratio: 1.0
```

### 7.2 Dynamic Configuration Management

```rust
pub struct ConfigManager {
    store: Arc<dyn ConfigStore>,
    watchers: Arc<RwLock<HashMap<String, ConfigWatcher>>>,
    reload_notifier: broadcast::Sender<ConfigChangeEvent>,
}

#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub tenant_id: Option<String>,
    pub config_type: ConfigType,
    pub change_type: ChangeType,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone)]
pub enum ConfigType {
    Server,
    Escaper,
    Resolver,
    UserGroup,
    Auditor,
    Global,
}

#[derive(Debug, Clone)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}

impl ConfigManager {
    pub async fn watch_config_changes(&self) -> Result<(), ConfigError> {
        let mut change_stream = self.store.watch_config_changes();
        
        while let Some(event) = change_stream.next().await {
            match event {
                Ok(change_event) => {
                    info!("Config change detected: {:?}", change_event);
                    
                    // Validate new configuration
                    if let Err(e) = self.validate_config_change(&change_event).await {
                        error!("Config validation failed: {:?}", e);
                        continue;
                    }
                    
                    // Notify all components
                    if let Err(e) = self.reload_notifier.send(change_event.clone()) {
                        error!("Failed to notify config change: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("Error watching config changes: {:?}", e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn validate_config_change(&self, event: &ConfigChangeEvent) -> Result<(), ConfigError> {
        match (&event.tenant_id, &event.config_type) {
            (Some(tenant_id), ConfigType::Server) => {
                let config = self.store.get_tenant_config(tenant_id).await?;
                self.validate_server_config(&config.servers)?;
            }
            (Some(tenant_id), ConfigType::Escaper) => {
                let config = self.store.get_tenant_config(tenant_id).await?;
                self.validate_escaper_config(&config.escapers)?;
            }
            (None, ConfigType::Global) => {
                let config = self.store.get_global_config().await?;
                self.validate_global_config(&config)?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    pub async fn reload_component_config(&self, 
        component_type: ConfigType, 
        tenant_id: Option<String>
    ) -> Result<(), ConfigError> {
        match (component_type, tenant_id) {
            (ConfigType::Server, Some(tenant_id)) => {
                // Reload tenant-specific server configuration
                let config = self.store.get_tenant_config(&tenant_id).await?;
                let server_manager = self.get_server_manager(&tenant_id)?;
                server_manager.reload_config(config.servers).await?;
            }
            (ConfigType::UserGroup, Some(tenant_id)) => {
                // Reload tenant-specific user group configuration
                let config = self.store.get_tenant_config(&tenant_id).await?;
                let user_manager = self.get_user_manager(&tenant_id)?;
                user_manager.reload_config(config.user_groups).await?;
            }
            _ => {}
        }
        
        Ok(())
    }
}

pub struct ConfigValidator {
    schema_validators: HashMap<ConfigType, Box<dyn SchemaValidator>>,
}

impl ConfigValidator {
    pub fn validate_server_config(&self, config: &HashMap<String, ServerConfig>) -> Result<(), ConfigError> {
        for (name, server_config) in config {
            // Validate server name uniqueness
            if name.is_empty() {
                return Err(ConfigError::InvalidConfig("Server name cannot be empty".to_string()));
            }
            
            // Validate listen address
            match server_config.listen.address.parse::<SocketAddr>() {
                Ok(_) => {}
                Err(_) => {
                    return Err(ConfigError::InvalidConfig(
                        format!("Invalid listen address for server {}: {}", name, server_config.listen.address)
                    ));
                }
            }
            
            // Validate escaper reference
            if server_config.escaper.is_empty() {
                return Err(ConfigError::InvalidConfig(
                    format!("Server {} must specify an escaper", name)
                ));
            }
            
            // Validate TLS configuration if present
            if let Some(tls_config) = &server_config.tls_server {
                self.validate_tls_config(tls_config)?;
            }
        }
        
        Ok(())
    }
    
    pub fn validate_escaper_config(&self, config: &HashMap<String, EscaperConfig>) -> Result<(), ConfigError> {
        let mut escaper_names = HashSet::new();
        
        for (name, escaper_config) in config {
            // Check for duplicate names
            if !escaper_names.insert(name.clone()) {
                return Err(ConfigError::InvalidConfig(
                    format!("Duplicate escaper name: {}", name)
                ));
            }
            
            // Validate escaper-specific configuration
            match &escaper_config.escaper_type {
                EscaperType::RouteUpstream => {
                    self.validate_route_upstream_config(escaper_config)?;
                }
                EscaperType::ProxyHttp => {
                    self.validate_proxy_http_config(escaper_config)?;
                }
                EscaperType::DirectFixed => {
                    self.validate_direct_fixed_config(escaper_config)?;
                }
                _ => {}
            }
        }
        
        // Validate escaper references
        self.validate_escaper_references(config)?;
        
        Ok(())
    }
}
```

---

## 8. Monitoring & Logging

### 8.1 Metrics Collection Architecture

```rust
pub struct MetricsCollector {
    collectors: HashMap<MetricType, Box<dyn MetricCollector>>,
    aggregator: Arc<MetricsAggregator>,
    exporters: Vec<Box<dyn MetricsExporter>>,
}

pub trait MetricCollector: Send + Sync {
    fn collect(&self) -> BoxFuture<'_, Result<Vec<Metric>, MetricError>>;
    fn get_metric_type(&self) -> MetricType;
}

#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub value: MetricValue,
    pub tags: HashMap<String, String>,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Timer(Duration),
}

// Server Metrics Collector
pub struct ServerMetricsCollector {
    servers: Arc<RwLock<HashMap<String, Arc<dyn Server>>>>,
}

impl MetricCollector for ServerMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>, MetricError> {
        let mut metrics = Vec::new();
        let servers = self.servers.read().await;
        
        for (name, server) in servers.iter() {
            let stats = server.get_stats();
            let base_tags = hashmap! {
                "server".to_string() => name.clone(),
                "server_type".to_string() => server.server_type().to_string(),
            };
            
            // Connection metrics
            metrics.push(Metric {
                name: "g3proxy.server.connections.total".to_string(),
                value: MetricValue::Counter(stats.total_connections),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            metrics.push(Metric {
                name: "g3proxy.server.connections.active".to_string(),
                value: MetricValue::Gauge(stats.active_connections as f64),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            // Request metrics
            metrics.push(Metric {
                name: "g3proxy.server.requests.total".to_string(),
                value: MetricValue::Counter(stats.total_requests),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            metrics.push(Metric {
                name: "g3proxy.server.requests.rate".to_string(),
                value: MetricValue::Gauge(stats.request_rate),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            // Error metrics
            metrics.push(Metric {
                name: "g3proxy.server.errors.total".to_string(),
                value: MetricValue::Counter(stats.total_errors),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            // Response time histogram
            metrics.push(Metric {
                name: "g3proxy.server.response_time".to_string(),
                value: MetricValue::Histogram(stats.response_time_histogram.clone()),
                tags: base_tags,
                timestamp: SystemTime::now(),
            });
        }
        
        Ok(metrics)
    }
}

// Tenant Metrics Collector
pub struct TenantMetricsCollector {
    tenant_manager: Arc<TenantManager>,
    resource_monitor: Arc<ResourceMonitor>,
}

impl MetricCollector for TenantMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>, MetricError> {
        let mut metrics = Vec::new();
        let tenants = self.tenant_manager.list_tenants().await?;
        
        for tenant_id in tenants {
            let tenant_usage = self.resource_monitor.get_tenant_usage(&tenant_id).await?;
            let base_tags = hashmap! {
                "tenant_id".to_string() => tenant_id.clone(),
            };
            
            // Resource usage metrics
            metrics.push(Metric {
                name: "g3proxy.tenant.connections.active".to_string(),
                value: MetricValue::Gauge(tenant_usage.current_connections.load(Ordering::Relaxed) as f64),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            metrics.push(Metric {
                name: "g3proxy.tenant.bandwidth.usage".to_string(),
                value: MetricValue::Gauge(tenant_usage.bandwidth_usage_mbps.load(Ordering::Relaxed)),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            metrics.push(Metric {
                name: "g3proxy.tenant.requests.rate".to_string(),
                value: MetricValue::Gauge(tenant_usage.requests_per_second.load(Ordering::Relaxed)),
                tags: base_tags.clone(),
                timestamp: SystemTime::now(),
            });
            
            metrics.push(Metric {
                name: "g3proxy.tenant.memory.usage".to_string(),
                value: MetricValue::Gauge(tenant_usage.memory_usage_mb.load(Ordering::Relaxed) as f64),
                tags: base_tags,
                timestamp: SystemTime::now(),
            });
        }
        
        Ok(metrics)
    }
}
```

### 8.2 Structured Logging Implementation

```rust
pub struct StructuredLogger {
    config: LogConfig,
    sinks: Vec<Box<dyn LogSink>>,
    filters: Vec<Box<dyn LogFilter>>,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
}

pub trait LogSink: Send + Sync {
    fn write(&self, entry: &LogEntry) -> BoxFuture<'_, Result<(), LogError>>;
    fn flush(&self) -> BoxFuture<'_, Result<(), LogError>>;
}

// Journal Log Sink
pub struct JournalLogSink {
    journal: Arc<Mutex<Journal>>,
}

impl LogSink for JournalLogSink {
    async fn write(&self, entry: &LogEntry) -> Result<(), LogError> {
        let mut journal = self.journal.lock().await;
        
        let mut journal_entry = JournalEntry::new();
        journal_entry.set_message(&entry.message);
        journal_entry.set_priority(entry.level.to_syslog_priority());
        journal_entry.set_syslog_identifier("g3proxy");
        
        // Add structured fields
        for (key, value) in &entry.fields {
            journal_entry.set_field(&key.to_uppercase(), &value.to_string());
        }
        
        journal.send_entry(journal_entry).await?;
        Ok(())
    }
}

// Fluentd Log Sink
pub struct FluentdLogSink {
    client: Arc<FluentdClient>,
    tag: String,
}

impl LogSink for FluentdLogSink {
    async fn write(&self, entry: &LogEntry) -> Result<(), LogError> {
        let mut record = HashMap::new();
        record.insert("timestamp".to_string(), json!(entry.timestamp));
        record.insert("level".to_string(), json!(entry.level.to_string()));
        record.insert("target".to_string(), json!(entry.target));
        record.insert("message".to_string(), json!(entry.message));
        
        // Add all structured fields
        for (key, value) in &entry.fields {
            record.insert(key.clone(), value.clone());
        }
        
        self.client.post(&self.tag, record).await?;
        Ok(())
    }
}

// Tenant-aware Log Filter
pub struct TenantLogFilter {
    tenant_configs: Arc<RwLock<HashMap<String, TenantLogConfig>>>,
}

impl LogFilter for TenantLogFilter {
    fn should_log(&self, entry: &LogEntry) -> bool {
        // Extract tenant_id from log entry
        if let Some(tenant_id) = entry.fields.get("tenant_id") {
            if let Some(tenant_id_str) = tenant_id.as_str() {
                let configs = self.tenant_configs.read().unwrap();
                if let Some(config) = configs.get(tenant_id_str) {
                    return entry.level >= config.min_log_level;
                }
            }
        }
        
        // Default: allow all logs if no tenant-specific config
        true
    }
}

// Log macros for structured logging
#[macro_export]
macro_rules! log_with_context {
    ($level:expr, $tenant_ctx:expr, $user_ctx:expr, $message:expr, { $($key:expr => $value:expr),* }) => {
        {
            let mut fields = std::collections::HashMap::new();
            
            if let Some(tenant) = $tenant_ctx {
                fields.insert("tenant_id".to_string(), serde_json::json!(tenant.tenant_id));
                fields.insert("tenant_name".to_string(), serde_json::json!(tenant.tenant_name));
            }
            
            if let Some(user) = $user_ctx {
                fields.insert("user_id".to_string(), serde_json::json!(user.user_id));
                fields.insert("user_name".to_string(), serde_json::json!(user.user_name));
            }
            
            $(
                fields.insert($key.to_string(), serde_json::json!($value));
            )*
            
            let entry = LogEntry {
                timestamp: std::time::SystemTime::now(),
                level: $level,
                target: module_path!().to_string(),
                message: $message.to_string(),
                fields,
            };
            
            GLOBAL_LOGGER.log(entry);
        }
    };
}

// Usage example
async fn handle_request(request: &HttpRequest, tenant_ctx: &TenantContext, user_ctx: Option<&UserContext>) {
    log_with_context!(
        LogLevel::Info,
        Some(tenant_ctx),
        user_ctx,
        "Processing HTTP request",
        {
            "method" => request.method().as_str(),
            "uri" => request.uri().to_string(),
            "user_agent" => request.headers().get("user-agent").map(|h| h.to_str().unwrap_or("")).unwrap_or(""),
            "content_length" => request.headers().get("content-length").map(|h| h.to_str().unwrap_or("0")).unwrap_or("0")
        }
    );
}
```

---

## 9. API Design

### 9.1 Management API

```rust
// REST API Definition
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};

#[derive(Clone)]
pub struct ApiState {
    pub tenant_manager: Arc<TenantManager>,
    pub config_manager: Arc<ConfigManager>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub auth_manager: Arc<AuthManager>,
}

pub fn create_api_router() -> Router<ApiState> {
    Router::new()
        // Tenant Management
        .route("/api/v1/tenants", get(list_tenants).post(create_tenant))
        .route("/api/v1/tenants/:tenant_id", get(get_tenant).put(update_tenant).delete(delete_tenant))
        .route("/api/v1/tenants/:tenant_id/config", get(get_tenant_config).put(update_tenant_config))
        .route("/api/v1/tenants/:tenant_id/reload", post(reload_tenant_config))
        
        // User Management
        .route("/api/v1/tenants/:tenant_id/users", get(list_users).post(create_user))
        .route("/api/v1/tenants/:tenant_id/users/:user_id", get(get_user).put(update_user).delete(delete_user))
        
        // Policy Management
        .route("/api/v1/tenants/:tenant_id/policies", get(list_policies).post(create_policy))
        .route("/api/v1/tenants/:tenant_id/policies/:policy_id", get(get_policy).put(update_policy).delete(delete_policy))
        
        // Monitoring & Statistics
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/tenants/:tenant_id/metrics", get(get_tenant_metrics))
        .route("/api/v1/health", get(health_check))
        
        // Configuration Management
        .route("/api/v1/config/global", get(get_global_config).put(update_global_config))
        .route("/api/v1/config/reload", post(reload_global_config))
}

// API Request/Response Models
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTenantRequest {
    pub tenant_id: String,
    pub tenant_name: String,
    pub config: TenantConfigRequest,
    pub resource_limits: ResourceLimitsRequest,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TenantResponse {
    pub tenant_id: String,
    pub tenant_name: String,
    pub status: TenantStatus,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub resource_usage: ResourceUsageResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TenantConfigRequest {
    pub servers: HashMap<String, ServerConfigRequest>,
    pub escapers: HashMap<String, EscaperConfigRequest>,
    pub user_groups: HashMap<String, UserGroupConfigRequest>,
    pub resolvers: HashMap<String, ResolverConfigRequest>,
    pub auditors: HashMap<String, AuditorConfigRequest>,
}

// API Handlers
pub async fn create_tenant(
    State(state): State<ApiState>,
    Json(request): Json<CreateTenantRequest>,
) -> Result<Json<TenantResponse>, ApiError> {
    // 1. Validate request
    validate_tenant_request(&request)?;
    
    // 2. Check if tenant already exists
    if state.tenant_manager.tenant_exists(&request.tenant_id).await? {
        return Err(ApiError::TenantAlreadyExists(request.tenant_id));
    }
    
    // 3. Create tenant configuration
    let tenant_config = TenantConfig::from_request(request.config)?;
    
    // 4. Store configuration
    state.config_manager.create_tenant_config(&request.tenant_id, &tenant_config).await?;
    
    // 5. Initialize tenant
    let tenant_ctx = state.tenant_manager.load_tenant(&request.tenant_id).await?;
    
    // 6. Return response
    Ok(Json(TenantResponse {
        tenant_id: tenant_ctx.tenant_id.clone(),
        tenant_name: tenant_ctx.tenant_name.clone(),
        status: TenantStatus::Active,
        created_at: tenant_ctx.created_at,
        updated_at: tenant_ctx.created_at,
        resource_usage: get_tenant_resource_usage(&state, &tenant_ctx.tenant_id).await?,
    }))
}

pub async fn get_tenant_metrics(
    State(state): State<ApiState>,
    Path(tenant_id): Path<String>,
    Query(params): Query<MetricsQueryParams>,
) -> Result<Json<MetricsResponse>, ApiError> {
    // 1. Verify tenant exists
    let _tenant_ctx = state.tenant_manager.get_tenant(&tenant_id).await?;
    
    // 2. Collect tenant-specific metrics
    let metrics = state.metrics_collector.collect_tenant_metrics(&tenant_id, &params).await?;
    
    // 3. Return response
    Ok(Json(MetricsResponse {
        tenant_id: Some(tenant_id),
        metrics,
        collected_at: SystemTime::now(),
    }))
}

pub async fn reload_tenant_config(
    State(state): State<ApiState>,
    Path(tenant_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    // 1. Verify tenant exists
    let _tenant_ctx = state.tenant_manager.get_tenant(&tenant_id).await?;
    
    // 2. Reload configuration
    state.config_manager.reload_tenant_config(&tenant_id).await?;
    
    // 3. Notify tenant manager
    state.tenant_manager.reload_tenant(&tenant_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}
```

### 9.2 Control Protocol (g3proxy-ctl)

```rust
// Control Protocol Messages
#[derive(Debug, Serialize, Deserialize)]
pub enum ControlMessage {
    // Configuration Commands
    ReloadConfig { component: Option<String> },
    GetConfig { component: String },
    SetConfig { component: String, config: serde_json::Value },
    
    // Statistics Commands
    GetStats { component: Option<String> },
    ResetStats { component: String },
    
    // Tenant Commands
    CreateTenant { tenant_id: String, config: TenantConfig },
    DeleteTenant { tenant_id: String },
    ListTenants,
    
    // User Commands
    CreateUser { tenant_id: String, user: User },
    DeleteUser { tenant_id: String, user_id: String },
    ListUsers { tenant_id: String },
    
    // Connection Management
    ListConnections { tenant_id: Option<String> },
    KillConnection { connection_id: String },
    
    // Health Check
    HealthCheck,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ControlResponse {
    Success { data: Option<serde_json::Value> },
    Error { code: u32, message: String },
}

// Control Server Implementation
pub struct ControlServer {
    listener: UnixListener,
    handlers: HashMap<String, Box<dyn ControlHandler>>,
    state: Arc<G3ProxyState>,
}

pub trait ControlHandler: Send + Sync {
    fn handle(&self, message: ControlMessage, state: &G3ProxyState) -> BoxFuture<'_, Result<ControlResponse, ControlError>>;
}

impl ControlServer {
    pub async fn start(&self) -> Result<(), ControlError> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let handlers = self.handlers.clone();
            let state = self.state.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, handlers, state).await {
                    error!("Control connection error: {:?}", e);
                }
            });
        }
    }
    
    async fn handle_connection(
        stream: UnixStream,
        handlers: HashMap<String, Box<dyn ControlHandler>>,
        state: Arc<G3ProxyState>,
    ) -> Result<(), ControlError> {
        let mut lines = BufReader::new(stream).lines();
        
        while let Some(line) = lines.next_line().await? {
            let message: ControlMessage = serde_json::from_str(&line)?;
            let response = Self::process_message(message, &handlers, &state).await;
            
            let response_json = serde_json::to_string(&response)?;
            lines.write_all(response_json.as_bytes()).await?;
            lines.write_all(b"\n").await?;
            lines.flush().await?;
        }
        
        Ok(())
    }
    
    async fn process_message(
        message: ControlMessage,
        handlers: &HashMap<String, Box<dyn ControlHandler>>,
        state: &G3ProxyState,
    ) -> ControlResponse {
        match message {
            ControlMessage::ReloadConfig { component } => {
                match component {
                    Some(comp) => {
                        if let Some(handler) = handlers.get(&comp) {
                            handler.handle(message, state).await.unwrap_or_else(|e| {
                                ControlResponse::Error {
                                    code: 1,
                                    message: format!("Failed to reload {}: {}", comp, e),
                                }
                            })
                        } else {
                            ControlResponse::Error {
                                code: 2,
                                message: format!("Unknown component: {}", comp),
                            }
                        }
                    }
                    None => {
                        // Reload all components
                        for handler in handlers.values() {
                            if let Err(e) = handler.handle(message.clone(), state).await {
                                return ControlResponse::Error {
                                    code: 1,
                                    message: format!("Failed to reload: {}", e),
                                };
                            }
                        }
                        ControlResponse::Success { data: None }
                    }
                }
            }
            _ => {
                ControlResponse::Error {
                    code: 3,
                    message: "Unimplemented command".to_string(),
                }
            }
        }
    }
}
```

---

## 10. Database Design

### 10.1 Configuration Storage Schema

```sql
-- Tenant Configuration Tables
CREATE TABLE tenants (
    tenant_id VARCHAR(255) PRIMARY KEY,
    tenant_name VARCHAR(255) NOT NULL,
    status ENUM('active', 'inactive', 'suspended') DEFAULT 'active',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    config JSON NOT NULL,
    resource_limits JSON,
    metadata JSON
);

CREATE INDEX idx_tenants_status ON tenants(status);
CREATE INDEX idx_tenants_created ON tenants(created_at);

-- Server Configuration
CREATE TABLE server_configs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    server_name VARCHAR(255) NOT NULL,
    server_type ENUM('http_proxy', 'socks_proxy', 'sni_proxy', 'tcp_tproxy', 'http_rproxy', 'tcp_stream', 'tls_stream') NOT NULL,
    config JSON NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    UNIQUE KEY uk_tenant_server (tenant_id, server_name)
);

-- Escaper Configuration
CREATE TABLE escaper_configs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    escaper_name VARCHAR(255) NOT NULL,
    escaper_type ENUM('direct_fixed', 'direct_float', 'proxy_http', 'proxy_https', 'proxy_socks5', 'route_upstream', 'route_geoip', 'route_client', 'route_failover') NOT NULL,
    config JSON NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    UNIQUE KEY uk_tenant_escaper (tenant_id, escaper_name)
);

-- User Management
CREATE TABLE user_groups (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    group_name VARCHAR(255) NOT NULL,
    config JSON NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    UNIQUE KEY uk_tenant_group (tenant_id, group_name)
);

CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    user_group_id BIGINT NOT NULL,
    username VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255),
    salt VARCHAR(255),
    permissions JSON,
    rate_limits JSON,
    explicit_sites JSON,
    blocked BOOLEAN DEFAULT FALSE,
    block_config JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    FOREIGN KEY (user_group_id) REFERENCES user_groups(id) ON DELETE CASCADE,
    UNIQUE KEY uk_tenant_username (tenant_id, username)
);

CREATE INDEX idx_users_group ON users(user_group_id);
CREATE INDEX idx_users_blocked ON users(blocked);

-- Policy Configuration
CREATE TABLE policies (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    policy_name VARCHAR(255) NOT NULL,
    policy_type ENUM('url_filter', 'content_filter', 'time_restriction', 'ip_filter') NOT NULL,
    config JSON NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    priority INT DEFAULT 100,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    UNIQUE KEY uk_tenant_policy (tenant_id, policy_name)
);

CREATE INDEX idx_policies_type ON policies(policy_type);
CREATE INDEX idx_policies_priority ON policies(priority);

-- Audit Logs
CREATE TABLE audit_logs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    user_id VARCHAR(255),
    event_type ENUM('connection', 'request', 'response', 'block', 'allow', 'error') NOT NULL,
    source_ip INET,
    target_host VARCHAR(255),
    target_port INT,
    method VARCHAR(10),
    url TEXT,
    user_agent TEXT,
    response_code INT,
    bytes_sent BIGINT DEFAULT 0,
    bytes_received BIGINT DEFAULT 0,
    duration_ms INT,
    blocked BOOLEAN DEFAULT FALSE,
    block_reason VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSON,
    INDEX idx_audit_tenant (tenant_id),
    INDEX idx_audit_user (user_id),
    INDEX idx_audit_created (created_at),
    INDEX idx_audit_type (event_type),
    INDEX idx_audit_blocked (blocked)
);

-- Metrics Storage (Time Series)
CREATE TABLE metrics_data (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255),
    metric_name VARCHAR(255) NOT NULL,
    metric_type ENUM('counter', 'gauge', 'histogram', 'timer') NOT NULL,
    value DOUBLE NOT NULL,
    tags JSON,
    timestamp TIMESTAMP(3) DEFAULT CURRENT_TIMESTAMP(3),
    INDEX idx_metrics_tenant (tenant_id),
    INDEX idx_metrics_name (metric_name),
    INDEX idx_metrics_timestamp (timestamp),
    INDEX idx_metrics_tenant_name_time (tenant_id, metric_name, timestamp)
);

-- Partitioning for large tables
ALTER TABLE audit_logs PARTITION BY RANGE (UNIX_TIMESTAMP(created_at)) (
    PARTITION p202501 VALUES LESS THAN (UNIX_TIMESTAMP('2025-02-01')),
    PARTITION p202502 VALUES LESS THAN (UNIX_TIMESTAMP('2025-03-01')),
    PARTITION p202503 VALUES LESS THAN (UNIX_TIMESTAMP('2025-04-01')),
    PARTITION p202504 VALUES LESS THAN (UNIX_TIMESTAMP('2025-05-01')),
    PARTITION p_future VALUES LESS THAN MAXVALUE
);

ALTER TABLE metrics_data PARTITION BY RANGE (UNIX_TIMESTAMP(timestamp)) (
    PARTITION m202501 VALUES LESS THAN (UNIX_TIMESTAMP('2025-02-01')),
    PARTITION m202502 VALUES LESS THAN (UNIX_TIMESTAMP('2025-03-01')),
    PARTITION m202503 VALUES LESS THAN (UNIX_TIMESTAMP('2025-04-01')),
    PARTITION m202504 VALUES LESS THAN (UNIX_TIMESTAMP('2025-05-01')),
    PARTITION m_future VALUES LESS THAN MAXVALUE
);
```

### 10.2 Redis Cache Schema

```redis
# Tenant Configuration Cache
SET tenant:config:tenant-a '{"tenant_id":"tenant-a","config":{...}}'
EXPIRE tenant:config:tenant-a 3600

# User Session Cache
HSET user:session:token123 user_id "user1" tenant_id "tenant-a" expires_at 1672531200
EXPIRE user:session:token123 3600

# Rate Limiting Buckets
SET ratelimit:tenant:tenant-a:requests 100
EXPIRE ratelimit:tenant:tenant-a:requests 60
SET ratelimit:user:tenant-a:user1:requests 10
EXPIRE ratelimit:user:tenant-a:user1:requests 60

# Connection Tracking
SADD connections:active:tenant-a "conn1" "conn2" "conn3"
HSET connection:conn1 client_ip "192.168.1.100" target_host "example.com" created_at 1672531200
EXPIRE connection:conn1 300

# Certificate Cache
SET cert:cache:example.com '{"cert":"...","key":"...","expires_at":1672531200}'
EXPIRE cert:cache:example.com 86400

# GeoIP Cache
SET geoip:1.1.1.1 '{"country":"US","city":"San Francisco","region":"CA"}'
EXPIRE geoip:1.1.1.1 3600

# DNS Cache
SET dns:example.com:A '["1.2.3.4","5.6.7.8"]'
EXPIRE dns:example.com:A 300

# Metrics Aggregation
HINCRBY metrics:tenant:tenant-a:requests total 1
HINCRBY metrics:tenant:tenant-a:requests success 1
EXPIRE metrics:tenant:tenant-a:requests 300

# Configuration Version Control
SET config:version:tenant-a 5
SET config:version:tenant-a:4 '{"servers":{...},"timestamp":"2025-01-01T12:00:00Z"}'
EXPIRE config:version:tenant-a:4 86400
```

### 10.3 Data Access Layer

```rust
pub struct DatabaseManager {
    mysql_pool: Pool<MySql>,
    redis_pool: Pool<Redis>,
    config: DatabaseConfig,
}

impl DatabaseManager {
    pub async fn create_tenant(&self, tenant: &Tenant) -> Result<(), DatabaseError> {
        let mut tx = self.mysql_pool.begin().await?;
        
        // Insert tenant record
        sqlx::query!(
            "INSERT INTO tenants (tenant_id, tenant_name, config, resource_limits, metadata) VALUES (?, ?, ?, ?, ?)",
            tenant.tenant_id,
            tenant.tenant_name,
            serde_json::to_string(&tenant.config)?,
            serde_json::to_string(&tenant.resource_limits)?,
            serde_json::to_string(&tenant.metadata)?
        ).execute(&mut *tx).await?;
        
        // Insert server configurations
        for (name, server_config) in &tenant.config.servers {
            sqlx::query!(
                "INSERT INTO server_configs (tenant_id, server_name, server_type, config) VALUES (?, ?, ?, ?)",
                tenant.tenant_id,
                name,
                server_config.server_type.to_string(),
                serde_json::to_string(server_config)?
            ).execute(&mut *tx).await?;
        }
        
        // Insert escaper configurations
        for (name, escaper_config) in &tenant.config.escapers {
            sqlx::query!(
                "INSERT INTO escaper_configs (tenant_id, escaper_name, escaper_type, config) VALUES (?, ?, ?, ?)",
                tenant.tenant_id,
                name,
                escaper_config.escaper_type.to_string(),
                serde_json::to_string(escaper_config)?
            ).execute(&mut *tx).await?;
        }
        
        tx.commit().await?;
        
        // Cache tenant configuration
        self.cache_tenant_config(tenant).await?;
        
        Ok(())
    }
    
    pub async fn get_tenant(&self, tenant_id: &str) -> Result<Option<Tenant>, DatabaseError> {
        // Try cache first
        if let Some(cached_tenant) = self.get_cached_tenant(tenant_id).await? {
            return Ok(Some(cached_tenant));
        }
        
        // Query from database
        let tenant_record = sqlx::query!(
            "SELECT * FROM tenants WHERE tenant_id = ?",
            tenant_id
        ).fetch_optional(&self.mysql_pool).await?;
        
        if let Some(record) = tenant_record {
            let config: TenantConfig = serde_json::from_str(&record.config)?;
            let resource_limits: ResourceLimits = serde_json::from_str(&record.resource_limits.unwrap_or_else(|| "{}".to_string()))?;
            let metadata: serde_json::Value = serde_json::from_str(&record.metadata.unwrap_or_else(|| "{}".to_string()))?;
            
            let tenant = Tenant {
                tenant_id: record.tenant_id,
                tenant_name: record.tenant_name,
                status: record.status.parse()?,
                config,
                resource_limits,
                metadata,
                created_at: record.created_at,
                updated_at: record.updated_at,
            };
            
            // Cache for future requests
            self.cache_tenant_config(&tenant).await?;
            
            Ok(Some(tenant))
        } else {
            Ok(None)
        }
    }
    
    async fn cache_tenant_config(&self, tenant: &Tenant) -> Result<(), DatabaseError> {
        let mut conn = self.redis_pool.get().await?;
        let cache_key = format!("tenant:config:{}", tenant.tenant_id);
        let tenant_json = serde_json::to_string(tenant)?;
        
        conn.set_ex(&cache_key, tenant_json, 3600).await?;
        
        Ok(())
    }
    
    pub async fn log_audit_event(&self, event: &AuditEvent) -> Result<(), DatabaseError> {
        // Insert into database
        sqlx::query!(
            "INSERT INTO audit_logs (tenant_id, user_id, event_type, source_ip, target_host, target_port, method, url, user_agent, response_code, bytes_sent, bytes_received, duration_ms, blocked, block_reason, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            event.tenant_id,
            event.user_id,
            event.event_type.to_string(),
            event.source_ip.map(|ip| ip.to_string()),
            event.target_host,
            event.target_port,
            event.method,
            event.url,
            event.user_agent,
            event.response_code,
            event.bytes_sent,
            event.bytes_received,
            event.duration_ms,
            event.blocked,
            event.block_reason,
            serde_json::to_string(&event.metadata)?
        ).execute(&self.mysql_pool).await?;
        
        Ok(())
    }
    
    pub async fn store_metric(&self, metric: &Metric) -> Result<(), DatabaseError> {
        // Store in time series database
        sqlx::query!(
            "INSERT INTO metrics_data (tenant_id, metric_name, metric_type, value, tags) VALUES (?, ?, ?, ?, ?)",
            metric.tenant_id,
            metric.name,
            metric.metric_type.to_string(),
            metric.value,
            serde_json::to_string(&metric.tags)?
        ).execute(&self.mysql_pool).await?;
        
        // Also aggregate in Redis for real-time queries
        if let Some(tenant_id) = &metric.tenant_id {
            let mut conn = self.redis_pool.get().await?;
            let key = format!("metrics:{}:{}", tenant_id, metric.name);
            
            match metric.metric_type {
                MetricType::Counter => {
                    conn.hincrbyfloat(&key, "value", metric.value).await?;
                }
                MetricType::Gauge => {
                    conn.hset(&key, "value", metric.value).await?;
                }
                _ => {}
            }
            
            conn.expire(&key, 300).await?;
        }
        
        Ok(())
    }
}
```

---

## 11. Deployment Architecture

### 11.1 Kubernetes Deployment

```yaml
# Namespace
apiVersion: v1
kind: Namespace
metadata:
  name: g3proxy
  labels:
    name: g3proxy

---
# ConfigMap for global configuration
apiVersion: v1
kind: ConfigMap
metadata:
  name: g3proxy-global-config
  namespace: g3proxy
data:
  main.yaml: |
    runtime:
      thread_number: 8
    worker:
      thread_number: 8
      sched_affinity: true
    controller:
      local:
        recv_timeout: 30
        send_timeout: 1
    log:
      target: journal
      level: info
    stat:
      target:
        udp: "g3statsd:8125"
      prefix: "g3proxy"
      emit_interval: 30s

---
# G3Proxy Deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: g3proxy
  namespace: g3proxy
  labels:
    app: g3proxy
    component: proxy
spec:
  replicas: 3
  selector:
    matchLabels:
      app: g3proxy
      component: proxy
  template:
    metadata:
      labels:
        app: g3proxy
        component: proxy
    spec:
      serviceAccountName: g3proxy
      containers:
      - name: g3proxy
        image: g3proxy:v1.12.2
        ports:
        - containerPort: 8080
          name: http-proxy
          protocol: TCP
        - containerPort: 8443
          name: https-proxy
          protocol: TCP
        - containerPort: 1080
          name: socks-proxy
          protocol: TCP
        - containerPort: 9090
          name: metrics
          protocol: TCP
        env:
        - name: G3PROXY_CONFIG
          value: "/config/main.yaml"
        - name: G3PROXY_LOG_LEVEL
          value: "info"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        volumeMounts:
        - name: config
          mountPath: /config
          readOnly: true
        - name: tenant-configs
          mountPath: /config/tenant-configs
          readOnly: true
        - name: certificates
          mountPath: /certificates
          readOnly: true
        - name: tmp
          mountPath: /tmp
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 9090
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 9090
          initialDelaySeconds: 5
          periodSeconds: 5
        lifecycle:
          preStop:
            exec:
              command:
              - /usr/bin/g3proxy-ctl
              - shutdown
              - --graceful
      volumes:
      - name: config
        configMap:
          name: g3proxy-global-config
      - name: tenant-configs
        configMap:
          name: g3proxy-tenant-configs
      - name: certificates
        secret:
          secretName: g3proxy-certificates
      - name: tmp
        emptyDir: {}

---
# G3StatsD Deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: g3statsd
  namespace: g3proxy
  labels:
    app: g3statsd
spec:
  replicas: 1
  selector:
    matchLabels:
      app: g3statsd
  template:
    metadata:
      labels:
        app: g3statsd
    spec:
      containers:
      - name: g3statsd
        image: g3statsd:v1.12.2
        ports:
        - containerPort: 8125
          name: statsd-udp
          protocol: UDP
        - containerPort: 9091
          name: metrics
          protocol: TCP
        volumeMounts:
        - name: config
          mountPath: /config
          readOnly: true
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
      volumes:
      - name: config
        configMap:
          name: g3statsd-config

---
# Services
apiVersion: v1
kind: Service
metadata:
  name: g3proxy
  namespace: g3proxy
  labels:
    app: g3proxy
spec:
  type: LoadBalancer
  selector:
    app: g3proxy
    component: proxy
  ports:
  - name: http-proxy
    port: 8080
    targetPort: 8080
    protocol: TCP
  - name: https-proxy
    port: 8443
    targetPort: 8443
    protocol: TCP
  - name: socks-proxy
    port: 1080
    targetPort: 1080
    protocol: TCP

---
apiVersion: v1
kind: Service
metadata:
  name: g3statsd
  namespace: g3proxy
  labels:
    app: g3statsd
spec:
  selector:
    app: g3statsd
  ports:
  - name: statsd-udp
    port: 8125
    targetPort: 8125
    protocol: UDP
  - name: metrics
    port: 9091
    targetPort: 9091
    protocol: TCP

---
# HorizontalPodAutoscaler
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: g3proxy-hpa
  namespace: g3proxy
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: g3proxy
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  - type: Pods
    pods:
      metric:
        name: g3proxy_connections_per_second
      target:
        type: AverageValue
        averageValue: "100"

---
# NetworkPolicy
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: g3proxy-network-policy
  namespace: g3proxy
spec:
  podSelector:
    matchLabels:
      app: g3proxy
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: default
    ports:
    - protocol: TCP
      port: 8080
    - protocol: TCP
      port: 8443
    - protocol: TCP
      port: 1080
  egress:
  - {} # Allow all egress traffic for proxy functionality

---
# ServiceMonitor for Prometheus
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: g3proxy-metrics
  namespace: g3proxy
  labels:
    app: g3proxy
spec:
  selector:
    matchLabels:
      app: g3proxy
  endpoints:
  - port: metrics
    path: /metrics
    interval: 30s
```

### 11.2 Helm Chart Structure

```yaml
# Chart.yaml
apiVersion: v2
name: g3proxy
description: A Helm chart for G3 Secure Web Gateway
type: application
version: 1.0.0
appVersion: "1.12.2"
dependencies:
- name: redis
  version: "17.3.14"
  repository: https://charts.bitnami.com/bitnami
  condition: redis.enabled
- name: mysql
  version: "9.4.6"
  repository: https://charts.bitnami.com/bitnami
  condition: mysql.enabled

---
# values.yaml
global:
  imageRegistry: ""
  imagePullSecrets: []

image:
  registry: docker.io
  repository: g3proxy/g3proxy
  tag: "v1.12.2"
  pullPolicy: IfNotPresent

replicaCount: 3

service:
  type: LoadBalancer
  ports:
    http: 8080
    https: 8443
    socks: 1080

ingress:
  enabled: false
  className: ""
  annotations: {}
  hosts:
    - host: g3proxy.example.com
      paths:
        - path: /
          pathType: Prefix

resources:
  limits:
    cpu: 1000m
    memory: 1Gi
  requests:
    cpu: 500m
    memory: 512Mi

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 20
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80

nodeSelector: {}
tolerations: []
affinity: {}

config:
  runtime:
    thread_number: 8
  worker:
    thread_number: 8
    sched_affinity: true
  log:
    level: info
    target: journal

tenants:
  - tenant_id: "default"
    tenant_name: "Default Tenant"
    servers:
      - name: "http-proxy"
        type: "http_proxy"
        listen:
          address: "[::]:8080"
    escapers:
      - name: "default"
        type: "direct_fixed"

redis:
  enabled: true
  auth:
    enabled: false
  master:
    persistence:
      enabled: true
      size: 8Gi

mysql:
  enabled: true
  auth:
    database: "g3proxy"
    username: "g3proxy"
  primary:
    persistence:
      enabled: true
      size: 20Gi

monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    namespace: monitoring
  prometheusRule:
    enabled: true
```

### 11.3 Docker Compose for Development

```yaml
# docker-compose.yml
version: '3.8'

services:
  g3proxy:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
      - "8443:8443"
      - "1080:1080"
      - "9090:9090"
    volumes:
      - ./config:/config:ro
      - ./certificates:/certificates:ro
      - g3proxy_data:/data
    environment:
      - G3PROXY_CONFIG=/config/main.yaml
      - RUST_LOG=info
    depends_on:
      - redis
      - mysql
      - g3statsd
    networks:
      - g3proxy-network

  g3statsd:
    build:
      context: .
      dockerfile: Dockerfile.g3statsd
    ports:
      - "8125:8125/udp"
      - "9091:9091"
    volumes:
      - ./config/g3statsd.yaml:/config/main.yaml:ro
    networks:
      - g3proxy-network

  g3fcgen:
    build:
      context: .
      dockerfile: Dockerfile.g3fcgen
    ports:
      - "2999:2999"
    volumes:
      - ./certificates:/certificates
    environment:
      - G3FCGEN_CA_CERT=/certificates/ca.crt
      - G3FCGEN_CA_KEY=/certificates/ca.key
    networks:
      - g3proxy-network

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes
    networks:
      - g3proxy-network

  mysql:
    image: mysql:8.0
    ports:
      - "3306:3306"
    environment:
      - MYSQL_ROOT_PASSWORD=rootpass
      - MYSQL_DATABASE=g3proxy
      - MYSQL_USER=g3proxy
      - MYSQL_PASSWORD=g3proxypass
    volumes:
      - mysql_data:/var/lib/mysql
      - ./sql/init.sql:/docker-entrypoint-initdb.d/init.sql:ro
    networks:
      - g3proxy-network

  clamav:
    image: clamav/clamav:latest
    ports:
      - "3310:3310"
    volumes:
      - clamav_data:/var/lib/clamav
    networks:
      - g3proxy-network

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9094:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
    networks:
      - g3proxy-network

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana/dashboards:/var/lib/grafana/dashboards:ro
      - ./monitoring/grafana/provisioning:/etc/grafana/provisioning:ro
    networks:
      - g3proxy-network

volumes:
  g3proxy_data:
  redis_data:
  mysql_data:
  clamav_data:
  prometheus_data:
  grafana_data:

networks:
  g3proxy-network:
    driver: bridge
```

---

## 12. Error Handling & Recovery

### 12.1 Error Classification and Handling

```rust
// Error Hierarchy
#[derive(Debug, thiserror::Error)]
pub enum G3ProxyError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),
    
    #[error("TLS error: {0}")]
    Tls(#[from] TlsError),
    
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),
    
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Configuration not found: {0}")]
    NotFound(String),
    
    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Connection timeout: {0}")]
    Timeout(String),
    
    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Error Recovery Strategies
pub enum RecoveryStrategy {
    Retry { max_attempts: usize, backoff: Duration },
    Fallback { fallback_action: Box<dyn FallbackAction> },
    CircuitBreaker { failure_threshold: usize, timeout: Duration },
    Ignore,
    Fail,
}

pub trait ErrorHandler: Send + Sync {
    fn can_handle(&self, error: &G3ProxyError) -> bool;
    fn handle(&self, error: G3ProxyError, context: &ErrorContext) -> BoxFuture<'_, RecoveryAction>;
}

#[derive(Debug)]
pub struct ErrorContext {
    pub component: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub connection_id: Option<String>,
    pub retry_count: usize,
    pub timestamp: SystemTime,
}

pub enum RecoveryAction {
    Retry(Duration),
    Fallback(String),
    Fail(G3ProxyError),
    Ignore,
}

// Connection Error Handler
pub struct ConnectionErrorHandler {
    max_retries: usize,
    backoff_multiplier: f64,
    max_backoff: Duration,
}

impl ErrorHandler for ConnectionErrorHandler {
    fn can_handle(&self, error: &G3ProxyError) -> bool {
        matches!(error, G3ProxyError::Network(NetworkError::ConnectionFailed(_)) | 
                       G3ProxyError::Network(NetworkError::Timeout(_)))
    }
    
    async fn handle(&self, error: G3ProxyError, context: &ErrorContext) -> RecoveryAction {
        if context.retry_count >= self.max_retries {
            error!("Max retries exceeded for connection error: {:?}", error);
            return RecoveryAction::Fail(error);
        }
        
        let backoff_duration = Duration::from_millis(
            (100.0 * self.backoff_multiplier.powi(context.retry_count as i32)) as u64
        ).min(self.max_backoff);
        
        warn!(
            "Connection failed, retrying in {:?} (attempt {}/{}): {:?}",
            backoff_duration, context.retry_count + 1, self.max_retries, error
        );
        
        RecoveryAction::Retry(backoff_duration)
    }
}

// Circuit Breaker Implementation
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_threshold: usize,
    timeout: Duration,
}

#[derive(Debug, Clone)]
enum CircuitBreakerState {
    Closed,
    Open { opened_at: Instant },
    HalfOpen,
}

impl CircuitBreaker {
    pub async fn call<F, R, E>(&self, f: F) -> Result<R, CircuitBreakerError<E>>
    where
        F: Future<Output = Result<R, E>>,
        E: std::error::Error,
    {
        // Check circuit breaker state
        match *self.state.read().await {
            CircuitBreakerState::Open { opened_at } => {
                if opened_at.elapsed() > self.timeout {
                    // Transition to half-open
                    *self.state.write().await = CircuitBreakerState::HalfOpen;
                } else {
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Allow one request through
            }
            CircuitBreakerState::Closed => {
                // Normal operation
            }
        }
        
        // Execute the function
        match f.await {
            Ok(result) => {
                // Success - reset to closed state if half-open
                if matches!(*self.state.read().await, CircuitBreakerState::HalfOpen) {
                    *self.state.write().await = CircuitBreakerState::Closed;
                }
                Ok(result)
            }
            Err(e) => {
                // Failure - potentially open circuit
                let mut state = self.state.write().await;
                match *state {
                    CircuitBreakerState::HalfOpen => {
                        // Failure in half-open state - go back to open
                        *state = CircuitBreakerState::Open { opened_at: Instant::now() };
                    }
                    CircuitBreakerState::Closed => {
                        // Track failures and potentially open circuit
                        // (Implementation would track failure count)
                        if self.should_open_circuit() {
                            *state = CircuitBreakerState::Open { opened_at: Instant::now() };
                        }
                    }
                    _ => {}
                }
                Err(CircuitBreakerError::ExecutionFailed(e))
            }
        }
    }
}
```

### 12.2 Health Monitoring and Recovery

```rust
pub struct HealthMonitor {
    checks: Vec<Box<dyn HealthCheck>>,
    recovery_manager: Arc<RecoveryManager>,
    check_interval: Duration,
}

pub trait HealthCheck: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self) -> BoxFuture<'_, HealthStatus>;
}

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

// Database Health Check
pub struct DatabaseHealthCheck {
    database: Arc<DatabaseManager>,
}

impl HealthCheck for DatabaseHealthCheck {
    fn name(&self) -> &str {
        "database"
    }
    
    async fn check(&self) -> HealthStatus {
        match self.database.ping().await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy {
                reason: format!("Database connection failed: {}", e),
            },
        }
    }
}

// Redis Health Check
pub struct RedisHealthCheck {
    redis_pool: Arc<redis::Pool>,
}

impl HealthCheck for RedisHealthCheck {
    fn name(&self) -> &str {
        "redis"
    }
    
    async fn check(&self) -> HealthStatus {
        match self.redis_pool.get().await {
            Ok(mut conn) => {
                match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
                    Ok(_) => HealthStatus::Healthy,
                    Err(e) => HealthStatus::Degraded {
                        reason: format!("Redis PING failed: {}", e),
                    },
                }
            }
            Err(e) => HealthStatus::Unhealthy {
                reason: format!("Redis connection failed: {}", e),
            },
        }
    }
}

// Upstream Health Check
pub struct UpstreamHealthCheck {
    upstreams: Arc<RwLock<HashMap<String, Upstream>>>,
}

impl HealthCheck for UpstreamHealthCheck {
    fn name(&self) -> &str {
        "upstreams"
    }
    
    async fn check(&self) -> HealthStatus {
        let upstreams = self.upstreams.read().await;
        let mut healthy_count = 0;
        let mut total_count = 0;
        
        for upstream in upstreams.values() {
            total_count += 1;
            if upstream.is_healthy().await {
                healthy_count += 1;
            }
        }
        
        if total_count == 0 {
            return HealthStatus::Unhealthy {
                reason: "No upstreams configured".to_string(),
            };
        }
        
        let health_ratio = healthy_count as f64 / total_count as f64;
        
        if health_ratio >= 0.8 {
            HealthStatus::Healthy
        } else if health_ratio >= 0.5 {
            HealthStatus::Degraded {
                reason: format!("Only {}/{} upstreams are healthy", healthy_count, total_count),
            }
        } else {
            HealthStatus::Unhealthy {
                reason: format!("Only {}/{} upstreams are healthy", healthy_count, total_count),
            }
        }
    }
}

// Recovery Manager
pub struct RecoveryManager {
    recovery_strategies: HashMap<String, Box<dyn RecoveryStrategy>>,
    notification_client: Arc<dyn NotificationClient>,
}

pub trait RecoveryStrategy: Send + Sync {
    fn can_recover(&self, component: &str, status: &HealthStatus) -> bool;
    fn recover(&self, component: &str, status: &HealthStatus) -> BoxFuture<'_, Result<(), RecoveryError>>;
}

// Service Restart Recovery Strategy
pub struct ServiceRestartStrategy {
    max_restarts: usize,
    restart_window: Duration,
    restart_history: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl RecoveryStrategy for ServiceRestartStrategy {
    fn can_recover(&self, component: &str, status: &HealthStatus) -> bool {
        matches!(status, HealthStatus::Unhealthy { .. })
    }
    
    async fn recover(&self, component: &str, _status: &HealthStatus) -> Result<(), RecoveryError> {
        // Check restart rate limiting
        let mut history = self.restart_history.write().await;
        let now = Instant::now();
        
        let component_history = history.entry(component.to_string()).or_default();
        
        // Remove old restart records outside the window
        component_history.retain(|&time| now.duration_since(time) < self.restart_window);
        
        if component_history.len() >= self.max_restarts {
            return Err(RecoveryError::RateLimited(
                format!("Component {} has been restarted {} times within {:?}", 
                        component, self.max_restarts, self.restart_window)
            ));
        }
        
        // Record this restart
        component_history.push(now);
        
        // Perform restart based on component type
        match component {
            "database" => {
                warn!("Database health check failed, attempting reconnection");
                // Trigger database reconnection
                // Implementation specific to database client
            }
            "redis" => {
                warn!("Redis health check failed, clearing connection pool");
                // Clear Redis connection pool
                // Implementation specific to Redis client
            }
            service_name => {
                warn!("Service {} health check failed, requesting restart", service_name);
                // Send restart signal to service
                // Implementation specific to service management
            }
        }
        
        Ok(())
    }
}

impl HealthMonitor {
    pub async fn start_monitoring(&self) {
        let mut interval = tokio::time::interval(self.check_interval);
        
        loop {
            interval.tick().await;
            
            let mut health_results = HashMap::new();
            
            // Run all health checks concurrently
            let check_futures: Vec<_> = self.checks.iter()
                .map(|check| async move {
                    let start = Instant::now();
                    let status = check.check().await;
                    let duration = start.elapsed();
                    (check.name().to_string(), status, duration)
                })
                .collect();
            
            let results = futures::future::join_all(check_futures).await;
            
            for (name, status, duration) in results {
                health_results.insert(name.clone(), status.clone());
                
                // Log health check results
                match &status {
                    HealthStatus::Healthy => {
                        debug!("Health check {} passed in {:?}", name, duration);
                    }
                    HealthStatus::Degraded { reason } => {
                        warn!("Health check {} degraded in {:?}: {}", name, duration, reason);
                    }
                    HealthStatus::Unhealthy { reason } => {
                        error!("Health check {} failed in {:?}: {}", name, duration, reason);
                        
                        // Attempt recovery
                        if let Err(e) = self.recovery_manager.attempt_recovery(&name, &status).await {
                            error!("Recovery failed for {}: {:?}", name, e);
                        }
                    }
                }
            }
            
            // Update overall system health status
            self.update_system_health(&health_results).await;
        }
    }
}
```

This completes the comprehensive Low-Level Design document for the G3-based Secure Web Gateway. The LLD covers all major aspects of the system including architecture, component design, data flows, multi-tenancy, security, performance, configuration, monitoring, APIs, database design, deployment, and error handling with recovery mechanisms.