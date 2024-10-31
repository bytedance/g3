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
