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

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::anyhow;
use ring::aead;
use ring::rand::{SecureRandom, SystemRandom};

use crate::net::{RollingTicketKey, TicketKeyName, TICKET_AES_KEY_LENGTH, TICKET_KEY_NAME_LENGTH};

pub struct RustlsTicketKey {
    name: TicketKeyName,
    lifetime: u32,
    aead_key: aead::LessSafeKey,

    /// Tracks the largest ciphertext produced by `encrypt`, and
    /// uses it to early-reject `decrypt` queries that are too long.
    ///
    /// Accepting excessively long ciphertexts means a "Partitioning
    /// Oracle Attack" (see <https://eprint.iacr.org/2020/1491.pdf>)
    /// can be more efficient, though also note that these are thought
    /// to be cryptographically hard if the key is full-entropy (as it
    /// is here).
    maximum_ciphertext_len: AtomicUsize,
}

impl RustlsTicketKey {
    pub fn new(name: &[u8], aes_key: &[u8], lifetime: u32) -> anyhow::Result<Self> {
        if name.len() < TICKET_KEY_NAME_LENGTH {
            return Err(anyhow!("too short ticket key name"));
        }
        if aes_key.len() < TICKET_AES_KEY_LENGTH {
            return Err(anyhow!("too short ticket AES key"));
        }

        let mut key_name = [0u8; TICKET_KEY_NAME_LENGTH];
        key_name.copy_from_slice(&name[..TICKET_KEY_NAME_LENGTH]);

        let mut aes = [0u8; TICKET_AES_KEY_LENGTH];
        aes.copy_from_slice(&aes_key[..TICKET_AES_KEY_LENGTH]);

        let key = aead::UnboundKey::new(&aead::AES_256_GCM, &aes).unwrap();

        Ok(RustlsTicketKey {
            name: key_name.into(),
            lifetime,
            aead_key: aead::LessSafeKey::new(key),
            maximum_ciphertext_len: AtomicUsize::new(0),
        })
    }

    /// Encrypt `message` and return the ciphertext.
    pub(super) fn encrypt(&self, message: &[u8]) -> Option<Vec<u8>> {
        // Random nonce, because a counter is a privacy leak.
        let mut nonce_buf = [0u8; aead::NONCE_LEN];
        SystemRandom::new().fill(&mut nonce_buf).ok()?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_buf);
        let aad = aead::Aad::from(&self.name);

        // ciphertext structure is:
        // key_name: [u8; 16]
        // nonce: [u8; 12]
        // message: [u8, _]
        // tag: [u8; 16]

        let mut ciphertext = Vec::with_capacity(
            TICKET_KEY_NAME_LENGTH
                + nonce_buf.len()
                + message.len()
                + self.aead_key.algorithm().tag_len(),
        );
        ciphertext.extend(self.name.as_ref());
        ciphertext.extend(nonce_buf);
        ciphertext.extend(message);
        let ciphertext = self
            .aead_key
            .seal_in_place_separate_tag(
                nonce,
                aad,
                &mut ciphertext[TICKET_KEY_NAME_LENGTH + nonce_buf.len()..],
            )
            .map(|tag| {
                ciphertext.extend(tag.as_ref());
                ciphertext
            })
            .ok()?;

        self.maximum_ciphertext_len
            .fetch_max(ciphertext.len(), Ordering::SeqCst);
        Some(ciphertext)
    }

    /// Decrypt `ciphertext` and recover the original message.
    pub(super) fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        if ciphertext.len() > self.maximum_ciphertext_len.load(Ordering::SeqCst) {
            return None;
        }

        let (alleged_key_name, ciphertext) = try_split_at(ciphertext, TICKET_KEY_NAME_LENGTH)?;
        let (nonce, ciphertext) = try_split_at(ciphertext, aead::NONCE_LEN)?;

        // This won't fail since `nonce` has the required length.
        let nonce = aead::Nonce::try_assume_unique_for_key(nonce).ok()?;

        let mut out = Vec::from(ciphertext);

        let plain_len = self
            .aead_key
            .open_in_place(nonce, aead::Aad::from(alleged_key_name), &mut out)
            .ok()?
            .len();
        out.truncate(plain_len);

        Some(out)
    }
}

/// Non-panicking `let (nonce, ciphertext) = ciphertext.split_at(...)`.
fn try_split_at(slice: &[u8], mid: usize) -> Option<(&[u8], &[u8])> {
    match mid > slice.len() {
        true => None,
        false => Some(slice.split_at(mid)),
    }
}

impl RollingTicketKey for RustlsTicketKey {
    fn new_random(lifetime: u32) -> anyhow::Result<Self> {
        let mut key = [0u8; TICKET_AES_KEY_LENGTH];
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| anyhow!("failed to generate random key"))?;

        let aes_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key).unwrap();

        let mut key_name = [0u8; TICKET_KEY_NAME_LENGTH];
        SystemRandom::new()
            .fill(&mut key_name)
            .map_err(|_| anyhow!("failed to generate random key name"))?;

        Ok(RustlsTicketKey {
            name: key_name.into(),
            lifetime,
            aead_key: aead::LessSafeKey::new(aes_key),
            maximum_ciphertext_len: AtomicUsize::new(0),
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
