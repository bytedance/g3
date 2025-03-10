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
use log::info;
use tokio::signal::windows::ctrl_c;

use super::AsyncSignalAction;

pub fn register_quit<QUIT>(do_quit: QUIT) -> anyhow::Result<()>
where
    QUIT: AsyncSignalAction + Send + 'static,
{
    let mut quit_sig = ctrl_c().map_err(|e| anyhow!("failed to create Ctrl-C listener: {e}"))?;
    tokio::spawn(async move {
        if quit_sig.recv().await.is_some() {
            info!("got quit signal");
            do_quit.run().await;
        }
    });

    Ok(())
}
