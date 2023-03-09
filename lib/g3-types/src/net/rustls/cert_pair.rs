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
use rustls::{Certificate, PrivateKey};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustlsCertificatePair {
    pub certs: Vec<Certificate>,
    pub key: PrivateKey,
}

impl Default for RustlsCertificatePair {
    fn default() -> Self {
        RustlsCertificatePair {
            certs: Vec::with_capacity(1),
            key: PrivateKey(Vec::new()),
        }
    }
}

impl RustlsCertificatePair {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.certs.is_empty() {
            return Err(anyhow!("no certificate set"));
        }
        if self.key.0.is_empty() {
            return Err(anyhow!("no private key set"));
        }
        Ok(())
    }

    pub fn is_set(&self) -> bool {
        !self.certs.is_empty()
    }
}
