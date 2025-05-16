/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

#[derive(Default)]
pub struct RustlsCertificatePairBuilder {
    certs: Vec<CertificateDer<'static>>,
    key: Option<PrivateKeyDer<'static>>,
}

impl RustlsCertificatePairBuilder {
    pub fn set_certs(&mut self, certs: Vec<CertificateDer<'static>>) {
        self.certs = certs;
    }

    pub fn set_key(&mut self, key: PrivateKeyDer<'static>) {
        self.key = Some(key);
    }

    pub fn build(self) -> anyhow::Result<RustlsCertificatePair> {
        if self.certs.is_empty() {
            return Err(anyhow!("no certificate set"));
        }
        let Some(key) = self.key else {
            return Err(anyhow!("no private key set"));
        };
        Ok(RustlsCertificatePair {
            certs: self.certs,
            key,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RustlsCertificatePair {
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl Clone for RustlsCertificatePair {
    fn clone(&self) -> Self {
        RustlsCertificatePair {
            certs: self.certs.clone(),
            key: self.key.clone_key(),
        }
    }
}

impl RustlsCertificatePair {
    pub fn certs_owned(&self) -> Vec<CertificateDer<'static>> {
        self.certs.clone()
    }

    pub fn key_owned(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }

    pub fn key_ref(&self) -> &PrivateKeyDer<'_> {
        &self.key
    }

    pub fn into_inner(self) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
        (self.certs, self.key)
    }
}
