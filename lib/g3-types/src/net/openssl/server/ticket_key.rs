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

use crate::net::{
    RollingTicketKey, TicketKeyName, TICKET_AES_KEY_LENGTH, TICKET_HMAC_KEY_LENGTH,
    TICKET_KEY_NAME_LENGTH,
};

pub struct OpensslTicketKey {
    name: TicketKeyName,
    lifetime: u32,
    aes_key: [u8; TICKET_AES_KEY_LENGTH],
    hmac_key: [u8; TICKET_HMAC_KEY_LENGTH],
}

impl OpensslTicketKey {
    pub fn new(
        name: &[u8],
        aes_key: &[u8],
        hmac_key: &[u8],
        lifetime: u32,
    ) -> anyhow::Result<Self> {
        if name.len() < TICKET_KEY_NAME_LENGTH {
            return Err(anyhow!("too short ticket key name"));
        }
        if aes_key.len() < TICKET_AES_KEY_LENGTH {
            return Err(anyhow!("too short ticket AES key"));
        }
        if hmac_key.len() < TICKET_HMAC_KEY_LENGTH {
            return Err(anyhow!("too short ticket HMAC key"));
        }

        let mut key_name = [0u8; TICKET_KEY_NAME_LENGTH];
        key_name.copy_from_slice(&name[..TICKET_KEY_NAME_LENGTH]);
        let mut aes = [0u8; TICKET_AES_KEY_LENGTH];
        aes.copy_from_slice(&aes_key[..TICKET_AES_KEY_LENGTH]);
        let mut hmac = [0u8; TICKET_HMAC_KEY_LENGTH];
        hmac.copy_from_slice(&hmac_key[..TICKET_HMAC_KEY_LENGTH]);

        Ok(OpensslTicketKey {
            name: key_name.into(),
            aes_key: aes,
            hmac_key: hmac,
            lifetime,
        })
    }

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

        cipher_ctx.encrypt_init(Some(Cipher::aes_256_cbc()), Some(&self.aes_key), Some(iv))?;
        hmac_ctx.init_ex(Some(&self.hmac_key), Md::sha256())?;

        Ok(TicketKeyStatus::SUCCESS)
    }

    pub(super) fn decrypt_init(
        &self,
        iv: &[u8],
        cipher_ctx: &mut CipherCtxRef,
        hmac_ctx: &mut HMacCtxRef,
    ) -> Result<(), ErrorStack> {
        hmac_ctx.init_ex(Some(&self.hmac_key), Md::sha256())?;
        cipher_ctx.decrypt_init(Some(Cipher::aes_256_cbc()), Some(&self.aes_key), Some(iv))?;
        Ok(())
    }
}

impl RollingTicketKey for OpensslTicketKey {
    fn new_random(lifetime: u32) -> anyhow::Result<Self> {
        let mut aes_key = [0u8; TICKET_AES_KEY_LENGTH];
        rand::rand_bytes(&mut aes_key)
            .map_err(|e| anyhow!("failed to generate random AES key: {e}"))?;

        let mut hmac_key = [0u8; TICKET_HMAC_KEY_LENGTH];
        rand::rand_bytes(&mut hmac_key)
            .map_err(|e| anyhow!("failed to generate random HMAC key: {e}"))?;

        let mut key_name = [0u8; TICKET_KEY_NAME_LENGTH];
        rand::rand_bytes(&mut key_name)
            .map_err(|e| anyhow!("failed to generate random key name: {e}"))?;

        Ok(OpensslTicketKey {
            name: key_name.into(),
            lifetime,
            aes_key,
            hmac_key,
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
