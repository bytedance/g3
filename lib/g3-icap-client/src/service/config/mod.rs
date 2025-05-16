/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::io::Write;
use std::time::Duration;

use anyhow::anyhow;
use http::HeaderName;
use rustls_pki_types::ServerName;
use url::Url;

use g3_types::net::{
    ConnectionPoolConfig, HttpAuth, RustlsClientConfigBuilder, TcpKeepAliveConfig, UpstreamAddr,
};

#[cfg(feature = "yaml")]
mod yaml;

use super::IcapMethod;

pub struct IcapServiceConfig {
    pub(crate) method: IcapMethod,
    url: Url,
    auth: HttpAuth,
    user_agent: Option<String>,
    pub(crate) upstream: UpstreamAddr,
    pub(crate) tls_client: Option<RustlsClientConfigBuilder>,
    pub(crate) tls_name: ServerName<'static>,
    pub connection_pool: ConnectionPoolConfig,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) icap_206_enable: bool,
    pub(crate) icap_max_header_size: usize,
    pub(crate) preview_data_read_timeout: Duration,
    pub(crate) respond_shared_names: BTreeSet<String>,
    pub(crate) bypass: bool,
}

impl IcapServiceConfig {
    pub fn new(method: IcapMethod, mut url: Url) -> anyhow::Result<Self> {
        let tls_client = match url.scheme().to_ascii_lowercase().as_str() {
            "icap" => None,
            "icaps" => Some(RustlsClientConfigBuilder::default()),
            _ => return Err(anyhow!("unsupported ICAP URL scheme: {}", url.scheme())),
        };

        if !url.has_authority() {
            return Err(anyhow!("no authority part found in this url"));
        }
        let auth = HttpAuth::try_from(&url).map_err(|e| anyhow!("invalid auth info: {e}"))?;
        url.set_username("")
            .map_err(|_| anyhow!("failed to clear username in url"))?;
        url.set_password(None)
            .map_err(|_| anyhow!("failed to clear password in url"))?;

        let upstream = UpstreamAddr::try_from(&url)
            .map_err(|e| anyhow!("failed to get upstream address from url: {e}"))?;
        let tls_name = ServerName::try_from(upstream.host())
            .map_err(|e| anyhow!("invalid ICAP server name: {e}"))?;
        Ok(IcapServiceConfig {
            method,
            url,
            auth,
            user_agent: None,
            upstream,
            tls_client,
            tls_name,
            connection_pool: ConnectionPoolConfig::default(),
            tcp_keepalive: TcpKeepAliveConfig::default_enabled(),
            icap_206_enable: false,
            icap_max_header_size: 8192,
            preview_data_read_timeout: Duration::from_secs(4),
            respond_shared_names: BTreeSet::new(),
            bypass: false,
        })
    }

    pub fn set_tcp_keepalive(&mut self, config: TcpKeepAliveConfig) {
        self.tcp_keepalive = config;
    }

    pub fn set_tls_client(&mut self, config: RustlsClientConfigBuilder) {
        self.tls_client = Some(config);
    }

    pub fn set_tls_name(&mut self, name: ServerName<'static>) {
        self.tls_name = name;
    }

    pub fn set_icap_max_header_size(&mut self, max_size: usize) {
        self.icap_max_header_size = max_size;
    }

    pub fn set_preview_data_read_timeout(&mut self, time: Duration) {
        self.preview_data_read_timeout = time;
    }

    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypass = bypass;
    }

    pub fn add_respond_shared_name(&mut self, name: HeaderName) {
        self.respond_shared_names.insert(name.as_str().to_string());
    }

    pub(crate) fn build_request_header(&self) -> Vec<u8> {
        let mut header = Vec::with_capacity(1024);
        self.write_header(&mut header, self.method.as_str());
        header
    }

    pub(crate) fn build_options_request(&self) -> Vec<u8> {
        let mut header = Vec::with_capacity(256);
        self.write_header(&mut header, "OPTIONS");
        header
    }

    fn write_header(&self, header: &mut Vec<u8>, method: &str) {
        let _ = write!(header, "{method} {} ICAP/1.0\r\n", self.url);
        if let Some(host) = self.url.host_str() {
            let _ = write!(header, "Host: {host}\r\n");
        }
        if let Some(user_agent) = &self.user_agent {
            let _ = write!(header, "User-Agent: {user_agent}\r\n");
        }
        match &self.auth {
            HttpAuth::None => {}
            HttpAuth::Basic(basic_auth) => {
                let _ = write!(
                    header,
                    "Authorization: Basic {}\r\n",
                    basic_auth.encoded_value()
                );
            }
        }
    }
}
