/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::anyhow;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, AsyncConnectionConfig, ProtocolVersion, RedisConnectionInfo};
use rustls_pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;

use g3_types::net::{Host, RustlsClientConfig, RustlsClientConfigBuilder, UpstreamAddr};

#[cfg(feature = "yaml")]
mod yaml;

pub const REDIS_DEFAULT_PORT: u16 = 6379;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedisClientConfigBuilder {
    addr: UpstreamAddr,
    tls_client: Option<RustlsClientConfigBuilder>,
    tls_name: Option<ServerName<'static>>,
    db: i64,
    username: Option<String>,
    password: Option<String>,
    connect_timeout: Duration,
    response_timeout: Duration,
}

pub struct RedisClientConfig {
    server: UpstreamAddr,
    tls_client: Option<RustlsClientConfig>,
    tls_name: Option<ServerName<'static>>,
    db_info: RedisConnectionInfo,
    connect_timeout: Duration,
    response_timeout: Duration,
}

impl Default for RedisClientConfigBuilder {
    fn default() -> Self {
        RedisClientConfigBuilder::new(UpstreamAddr::new(
            Host::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            REDIS_DEFAULT_PORT,
        ))
    }
}

impl RedisClientConfigBuilder {
    pub fn new(server: UpstreamAddr) -> Self {
        RedisClientConfigBuilder {
            addr: server,
            tls_client: None,
            tls_name: None,
            db: 0,
            username: None,
            password: None,
            connect_timeout: Duration::from_secs(5),
            response_timeout: Duration::from_secs(2),
        }
    }

    pub fn set_addr(&mut self, addr: UpstreamAddr) {
        self.addr = addr;
    }

    pub fn set_tls_client(&mut self, tls: RustlsClientConfigBuilder) {
        self.tls_client = Some(tls);
    }

    pub fn set_tls_name(&mut self, name: ServerName<'static>) {
        self.tls_name = Some(name);
    }

    pub fn set_db(&mut self, db: i64) {
        self.db = db;
    }

    pub fn set_username(&mut self, name: String) {
        self.username = Some(name);
    }

    pub fn set_password(&mut self, password: String) {
        self.password = Some(password);
    }

    pub fn set_connect_timeout(&mut self, timeout: Duration) {
        self.connect_timeout = timeout;
    }

    pub fn set_response_timeout(&mut self, timeout: Duration) {
        self.response_timeout = timeout;
    }

    pub fn build(&self) -> anyhow::Result<RedisClientConfig> {
        let mut client = RedisClientConfig {
            server: self.addr.clone(),
            tls_client: None,
            tls_name: None,
            db_info: RedisConnectionInfo {
                db: self.db,
                username: self.username.clone(),
                password: self.password.clone(),
                protocol: ProtocolVersion::RESP3,
            },
            connect_timeout: self.connect_timeout,
            response_timeout: self.response_timeout,
        };

        if let Some(config) = &self.tls_client {
            client.tls_client = Some(config.build()?);
            let tls_name = if let Some(name) = &self.tls_name {
                name.clone()
            } else {
                ServerName::try_from(self.addr.host())
                    .map_err(|e| anyhow!("invalid tls server name: {e}"))?
            };
            client.tls_name = Some(tls_name);
        }

        Ok(client)
    }
}

impl RedisClientConfig {
    async fn lookup_server(&self) -> anyhow::Result<SocketAddr> {
        match self.server.host() {
            Host::Domain(domain) => {
                let mut ips = tokio::net::lookup_host((domain.as_ref(), self.server.port()))
                    .await
                    .map_err(|e| anyhow!("failed to resolve domain {domain}: {e}"))?;
                ips.next()
                    .ok_or_else(|| anyhow!("no ip address resolved for domain {domain}"))
            }
            Host::Ip(ip) => Ok(SocketAddr::new(*ip, self.server.port())),
        }
    }

    pub async fn connect(&self) -> anyhow::Result<impl AsyncCommands + use<>> {
        let peer = self.lookup_server().await?;
        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            &Default::default(),
            &Default::default(),
            &Default::default(),
            true,
        )
        .map_err(|e| anyhow!("failed to create new socket: {e}"))?;

        let stream = match tokio::time::timeout(self.connect_timeout, socket.connect(peer)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return Err(anyhow!("failed to connect to {}: {e}", self.server)),
            Err(_) => return Err(anyhow!("timeout to connect to {}", self.server)),
        };

        if let Some(tls_client) = &self.tls_client {
            let tls_connector = TlsConnector::from(tls_client.driver.clone());
            let tls_name = self.tls_name.as_ref().unwrap();
            match tokio::time::timeout(
                tls_client.handshake_timeout,
                tls_connector.connect(tls_name.clone(), stream),
            )
            .await
            {
                Ok(Ok(stream)) => self.redis_handshake(stream).await,
                Ok(Err(e)) => Err(anyhow!("failed to tls handshake with {}: {e}", self.server)),
                Err(_) => Err(anyhow!("timeout to tls handshake with {}", self.server)),
            }
        } else {
            self.redis_handshake(stream).await
        }
    }

    async fn redis_handshake<S>(&self, stream: S) -> anyhow::Result<MultiplexedConnection>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let async_config = AsyncConnectionConfig::new().set_response_timeout(self.response_timeout);

        let (conn, background) =
            MultiplexedConnection::new_with_config(&self.db_info, stream, async_config)
                .await
                .map_err(|e| anyhow!("redis handshake with {} failed: {e}", self.server))?;
        tokio::spawn(background);
        Ok(conn)
    }
}
