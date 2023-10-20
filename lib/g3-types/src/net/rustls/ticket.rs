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

use std::sync::{Arc, Mutex};
use std::time;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use rand::Fill;
use ring::aead;
use rustls::server::ProducesTickets;

#[derive(Clone)]
struct AeadKey(aead::LessSafeKey);

impl AeadKey {
    /// Make a ticketer with recommended configuration and a random key.
    fn new() -> Result<Self, rand::Error> {
        let mut key = [0u8; 32];
        key.try_fill(&mut rand::thread_rng())?;

        let alg = &aead::CHACHA20_POLY1305;
        let key = aead::UnboundKey::new(alg, &key).unwrap();

        Ok(Self(aead::LessSafeKey::new(key)))
    }

    /// Encrypt `message` and return the ciphertext.
    fn encrypt(&self, message: &[u8]) -> Option<Vec<u8>> {
        // Random nonce, because a counter is a privacy leak.
        let mut nonce_buf = [0u8; 12];
        nonce_buf.try_fill(&mut rand::thread_rng()).ok()?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_buf);
        let aad = aead::Aad::empty();

        let mut ciphertext =
            Vec::with_capacity(nonce_buf.len() + message.len() + self.0.algorithm().tag_len());
        ciphertext.extend(nonce_buf);
        ciphertext.extend(message);
        self.0
            .seal_in_place_separate_tag(nonce, aad, &mut ciphertext[nonce_buf.len()..])
            .map(|tag| {
                ciphertext.extend(tag.as_ref());
                ciphertext
            })
            .ok()
    }

    /// Decrypt `ciphertext` and recover the original message.
    fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        // Non-panicking `let (nonce, ciphertext) = ciphertext.split_at(...)`.
        let nonce = ciphertext.get(..self.0.algorithm().nonce_len())?;
        let ciphertext = ciphertext.get(nonce.len()..)?;

        // This won't fail since `nonce` has the required length.
        let nonce = aead::Nonce::try_assume_unique_for_key(nonce).ok()?;

        let mut out = Vec::from(ciphertext);

        let plain_len = self
            .0
            .open_in_place(nonce, aead::Aad::empty(), &mut out)
            .ok()?
            .len();
        out.truncate(plain_len);

        Some(out)
    }
}

struct AeadKeys {
    current: AeadKey,
    previous: Option<AeadKey>,
    expire_time: u64,
}

impl AeadKeys {
    fn new(expire_time: u64) -> Result<Self, rand::Error> {
        let current = AeadKey::new()?;
        Ok(AeadKeys {
            current,
            previous: None,
            expire_time,
        })
    }

    fn rotate_new(&self, expire_time: u64) -> Option<Self> {
        let current = AeadKey::new().ok()?;
        let previous = self.current.clone();
        Some(AeadKeys {
            current,
            previous: Some(previous),
            expire_time,
        })
    }
}

pub struct RustlsSessionTicketer {
    lifetime: u32,
    keys: ArcSwap<AeadKeys>,
    lock: Mutex<Arc<AeadKeys>>,
}

impl RustlsSessionTicketer {
    pub fn new() -> Result<Self, anyhow::Error> {
        let time_now = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .map_err(|e| anyhow!("failed to get timestamp now: {e}"))?
            .as_secs();
        let lifetime: u32 = 6 * 60 * 60;
        let keys = AeadKeys::new(time_now.saturating_add(lifetime as u64))
            .map_err(|e| anyhow!("failed to create aead keys: {e}"))?;
        let keys = Arc::new(keys);
        Ok(RustlsSessionTicketer {
            lifetime,
            keys: ArcSwap::new(keys.clone()),
            lock: Mutex::new(keys),
        })
    }
}

impl ProducesTickets for RustlsSessionTicketer {
    fn enabled(&self) -> bool {
        true
    }

    fn lifetime(&self) -> u32 {
        self.lifetime
    }

    fn encrypt(&self, plain: &[u8]) -> Option<Vec<u8>> {
        let keys = self.keys.load_full();

        let time_now = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        if time_now < keys.expire_time {
            return keys.current.encrypt(plain);
        }

        let mut locked_keys = self.lock.lock().unwrap();
        if time_now < locked_keys.expire_time {
            let keys = locked_keys.clone();
            drop(locked_keys);

            // no need to keep a full reference as we have just switched
            keys.current.encrypt(plain)
        } else {
            let new_keys = locked_keys.rotate_new(time_now.saturating_add(self.lifetime as u64))?;
            let new_keys = Arc::new(new_keys);
            self.keys.store(new_keys.clone());
            *locked_keys = new_keys.clone();
            drop(locked_keys);

            new_keys.current.encrypt(plain)
        }
    }

    fn decrypt(&self, cipher: &[u8]) -> Option<Vec<u8>> {
        let keys = self.keys.load_full();
        // Decrypt with the current key; if that fails, try with the previous.
        keys.current
            .decrypt(cipher)
            .or_else(|| keys.previous.as_ref().and_then(|p| p.decrypt(cipher)))
    }
}
