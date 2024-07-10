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
use openssl::cipher::Cipher;
use openssl::cipher_ctx::CipherCtxRef;
use openssl::error::ErrorStack;
use openssl::hmac::HMacCtxRef;
use openssl::md::Md;
use openssl::rand;
use openssl::ssl::TicketKeyStatus;

use crate::net::{RollingTicketKey, TicketKeyName, TICKET_KEY_LENGTH, TICKET_KEY_NAME_LENGTH};

pub struct OpensslTicketKey {
    name: TicketKeyName,
    lifetime: u32,
    key: [u8; TICKET_KEY_LENGTH],
}

impl OpensslTicketKey {
    pub(super) fn encrypt_init(
        &self,
        key_name: &mut [u8],
        iv: &[u8],
        cipher_ctx: &mut CipherCtxRef,
        hmac_ctx: &mut HMacCtxRef,
    ) -> Result<TicketKeyStatus, ErrorStack> {
        if key_name.len() != TICKET_KEY_NAME_LENGTH {
            return Ok(TicketKeyStatus::FAILED);
        }
        key_name.copy_from_slice(self.name.as_ref());

        cipher_ctx.encrypt_init(Some(Cipher::aes_256_cbc()), Some(&self.key), Some(iv))?;
        hmac_ctx.init_ex(Some(&self.key), Md::sha256())?;

        Ok(TicketKeyStatus::SUCCESS)
    }

    pub(super) fn decrypt_init(
        &self,
        iv: &[u8],
        cipher_ctx: &mut CipherCtxRef,
        hmac_ctx: &mut HMacCtxRef,
    ) -> Result<(), ErrorStack> {
        hmac_ctx.init_ex(Some(&self.key), Md::sha256())?;
        cipher_ctx.decrypt_init(Some(Cipher::aes_256_cbc()), Some(&self.key), Some(iv))?;
        Ok(())
    }
}

impl RollingTicketKey for OpensslTicketKey {
    fn new(lifetime: u32) -> anyhow::Result<Self> {
        let mut key = [0u8; TICKET_KEY_LENGTH];
        rand::rand_bytes(&mut key).map_err(|e| anyhow!("failed to generate random key: {e}"))?;

        let mut key_name = [0u8; TICKET_KEY_NAME_LENGTH];
        rand::rand_bytes(&mut key_name)
            .map_err(|e| anyhow!("failed to generate random key name: {e}"))?;

        Ok(OpensslTicketKey {
            name: key_name.into(),
            lifetime,
            key,
        })
    }

    #[inline]
    fn name(&self) -> &TicketKeyName {
        &self.name
    }

    #[inline]
    fn lifetime(&self) -> u32 {
        self.lifetime
    }
}
