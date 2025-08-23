/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rmpv::ValueRef;

use g3_types::net::{TlsCertUsage, TlsServiceType};

use super::{CacheQueryKey, FakeCertPair, response_key, response_key_id};

pub(super) struct Response {
    host: String,
    service: TlsServiceType,
    usage: TlsCertUsage,
    certs: Vec<X509>,
    key: Option<PKey<Private>>,
    ttl: u32,
}

impl Response {
    fn new(protective_ttl: u32) -> Self {
        Response {
            host: String::default(),
            service: TlsServiceType::Http,
            usage: TlsCertUsage::TlsServer,
            certs: Vec::new(),
            key: None,
            ttl: protective_ttl,
        }
    }

    fn set(&mut self, k: ValueRef, v: ValueRef) -> anyhow::Result<()> {
        match k {
            ValueRef::String(s) => {
                let key = s
                    .as_str()
                    .ok_or_else(|| anyhow!("invalid string key {k}"))?;
                match g3_msgpack::key::normalize(key).as_str() {
                    response_key::HOST => {
                        self.host = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key {key}"))?;
                    }
                    response_key::SERVICE => {
                        self.service = g3_msgpack::value::as_tls_service_type(&v)
                            .context(format!("invalid tls service type value for key {key}"))?;
                    }
                    response_key::USAGE => {
                        self.usage = g3_msgpack::value::as_tls_cert_usage(&v)
                            .context(format!("invalid tls cert usage value for key {key}"))?;
                    }
                    response_key::CERT_CHAIN => {
                        self.certs = g3_msgpack::value::as_openssl_certificates(&v)
                            .context(format!("invalid tls certificate value for key {key}"))?;
                    }
                    response_key::PRIVATE_KEY => {
                        let key = g3_msgpack::value::as_openssl_private_key(&v)
                            .context(format!("invalid tls private key value for key {key}"))?;
                        self.key = Some(key);
                    }
                    response_key::TTL => {
                        self.ttl = g3_msgpack::value::as_u32(&v)
                            .context(format!("invalid u32 value for key {key}"))?;
                    }
                    _ => {} // ignore unknown keys
                }
            }
            ValueRef::Integer(i) => {
                let key_id = i.as_u64().ok_or_else(|| anyhow!("invalid u64 key {k}"))?;
                match key_id {
                    response_key_id::HOST => {
                        self.host = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key id {key_id}"))?;
                    }
                    response_key_id::SERVICE => {
                        self.service = g3_msgpack::value::as_tls_service_type(&v).context(
                            format!("invalid tls service type value for key id {key_id}"),
                        )?;
                    }
                    response_key_id::USAGE => {
                        self.usage = g3_msgpack::value::as_tls_cert_usage(&v)
                            .context(format!("invalid tls cert usage value for key id {key_id}"))?;
                    }
                    response_key_id::CERT_CHAIN => {
                        self.certs = g3_msgpack::value::as_openssl_certificates(&v).context(
                            format!("invalid tls certificate value for key id {key_id}"),
                        )?;
                    }
                    response_key_id::PRIVATE_KEY => {
                        let key = g3_msgpack::value::as_openssl_private_key(&v).context(
                            format!("invalid tls private key value for key id {key_id}"),
                        )?;
                        self.key = Some(key);
                    }
                    response_key_id::TTL => {
                        self.ttl = g3_msgpack::value::as_u32(&v)
                            .context(format!("invalid u32 value for key id {key_id}"))?;
                    }
                    _ => {} // ignore unknown keys
                }
            }
            _ => return Err(anyhow!("unsupported key type: {k}")),
        }
        Ok(())
    }

    pub(super) fn parse(v: ValueRef, protective_ttl: u32) -> anyhow::Result<Self> {
        if let ValueRef::Map(map) = v {
            let mut response = Response::new(protective_ttl);
            for (k, v) in map {
                response.set(k, v)?;
            }
            Ok(response)
        } else {
            Err(anyhow!("the response data type should be 'map'"))
        }
    }

    pub(super) fn into_parts(self) -> anyhow::Result<(CacheQueryKey, FakeCertPair, u32)> {
        if self.certs.is_empty() {
            return Err(anyhow!("no cert chain set"));
        }
        let key = self.key.ok_or_else(|| anyhow!("no private key set"))?;
        Ok((
            CacheQueryKey::new(self.service, self.usage, Arc::from(self.host)),
            FakeCertPair {
                certs: self.certs,
                key,
            },
            self.ttl,
        ))
    }
}
