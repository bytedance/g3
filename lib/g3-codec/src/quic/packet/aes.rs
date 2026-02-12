/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
    aad_vec: &[&[u8]],
    ciphertext: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, ErrorStack> {
    let mut cipher_ctx = CipherCtx::new()?;
    cipher_ctx.decrypt_init(Some(Cipher::aes_128_gcm()), Some(key), Some(iv))?;
    let mut output = Vec::with_capacity(ciphertext.len());
    for aad in aad_vec {
        cipher_ctx.cipher_update(aad, None)?;
    }
    cipher_ctx.cipher_update_vec(ciphertext, &mut output)?;
    cipher_ctx.set_tag(tag)?;
    let mut data = [0u8; 16];
    cipher_ctx.cipher_final(&mut data)?; // error means verify failed
    Ok(output)
}
