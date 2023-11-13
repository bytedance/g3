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
use rustls_pki_types::{
    CertificateDer, PrivateKeyDer, PrivatePkcs1KeyDer, PrivatePkcs8KeyDer, PrivateSec1KeyDer,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivateKey {
    Pkcs1(Vec<u8>),
    Sec1(Vec<u8>),
    Pkcs8(Vec<u8>),
}

impl PrivateKey {
    fn borrowed(&self) -> PrivateKeyDer<'_> {
        match self {
            PrivateKey::Pkcs1(v) => PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(v.as_ref())),
            PrivateKey::Sec1(v) => PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(v.as_ref())),
            PrivateKey::Pkcs8(v) => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(v.as_ref())),
        }
    }
}

impl TryFrom<PrivateKeyDer<'_>> for PrivateKey {
    type Error = anyhow::Error;

    fn try_from(value: PrivateKeyDer<'_>) -> anyhow::Result<Self> {
        match value {
            PrivateKeyDer::Pkcs1(d) => Ok(PrivateKey::Pkcs1(d.secret_pkcs1_der().to_vec())),
            PrivateKeyDer::Sec1(d) => Ok(PrivateKey::Sec1(d.secret_sec1_der().to_vec())),
            PrivateKeyDer::Pkcs8(d) => Ok(PrivateKey::Pkcs8(d.secret_pkcs8_der().to_vec())),
            _ => Err(anyhow!(
                "unsupported private key type, this code should be updated"
            )),
        }
    }
}

impl From<&PrivateKey> for PrivateKeyDer<'static> {
    fn from(value: &PrivateKey) -> Self {
        match value {
            PrivateKey::Pkcs1(v) => PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(v.clone())),
            PrivateKey::Sec1(v) => PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(v.clone())),
            PrivateKey::Pkcs8(v) => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(v.clone())),
        }
    }
}

impl From<PrivateKey> for PrivateKeyDer<'static> {
    fn from(value: PrivateKey) -> Self {
        match value {
            PrivateKey::Pkcs1(v) => PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(v)),
            PrivateKey::Sec1(v) => PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(v)),
            PrivateKey::Pkcs8(v) => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(v)),
        }
    }
}

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
        let key = PrivateKey::try_from(key)?;
        Ok(RustlsCertificatePair {
            certs: self.certs,
            key,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustlsCertificatePair {
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKey,
}

impl RustlsCertificatePair {
    pub fn certs_owned(&self) -> Vec<CertificateDer<'static>> {
        self.certs.clone()
    }

    pub fn key_owned(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::from(&self.key)
    }

    pub fn key_borrowed(&self) -> PrivateKeyDer<'_> {
        self.key.borrowed()
    }

    pub fn into_inner(self) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
        (self.certs, self.key.into())
    }
}
