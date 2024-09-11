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

use hkdf::Hkdf;
use sha2::Sha256;

pub struct QuicInitialHkdf {
    inner: Hkdf<Sha256>,
    hkdf_label_buf: Vec<u8>,
}

impl QuicInitialHkdf {
    pub fn new(initial_salt: &[u8], cid: &[u8]) -> Self {
        let inner = Hkdf::new(Some(initial_salt), cid);
        QuicInitialHkdf {
            inner,
            hkdf_label_buf: Vec::with_capacity(32),
        }
    }

    pub fn set_prk(&mut self, prk: &[u8]) {
        if let Ok(hk) = Hkdf::<Sha256>::from_prk(prk) {
            self.inner = hk;
        }
    }

    pub fn expand_label(&mut self, label: &[u8], output: &mut [u8]) {
        self.hkdf_label_buf.clear();
        let len = output.len() as u16;
        let l_bytes = len.to_be_bytes();
        self.hkdf_label_buf.extend_from_slice(&l_bytes);
        let label_len = 6 + label.len() as u8;
        self.hkdf_label_buf.push(label_len);
        self.hkdf_label_buf.extend_from_slice(b"tls13 ");
        self.hkdf_label_buf
            .extend_from_slice(&label[..label_len as usize - 6]);
        self.hkdf_label_buf.push(0); // no context

        let _ = self.inner.expand(&self.hkdf_label_buf, output);
    }
}
