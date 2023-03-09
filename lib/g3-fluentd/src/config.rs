/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::{anyhow, Context};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::time::Duration;

use digest::Digest;
use rand::Rng;
use rmp::encode::ValueWriteError;
use sha2::Sha512;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_openssl::SslStream;

use g3_types::net::{OpensslTlsClientConfig, OpensslTlsClientConfigBuilder, TcpKeepAliveConfig};

use super::FluentdConnection;

const FLUENTD_DEFAULT_PORT: u16 = 24224;

#[derive(Clone)]
pub struct FluentdClientConfig {
    server_addr: SocketAddr,
    bind_ip: Option<IpAddr>,
    shared_key: String,
    username: String,
    password: String,
    tcp_keepalive: TcpKeepAliveConfig,
    tls_client: Option<OpensslTlsClientConfig>,
    hostname: String,
    pub(super) connect_timeout: Duration,
    pub(super) connect_delay: Duration,
    pub(super) write_timeout: Duration,
    pub(super) flush_interval: Duration,
    pub(super) retry_queue_len: usize,
}

impl Default for FluentdClientConfig {
    fn default() -> Self {
        FluentdClientConfig::new(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            FLUENTD_DEFAULT_PORT,
        ))
    }
}

impl FluentdClientConfig {
    pub fn new(server: SocketAddr) -> Self {
        let hostname = nix::unistd::gethostname()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        FluentdClientConfig {
            server_addr: server,
            bind_ip: None,
            shared_key: String::new(),
            username: String::new(),
            password: String::new(),
            tcp_keepalive: TcpKeepAliveConfig::default_enabled(),
            tls_client: None,
            hostname,
            connect_timeout: Duration::from_secs(10),
            connect_delay: Duration::from_secs(10),
            write_timeout: Duration::from_secs(1),
            flush_interval: Duration::from_millis(100),
            retry_queue_len: 10,
        }
    }

    pub fn set_server_addr(&mut self, addr: SocketAddr) {
        self.server_addr = addr;
    }

    pub fn set_bind_ip(&mut self, ip: IpAddr) {
        self.bind_ip = Some(ip);
    }

    pub fn set_shared_key(&mut self, key: String) {
        self.shared_key = key;
    }

    pub fn set_username(&mut self, username: String) {
        self.username = username;
    }

    pub fn set_password(&mut self, password: String) {
        self.password = password;
    }

    pub fn set_hostname(&mut self, hostname: String) {
        self.hostname = hostname;
    }

    pub fn set_tcp_keepalive(&mut self, keepalive: TcpKeepAliveConfig) {
        self.tcp_keepalive = keepalive;
    }

    pub fn set_tls_client(
        &mut self,
        tls_config: OpensslTlsClientConfigBuilder,
    ) -> anyhow::Result<()> {
        let tls_client = tls_config
            .build()
            .context("failed to build tls client config")?;
        self.tls_client = Some(tls_client);
        Ok(())
    }

    pub fn set_connect_timeout(&mut self, timeout: Duration) {
        self.connect_timeout = timeout;
    }

    pub fn set_connect_delay(&mut self, delay: Duration) {
        self.connect_delay = delay;
    }

    pub fn set_write_timeout(&mut self, timeout: Duration) {
        self.write_timeout = timeout;
    }

    pub fn set_flush_interval(&mut self, interval: Duration) {
        self.flush_interval = interval;
    }

    pub fn set_retry_queue_len(&mut self, len: usize) {
        self.retry_queue_len = len;
    }

    pub(super) async fn new_connection(&self) -> anyhow::Result<FluentdConnection> {
        let socket = g3_socket::tcp::new_socket_to(
            self.server_addr.ip(),
            self.bind_ip,
            &self.tcp_keepalive,
            &Default::default(),
            false,
        )
        .map_err(|e| anyhow!("failed to setup socket: {e:?}"))?;
        let tcp_stream = socket
            .connect(self.server_addr)
            .await
            .map_err(|e| anyhow!("failed to tcp connect to peer {}: {e:?}", self.server_addr))?;

        if let Some(tls_client) = &self.tls_client {
            let tls_name = self.server_addr.ip().to_string();
            let tls = tls_client
                .build_ssl(&tls_name, self.server_addr.port())
                .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;
            let mut tls_stream = SslStream::new(tls, tcp_stream)
                .map_err(|e| anyhow!("failed to setup ssl context: {e}"))?;
            Pin::new(&mut tls_stream)
                .connect()
                .await
                .map_err(|e| anyhow!("failed to tls connect to peer {tls_name}: {e}"))?;
            let tls_stream = self
                .handshake(tls_stream)
                .await
                .context("fluentd handshake failed")?;
            Ok(FluentdConnection::Tls(tls_stream))
        } else {
            let tcp_stream = self
                .handshake(tcp_stream)
                .await
                .context("fluentd handshake failed")?;
            Ok(FluentdConnection::Tcp(tcp_stream))
        }
    }

