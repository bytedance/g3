/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use rustls_pki_types::CertificateDer;

pub fn load_native_certs_for_rustls() -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let mut r = rustls_native_certs::load_native_certs();
    if r.certs.is_empty() {
        match r.errors.pop() {
            Some(e) => Err(anyhow!("no certs loaded, the first error: {e}")),
            None => Err(anyhow!("no certs loaded, and no error reported")),
        }
    } else {
        Ok(r.certs)
    }
}
