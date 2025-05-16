/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use openssl::pkey::PKey;

pub(crate) async fn add_key(pem: &str) -> anyhow::Result<()> {
    let key = PKey::private_key_from_pem(pem.as_bytes())
        .map_err(|e| anyhow!("invalid private key content: {e}"))?;
    run_in_main_thread(async move { crate::store::add_global(key) }).await
}

pub(crate) async fn list_keys() -> anyhow::Result<Vec<Vec<u8>>> {
    run_in_main_thread(async move { Ok(crate::store::get_all_ski()) }).await
}

pub(crate) async fn check_key(ski: Vec<u8>) -> anyhow::Result<()> {
    run_in_main_thread(async move {
        crate::store::get_by_ski(&ski)
            .map(|_| ())
            .ok_or_else(|| anyhow!("key not found"))
    })
    .await
}

async fn run_in_main_thread<T, F>(future: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: Future<Output = anyhow::Result<T>> + Send + 'static,
{
    g3_daemon::runtime::main_handle()
        .ok_or(anyhow!("unable to get main runtime handle"))?
        .spawn(future)
        .await
        .map_err(|e| anyhow!("failed to spawn reload task: {e}"))?
}
