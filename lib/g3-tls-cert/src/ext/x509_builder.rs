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
use openssl::hash::MessageDigest;
use openssl::pkey::{HasPrivate, Id, PKeyRef};
use openssl::x509::X509Builder;

pub trait X509BuilderExt {
    fn sign_with_optional_digest<T: HasPrivate>(
        &mut self,
        key: &PKeyRef<T>,
        digest: Option<MessageDigest>,
    ) -> Result<(), ErrorStack>;
}

impl X509BuilderExt for X509Builder {
    fn sign_with_optional_digest<T: HasPrivate>(
        &mut self,
        key: &PKeyRef<T>,
        digest: Option<MessageDigest>,
    ) -> Result<(), ErrorStack> {
        let digest = digest.unwrap_or_else(|| match key.id() {
            // TODO use Id::SM2 after supported
            Id::ED25519 | Id::ED448 => MessageDigest::null(),
            id => {
                if id.as_raw() == 1172 {
                    MessageDigest::sm3()
                } else {
                    MessageDigest::sha256()
                }
            }
        });
        self.sign(key, digest)
    }
}
