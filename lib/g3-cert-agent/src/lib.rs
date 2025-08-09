/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::anyhow;
use openssl::pkey::{PKey, Private};
use openssl::ssl::SslRef;
use openssl::x509::X509;

use g3_types::net::{TlsCertUsage, TlsServiceType};

mod protocol;
pub use protocol::*;

mod response;
use response::Response;

mod request;
pub use request::Request;

mod query;
use query::QueryRuntime;

mod config;
pub use config::CertAgentConfig;

mod handle;
pub use handle::CertAgentHandle;

mod runtime;
pub use runtime::*;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct CacheIndexKey {
    service: TlsServiceType,
    usage: TlsCertUsage,
    host: Arc<str>,
}

#[derive(Clone, Debug)]
struct CacheQueryKey {
    index: CacheIndexKey,
    mimic_cert: Option<X509>,
}

impl CacheQueryKey {
    fn new(service: TlsServiceType, usage: TlsCertUsage, host: Arc<str>) -> Self {
        CacheQueryKey {
            index: CacheIndexKey {
                service,
                usage,
                host,
            },
            mimic_cert: None,
        }
    }

    fn host(&self) -> &str {
        self.index.host.as_ref()
    }

    fn set_mimic_cert(&mut self, cert: X509) {
        self.mimic_cert = Some(cert);
    }

    fn encode(&self) -> Result<Vec<u8>, rmpv::encode::Error> {
        use rmpv::ValueRef;

        let mut map = Vec::with_capacity(4);
        map.push((
            ValueRef::Integer(request_key_id::HOST.into()),
            ValueRef::String(self.host().into()),
        ));
        map.push((
            ValueRef::Integer(request_key_id::SERVICE.into()),
            ValueRef::Integer((self.index.service as u8).into()),
        ));
        map.push((
            ValueRef::Integer(request_key_id::USAGE.into()),
            ValueRef::Integer((self.index.usage as u8).into()),
        ));
        if let Some(cert) = &self.mimic_cert
            && let Ok(der) = cert.to_der()
        {
            map.push((
                ValueRef::Integer(request_key_id::CERT.into()),
                ValueRef::Binary(&der),
            ));
            let mut buf = Vec::with_capacity(320 + der.len());
            rmpv::encode::write_value_ref(&mut buf, &ValueRef::Map(map))?;
            return Ok(buf);
        };
        let mut buf = Vec::with_capacity(320);
        rmpv::encode::write_value_ref(&mut buf, &ValueRef::Map(map))?;
        Ok(buf)
    }
}

impl Hash for CacheQueryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl PartialEq for CacheQueryKey {
    fn eq(&self, other: &Self) -> bool {
        self.index.eq(&other.index)
    }
}

impl Eq for CacheQueryKey {}

#[derive(Clone)]
pub struct FakeCertPair {
    certs: Vec<X509>,
    key: PKey<Private>,
}

impl FakeCertPair {
    pub fn add_to_ssl(self, ssl: &mut SslRef) -> anyhow::Result<()> {
        let FakeCertPair { certs, key } = self;
        let mut certs_iter = certs.into_iter();
        let Some(leaf_cert) = certs_iter.next() else {
            return Err(anyhow!("no certificate found"));
        };
        ssl.set_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set certificate: {e}"))?;
        for cert in certs_iter {
            ssl.add_chain_cert(cert)
                .map_err(|e| anyhow!("failed to add chain cert: {e}"))?;
        }
        ssl.set_private_key(&key)
            .map_err(|e| anyhow!("failed to set private key: {e}"))?;
        Ok(())
    }

    #[cfg(feature = "tongsuo")]
    pub fn add_enc_to_tlcp(self, ssl: &mut SslRef) -> anyhow::Result<()> {
        let FakeCertPair { certs, key } = self;
        let mut certs_iter = certs.into_iter();
        let Some(leaf_cert) = certs_iter.next() else {
            return Err(anyhow!("no certificate found"));
        };
        ssl.set_enc_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set enc certificate: {e}"))?;
        for cert in certs_iter {
            ssl.add_chain_cert(cert)
                .map_err(|e| anyhow!("failed to add chain cert: {e}"))?;
        }
        ssl.set_enc_private_key(&key)
            .map_err(|e| anyhow!("failed to set enc private key: {e}"))?;
        Ok(())
    }

    #[cfg(feature = "tongsuo")]
    pub fn add_sign_to_tlcp(self, ssl: &mut SslRef) -> anyhow::Result<()> {
        let FakeCertPair { certs, key } = self;
        let mut certs_iter = certs.into_iter();
        let Some(leaf_cert) = certs_iter.next() else {
            return Err(anyhow!("no certificate found"));
        };
        ssl.set_sign_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set sign certificate: {e}"))?;
        for cert in certs_iter {
            ssl.add_chain_cert(cert)
                .map_err(|e| anyhow!("failed to add chain cert: {e}"))?;
        }
        ssl.set_sign_private_key(&key)
            .map_err(|e| anyhow!("failed to set sign private key: {e}"))?;
        Ok(())
    }
}
