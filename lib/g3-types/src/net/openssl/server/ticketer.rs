/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

#[cfg(feature = "rustls")]
use std::fmt;

#[cfg(feature = "rustls")]
use log::warn;
use openssl::cipher_ctx::CipherCtxRef;
use openssl::error::ErrorStack;
use openssl::hmac::HMacCtxRef;
use openssl::ssl::TicketKeyStatus;
#[cfg(feature = "rustls")]
use rustls::server::ProducesTickets;

use super::OpensslTicketKey;
#[cfg(feature = "rustls")]
use crate::net::RollingTicketKey;
use crate::net::RollingTicketer;

impl RollingTicketer<OpensslTicketKey> {
    pub fn encrypt_init(
        &self,
        key_name: &mut [u8],
        iv: &mut [u8],
        cipher_ctx: &mut CipherCtxRef,
        hmac_ctx: &mut HMacCtxRef,
    ) -> Result<TicketKeyStatus, ErrorStack> {
        self.enc_key
            .load()
            .encrypt_init(key_name, iv, cipher_ctx, hmac_ctx)
    }

    pub fn decrypt_init(
        &self,
        key_name: &[u8],
        iv: &[u8],
        cipher_ctx: &mut CipherCtxRef,
        hmac_ctx: &mut HMacCtxRef,
    ) -> Result<TicketKeyStatus, ErrorStack> {
        let Some(key) = self.get_decrypt_key(key_name) else {
            return Ok(TicketKeyStatus::FAILED);
        };

        key.decrypt_init(iv, cipher_ctx, hmac_ctx)?;

        Ok(TicketKeyStatus::SUCCESS_AND_RENEW)
    }
}

#[cfg(feature = "rustls")]
impl ProducesTickets for RollingTicketer<OpensslTicketKey> {
    fn enabled(&self) -> bool {
        true
    }

    fn lifetime(&self) -> u32 {
        self.enc_key.load().lifetime()
    }

    fn encrypt(&self, plain: &[u8]) -> Option<Vec<u8>> {
        match self.enc_key.load().encrypt(plain) {
            Ok(d) => Some(d),
            Err(e) => {
                warn!("ticket encrypt failed: {e}");
                None
            }
        }
    }

    fn decrypt(&self, cipher: &[u8]) -> Option<Vec<u8>> {
        self.get_decrypt_key(cipher).and_then(|key| {
            key.decrypt(cipher).unwrap_or_else(|e| {
                warn!("ticket decrypt failed: {e}");
                None
            })
        })
    }
}

#[cfg(feature = "rustls")]
impl fmt::Debug for RollingTicketer<OpensslTicketKey> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RollingTicketer based on OpenSSL").finish()
    }
}
