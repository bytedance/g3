/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Arc;

use anyhow::{Context, anyhow};
use openssl::x509::X509;
use rmpv::ValueRef;

use g3_types::net::{TlsCertUsage, TlsServiceType};

use super::{request_key, request_key_id, response_key_id};

pub struct Request {
    pub(crate) host: Arc<str>,
    service: TlsServiceType,
    usage: TlsCertUsage,
    pub(crate) cert: Option<X509>,
}

impl Default for Request {
    fn default() -> Self {
        Request {
            host: Arc::from(""),
            service: TlsServiceType::Http,
            usage: TlsCertUsage::TlsServer,
            cert: None,
        }
    }
}

impl Request {
    #[inline]
    pub fn host(&self) -> Arc<str> {
        self.host.clone()
    }

    #[inline]
    pub fn host_str(&self) -> &str {
        self.host.as_ref()
    }

    #[inline]
    pub fn cert(&self) -> Option<&X509> {
        self.cert.as_ref()
    }

    #[inline]
    pub fn cert_usage(&self) -> TlsCertUsage {
        self.usage
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.host.is_empty() {
            return Err(anyhow!("no host value set"));
        }
        Ok(())
    }

    fn set(&mut self, k: ValueRef, v: ValueRef) -> anyhow::Result<()> {
        match k {
            ValueRef::String(s) => {
                let key = s
                    .as_str()
                    .ok_or_else(|| anyhow!("invalid string key {k}"))?;
                match g3_msgpack::key::normalize(key).as_str() {
                    request_key::HOST => self
                        .set_host_value(v)
                        .context(format!("invalid string value for key {key}")),
                    request_key::SERVICE => {
                        self.service = g3_msgpack::value::as_tls_service_type(&v)
                            .context(format!("invalid tls service type value for key {key}"))?;
                        Ok(())
                    }
                    request_key::USAGE => {
                        self.usage = g3_msgpack::value::as_tls_cert_usage(&v)
                            .context(format!("invalid tls cert usage value for key {key}"))?;
                        Ok(())
                    }
                    request_key::CERT => {
                        let cert = g3_msgpack::value::as_openssl_certificate(&v)
                            .context(format!("invalid mimic cert value for key {key}"))?;
                        self.cert = Some(cert);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {key}")),
                }
            }
            ValueRef::Integer(i) => {
                let key_id = i.as_u64().ok_or_else(|| anyhow!("invalid u64 key {k}"))?;
                match key_id {
                    request_key_id::HOST => self
                        .set_host_value(v)
                        .context(format!("invalid host string value for key id {key_id}")),
                    request_key_id::SERVICE => {
                        self.service = g3_msgpack::value::as_tls_service_type(&v).context(
                            format!("invalid tls service type value for key id {key_id}"),
                        )?;
                        Ok(())
                    }
                    request_key_id::USAGE => {
                        self.usage = g3_msgpack::value::as_tls_cert_usage(&v)
                            .context(format!("invalid tls cert usage value for key id {key_id}"))?;
                        Ok(())
                    }
                    request_key_id::CERT => {
                        let cert = g3_msgpack::value::as_openssl_certificate(&v)
                            .context(format!("invalid mimic cert value for key id {key_id}"))?;
                        self.cert = Some(cert);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key id {key_id}")),
                }
            }
            _ => Err(anyhow!("unsupported key type: {k}")),
        }
    }

    fn set_host_value(&mut self, v: ValueRef) -> anyhow::Result<()> {
        let host = g3_msgpack::value::as_string(&v)?;
        self.host = Arc::from(host);
        Ok(())
    }

    pub fn parse_req(mut data: &[u8]) -> anyhow::Result<Self> {
        let v = rmpv::decode::read_value_ref(&mut data)
            .map_err(|e| anyhow!("invalid req data: {e}"))?;

        let mut request = Request::default();
        if let ValueRef::Map(map) = v {
            for (k, v) in map {
                request.set(k, v)?;
            }
        } else {
            request
                .set_host_value(v)
                .context("invalid single host string value")?;
        }

        request.check()?;
        Ok(request)
    }

    pub fn encode_rsp(&self, pem_cert: &str, der_key: &[u8], ttl: u32) -> anyhow::Result<Vec<u8>> {
        let map = vec![
            (
                ValueRef::Integer(response_key_id::HOST.into()),
                ValueRef::String(self.host.as_ref().into()),
            ),
            (
                ValueRef::Integer(response_key_id::SERVICE.into()),
                ValueRef::Integer((self.service as u8).into()),
            ),
            (
                ValueRef::Integer(response_key_id::USAGE.into()),
                ValueRef::Integer((self.usage as u8).into()),
            ),
            (
                ValueRef::Integer(response_key_id::CERT_CHAIN.into()),
                ValueRef::String(pem_cert.into()),
            ),
            (
                ValueRef::Integer(response_key_id::PRIVATE_KEY.into()),
                ValueRef::Binary(der_key),
            ),
            (
                ValueRef::Integer(response_key_id::TTL.into()),
                ValueRef::Integer(ttl.into()),
            ),
        ];
        let mut buf = Vec::with_capacity(4096);
        let v = ValueRef::Map(map);
        rmpv::encode::write_value_ref(&mut buf, &v)
            .map_err(|e| anyhow!("msgpack encode failed: {e}"))?;
        Ok(buf)
    }
}
