/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
