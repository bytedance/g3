/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;

use anyhow::anyhow;
use log::info;
use tokio::signal::unix::{SignalKind, signal};

use super::AsyncSignalAction;

pub fn register_quit<QUIT>(do_quit: QUIT) -> anyhow::Result<()>
where
    QUIT: AsyncSignalAction + Send + 'static,
{
    let mut quit_sig = signal(SignalKind::quit())
        .map_err(|e| anyhow!("failed to create SIGQUIT listener: {e}"))?;
    tokio::spawn(async move {
        if poll_fn(|cx| quit_sig.poll_recv(cx)).await.is_some() {
            info!("got quit signal");
            do_quit.run().await;
        }
    });

    let mut int_sig = signal(SignalKind::interrupt())
        .map_err(|e| anyhow!("failed to create SIGINT listener: {e}"))?;
    tokio::spawn(async move {
        if poll_fn(|cx| int_sig.poll_recv(cx)).await.is_some() {
            info!("got quit signal");
            do_quit.run().await;
        }
    });

    Ok(())
}

pub fn register_offline<OFFLINE>(go_offline: OFFLINE) -> anyhow::Result<()>
where
    OFFLINE: AsyncSignalAction + Send + 'static,
{
    let mut term_sig = signal(SignalKind::terminate())
        .map_err(|e| anyhow!("failed to create SIGTERM listener: {e}"))?;
    tokio::spawn(async move {
        if poll_fn(|cx| term_sig.poll_recv(cx)).await.is_some() {
            info!("got offline signal");
            go_offline.run().await;
        }
    });

    Ok(())
}

pub fn register_reload<RELOAD>(call_reload: RELOAD) -> anyhow::Result<()>
where
    RELOAD: AsyncSignalAction + Send + 'static,
{
    let mut hup_sig = signal(SignalKind::hangup())
        .map_err(|e| anyhow!("failed to create SIGHUP listener: {e}"))?;
    tokio::spawn(async move {
        loop {
            if poll_fn(|cx| hup_sig.poll_recv(cx)).await.is_none() {
                break;
            }
            info!("got reload signal");
            call_reload.run().await;
        }
    });

    Ok(())
}
