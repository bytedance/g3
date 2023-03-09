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

use std::sync::Arc;

use anyhow::anyhow;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::{any_supported_type, CertifiedKey};

use super::RustlsCertificatePair;

#[derive(Default)]
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
        let signing_key =
            any_supported_type(&pair.key).map_err(|e| anyhow!("failed to add cert pair: {e}"))?;
        let ck = CertifiedKey::new(pair.certs.clone(), signing_key);
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
