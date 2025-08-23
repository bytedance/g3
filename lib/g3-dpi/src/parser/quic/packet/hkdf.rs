/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use openssl::error::ErrorStack;
use openssl::md::Md;
use openssl::pkey::Id;
use openssl::pkey_ctx::{HkdfMode, PkeyCtx};
use smallvec::SmallVec;

fn build_info_from_label(label: &[u8], output_len: u16) -> SmallVec<[u8; 32]> {
    let label_len = 6 + label.len() as u8;
    let mut info: SmallVec<[u8; 32]> = SmallVec::with_capacity(2 + 1 + label_len as usize + 1);
    let l_bytes = output_len.to_be_bytes();
    info.extend_from_slice(&l_bytes);
    info.push(label_len);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(&label[..label_len as usize - 6]);
    info.push(0); // no context
    info
}

pub(super) fn quic_hkdf_extract_expand(
    salt: &[u8],
    ikm: &[u8],
    label: &[u8],
    output: &mut [u8],
) -> Result<(), ErrorStack> {
    let mut pkey_ctx = PkeyCtx::new_id(Id::HKDF)?;
    pkey_ctx.derive_init()?;
    pkey_ctx.set_hkdf_mode(HkdfMode::EXTRACT_THEN_EXPAND)?;
    pkey_ctx.set_hkdf_md(Md::sha256())?;
    pkey_ctx.set_hkdf_salt(salt)?;
    pkey_ctx.set_hkdf_key(ikm)?;

    let info = build_info_from_label(label, output.len() as u16);
    pkey_ctx.add_hkdf_info(&info)?;

    pkey_ctx.derive(Some(output))?;
    Ok(())
}

pub(super) fn quic_hkdf_expand(
    prk: &[u8],
    label: &[u8],
    output: &mut [u8],
) -> Result<(), ErrorStack> {
    let mut pkey_ctx = PkeyCtx::new_id(Id::HKDF)?;
    pkey_ctx.derive_init()?;
    pkey_ctx.set_hkdf_mode(HkdfMode::EXPAND_ONLY)?;
    pkey_ctx.set_hkdf_md(Md::sha256())?;
    pkey_ctx.set_hkdf_key(prk)?;

    let info = build_info_from_label(label, output.len() as u16);
    pkey_ctx.add_hkdf_info(&info)?;

    pkey_ctx.derive(Some(output))?;
    Ok(())
}
