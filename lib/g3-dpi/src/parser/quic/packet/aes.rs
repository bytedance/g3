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

use openssl::cipher::Cipher;
use openssl::cipher_ctx::CipherCtx;
use openssl::error::ErrorStack;

pub(super) fn aes_ecb_mask(key: &[u8], sample: &[u8]) -> Result<[u8; 5], ErrorStack> {
    let mut cipher_ctx = CipherCtx::new()?;
    cipher_ctx.encrypt_init(Some(Cipher::aes_128_ecb()), Some(key), None)?;
    let mut mask = [0u8; 32];
    cipher_ctx.cipher_update(sample, Some(&mut mask))?;
    Ok([mask[0], mask[1], mask[2], mask[3], mask[4]])
}

pub(super) fn aes_gcm_decrypt(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, ErrorStack> {
    let mut cipher_ctx = CipherCtx::new()?;
    cipher_ctx.decrypt_init(Some(Cipher::aes_128_gcm()), Some(key), Some(iv))?;
    let mut output = Vec::with_capacity(ciphertext.len());
    cipher_ctx.cipher_update(aad, None)?;
    cipher_ctx.cipher_update_vec(ciphertext, &mut output)?;
    cipher_ctx.set_tag(tag)?;
    let mut data = [0u8; 16];
    cipher_ctx.cipher_final(&mut data)?; // error means verify failed
    Ok(output)
}
