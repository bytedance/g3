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

use openssl::error::ErrorStack;
use openssl::hash::{Hasher, MessageDigest};
use openssl::ssl::SslContextBuilder;
use openssl::x509::{X509NameRef, X509Ref};

pub struct OpensslSessionIdContext {
    hasher: Hasher,
}

impl OpensslSessionIdContext {
    pub fn new() -> Result<Self, ErrorStack> {
        let hasher = Hasher::new(MessageDigest::sha1())?;
        Ok(OpensslSessionIdContext { hasher })
    }

    pub fn add_text(&mut self, s: &str) -> Result<(), ErrorStack> {
        self.hasher.update(s.as_bytes())
    }

    pub fn add_cert(&mut self, cert: &X509Ref) -> Result<(), ErrorStack> {
        let cert_digest = cert.digest(MessageDigest::sha1())?;
        self.hasher.update(cert_digest.as_ref())
    }

    pub fn add_ca_subject(&mut self, name: &X509NameRef) -> Result<(), ErrorStack> {
        let name_digest = name.digest(MessageDigest::sha1())?;
        self.hasher.update(name_digest.as_ref())
    }

    pub fn build_set(mut self, ssl_builder: &mut SslContextBuilder) -> Result<(), ErrorStack> {
        let digest = self.hasher.finish()?;
        ssl_builder.set_session_id_context(digest.as_ref())
    }
}
