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

use anyhow::anyhow;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct S2nTlsCertPair {
    cert_chain: String,
    private_key: String,
}

impl S2nTlsCertPair {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_chain.is_empty() {
            return Err(anyhow!("no certificate set"));
        }
        if self.private_key.is_empty() {
            return Err(anyhow!("no private key set"));
        }
        Ok(())
    }

    #[inline]
    pub fn cert_chain(&self) -> &[u8] {
        self.cert_chain.as_bytes()
    }

    pub fn set_cert_chain(&mut self, chain: String) {
        self.cert_chain = chain;
    }

    #[inline]
    pub fn private_key(&self) -> &[u8] {
        self.private_key.as_bytes()
    }

    pub fn set_private_key(&mut self, key: String) {
        self.private_key = key;
    }
}