    async fn handshake<T>(&self, mut connection: T) -> anyhow::Result<T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if self.shared_key.is_empty() {
            return Ok(connection);
        }

        let mut helo_buf = Vec::with_capacity(1024); // TODO config
        let helo_len = connection
            .read_buf(&mut helo_buf)
            .await
            .map_err(|e| anyhow!("failed to read helo msg: {e:?}"))?;
        let helo = super::handshake::parse_helo(&helo_buf[0..helo_len])
            .context("failed to parse helo msg")?;

        let mut rng = rand::thread_rng();
        let shared_key_salt: [u8; 16] = rng.gen();

        let ping_msg = self
            .build_ping(&helo, &shared_key_salt)
            .map_err(|e| anyhow!("failed to build ping msg: {e}"))?;
        connection
            .write_all(ping_msg.as_slice())
            .await
            .map_err(|e| anyhow!("failed to write ping msg: {e:?}"))?;

        let mut pong_buf = Vec::with_capacity(1024); // TODO config
        let pong_len = connection
            .read_buf(&mut pong_buf)
            .await
            .map_err(|e| anyhow!("failed to read pong msg: {e:?}"))?;
        let pong = super::handshake::parse_pong(&pong_buf[0..pong_len])
            .context("failed to parse pong msg")?;

        self.verify_pong(pong, &shared_key_salt, helo.nonce)?;

        Ok(connection)
    }

    fn build_ping(
        &self,
        helo: &super::handshake::HeloMsgRef<'_>,
        shared_key_salt: &[u8],
    ) -> Result<Vec<u8>, ValueWriteError> {
        let mut buf = Vec::with_capacity(1024);

        rmp::encode::write_array_len(&mut buf, 6)?;
        {
            rmp::encode::write_str(&mut buf, "PING")?;

            rmp::encode::write_str(&mut buf, &self.hostname)?;

            rmp::encode::write_bin(&mut buf, shared_key_salt)?;

            let mut digest = Sha512::new();
            digest.update(shared_key_salt);
            digest.update(self.hostname.as_bytes());
            digest.update(helo.nonce);
            digest.update(self.shared_key.as_bytes());
            let v = &digest.finalize()[..];
            let shared_key_digest = hex::encode(v);
            rmp::encode::write_str(&mut buf, &shared_key_digest)?;

            if helo.nonce.is_empty() {
                rmp::encode::write_str(&mut buf, "")?;

                rmp::encode::write_str(&mut buf, "")?;
            } else {
                rmp::encode::write_str(&mut buf, &self.username)?;

                let mut digest = Sha512::new();
                digest.update(helo.auth_salt);
                digest.update(self.username.as_bytes());
                digest.update(self.password.as_bytes());
                let v = &digest.finalize()[..];
                let password_digest = hex::encode(v);
                rmp::encode::write_str(&mut buf, &password_digest)?;
            }
        }

        Ok(buf)
    }

    fn verify_pong(
        &self,
        pong: super::handshake::PongMsgRef,
        shared_key_salt: &[u8],
        nonce_salt: &[u8],
    ) -> anyhow::Result<()> {
        if !pong.auth_result {
            return Err(anyhow!("server auth failed, reason: {}", pong.reason));
        }

        let remote_v = hex::decode(pong.shared_key_digest)
            .map_err(|_| anyhow!("invalid shared_key_hex_digest returned by server"))?;

        let mut digest = Sha512::new();
        digest.update(shared_key_salt);
        digest.update(pong.server_hostname.as_bytes());
        digest.update(nonce_salt);
        digest.update(self.shared_key.as_bytes());
        let local_v = &digest.finalize()[..];

        if local_v.ne(&remote_v) {
            return Err(anyhow!("shared_key_hex_digest mismatch"));
        }

        Ok(())
    }
}
