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

use std::collections::BTreeSet;
use std::io::Write;
use std::time::Duration;

use anyhow::anyhow;
use http::HeaderName;
use url::Url;

use g3_types::net::{HttpAuth, TcpKeepAliveConfig, UpstreamAddr};

use super::IcapMethod;

pub struct IcapConnectionPoolConfig {
    pub(crate) check_interval: Duration,
    pub(crate) max_idle_count: usize,
    pub(crate) min_idle_count: usize,
}

impl Default for IcapConnectionPoolConfig {
    fn default() -> Self {
        IcapConnectionPoolConfig {
            check_interval: Duration::from_secs(10),
            max_idle_count: 128,
            min_idle_count: 16,
        }
    }
}

impl IcapConnectionPoolConfig {
    #[inline]
    pub fn set_check_interval(&mut self, interval: Duration) {
        self.check_interval = interval;
    }

    #[inline]
    pub fn set_max_idle_count(&mut self, count: usize) {
        self.max_idle_count = count;
    }

    #[inline]
    pub fn set_min_idle_count(&mut self, count: usize) {
        self.min_idle_count = count;
    }
}

pub struct IcapServiceConfig {
    pub(crate) method: IcapMethod,
    url: Url,
    auth: HttpAuth,
    user_agent: Option<String>,
    pub(crate) upstream: UpstreamAddr,
    pub connection_pool: IcapConnectionPoolConfig,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) icap_206_enable: bool,
    pub(crate) icap_max_header_size: usize,
    pub(crate) preview_data_read_timeout: Duration,
    pub(crate) respond_shared_names: BTreeSet<String>,
    pub(crate) bypass: bool,
}

impl IcapServiceConfig {
    pub fn new(method: IcapMethod, mut url: Url) -> anyhow::Result<Self> {
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
        Ok(IcapServiceConfig {
            method,
            url,
            auth,
            user_agent: None,
            upstream,
            connection_pool: IcapConnectionPoolConfig::default(),
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
