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

use std::str::FromStr;

use anyhow::anyhow;
use rustls::ServerName;

use crate::net::{RustlsClientConfig, RustlsClientConfigBuilder};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DnsEncryptionProtocol {
    Tls,
    Https,
    Quic,
}

impl FromStr for DnsEncryptionProtocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "tls" | "dns_over_tls" | "dnsovertls" | "dot" => Ok(DnsEncryptionProtocol::Tls),
            "https" | "dns_over_https" | "dnsoverhttps" | "doh" => Ok(DnsEncryptionProtocol::Https),
            "quic" | "dns_over_quic" | "dnsoverquic" | "doq" => Ok(DnsEncryptionProtocol::Quic),
            _ => Err(anyhow!("unknown protocol {}", s)),
        }
    }
}

impl DnsEncryptionProtocol {
    fn as_str(&self) -> &'static str {
        match self {
            DnsEncryptionProtocol::Tls => "DnsOverTls",
            DnsEncryptionProtocol::Https => "DnsOverHttps",
            DnsEncryptionProtocol::Quic => "DnsOverQuic",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DnsEncryptionConfigBuilder {
    protocol: DnsEncryptionProtocol,
    tls_name: ServerName,
    tls_config: Option<RustlsClientConfigBuilder>,
}

impl DnsEncryptionConfigBuilder {
    pub fn new(tls_name: ServerName) -> Self {
        DnsEncryptionConfigBuilder {
            protocol: DnsEncryptionProtocol::Tls,
            tls_name,
            tls_config: None,
        }
    }

    pub fn set_protocol(&mut self, protocol: DnsEncryptionProtocol) {
        self.protocol = protocol;
    }

    #[inline]
    pub fn protocol(&self) -> DnsEncryptionProtocol {
        self.protocol
    }

    pub fn set_tls_name(&mut self, name: ServerName) {
        self.tls_name = name;
    }

    #[inline]
    pub fn tls_name(&self) -> &ServerName {
        &self.tls_name
    }

    pub fn set_tls_client_config(&mut self, config_builder: RustlsClientConfigBuilder) {
        self.tls_config = Some(config_builder);
    }

    pub fn build_tls_client_config(&self) -> anyhow::Result<Option<RustlsClientConfig>> {
        if let Some(builder) = &self.tls_config {
            let config = builder.build()?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    pub fn summary(&self) -> String {
        match &self.tls_name {
            ServerName::DnsName(n) => format!("{}({})", self.protocol.as_str(), n.as_ref()),
            ServerName::IpAddress(ip) => format!("{}({ip})", self.protocol.as_str()),
            _ => format!("{}(other)", self.protocol.as_str()), // FIXME support other server name variants
        }
    }
}
