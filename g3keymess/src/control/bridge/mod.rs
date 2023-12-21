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

use std::future::Future;

use anyhow::anyhow;
use openssl::pkey::PKey;

pub(crate) async fn offline() -> anyhow::Result<()> {
    run_in_main_thread(async move {
        crate::control::DaemonController::abort().await;
        Ok(())
    })
    .await
}

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
