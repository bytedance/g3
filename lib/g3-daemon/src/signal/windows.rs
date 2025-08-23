/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
