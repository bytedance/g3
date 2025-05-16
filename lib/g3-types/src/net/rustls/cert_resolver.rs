/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use rustls::crypto::CryptoProvider;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use super::RustlsCertificatePair;

#[derive(Debug, Default)]
pub struct MultipleCertResolver {
    keys: Vec<Arc<CertifiedKey>>,
}

impl MultipleCertResolver {
    pub fn with_capacity(cap: usize) -> Self {
        MultipleCertResolver {
            keys: Vec::with_capacity(cap),
        }
    }

    pub fn push_cert_pair(&mut self, pair: &RustlsCertificatePair) -> anyhow::Result<()> {
        let Some(provider) = CryptoProvider::get_default() else {
            return Err(anyhow!("no rustls provider registered"));
        };
        let ck = CertifiedKey::from_der(pair.certs_owned(), pair.key_owned(), provider)
            .map_err(|e| anyhow!("failed to load cert pair: {e}"))?;
        self.keys.push(Arc::new(ck));
        Ok(())
    }
}

impl ResolvesServerCert for MultipleCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        let schemes = client_hello.signature_schemes();
        for ck in &self.keys {
            if ck.key.choose_scheme(schemes).is_some() {
                return Some(ck.clone());
            }
        }
        None
    }
}
