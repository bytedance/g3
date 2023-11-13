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
