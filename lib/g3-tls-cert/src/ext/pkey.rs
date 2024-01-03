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
use openssl::hash::{DigestBytes, MessageDigest};
use openssl::pkey::{HasPublic, PKey};
use openssl::x509::X509Pubkey;

pub trait PublicKeyExt {
    fn ski(&self) -> Result<DigestBytes, ErrorStack>;
}

impl<T: HasPublic> PublicKeyExt for PKey<T> {
    fn ski(&self) -> Result<DigestBytes, ErrorStack> {
        let x = X509Pubkey::from_pubkey(self)?;
        let encoded = x.encoded_bytes()?;
        openssl::hash::hash(MessageDigest::sha1(), encoded)
    }
}
