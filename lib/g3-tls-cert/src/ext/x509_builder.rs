/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::pkey::{HasPrivate, PKeyRef};
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
        use openssl::pkey::Id;

        let digest = digest.unwrap_or_else(|| match key.id() {
            // see https://www.openssl.org/docs/manmaster/man3/EVP_DigestSign.html
            #[cfg(not(osslconf = "OPENSSL_NO_SM2"))]
            Id::SM2 => MessageDigest::sm3(),
            Id::ED25519 => null_message_digest(),
            #[cfg(not(any(libressl, boringssl, awslc)))]
            Id::ED448 => MessageDigest::null(),
            id => {
                if id.as_raw() == -1 {
                    null_message_digest()
                } else {
                    MessageDigest::sha256()
                }
            }
        });
        self.sign(key, digest)
    }
}

#[cfg(not(boringssl))]
fn null_message_digest() -> MessageDigest {
    MessageDigest::null()
}

#[cfg(boringssl)]
fn null_message_digest() -> MessageDigest {
    unsafe { MessageDigest::from_ptr(std::ptr::null()) }
}
