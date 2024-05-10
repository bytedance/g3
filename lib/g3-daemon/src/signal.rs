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

use std::future::{poll_fn, Future};

use anyhow::anyhow;
use log::info;

pub trait AsyncSignalAction: Copy {
    fn run(&self) -> impl Future<Output = ()> + Send;
}

pub fn register<QUIT, OFFLINE, RELOAD>(
    do_quit: QUIT,
    go_offline: OFFLINE,
    call_reload: RELOAD,
) -> anyhow::Result<()>
where
    QUIT: AsyncSignalAction + Send + 'static,
    OFFLINE: AsyncSignalAction + Send + 'static,
    RELOAD: AsyncSignalAction + Send + 'static,
{
    use tokio::signal::unix::{signal, SignalKind};

    let mut quit_sig = signal(SignalKind::quit())
        .map_err(|e| anyhow!("failed to create SIGQUIT listener: {e}"))?;
    tokio::spawn(async move {
        if poll_fn(|cx| quit_sig.poll_recv(cx)).await.is_some() {
            info!("got offline signal");
            do_quit.run().await;
        }
    });

    let mut int_sig = signal(SignalKind::interrupt())
        .map_err(|e| anyhow!("failed to create SIGINT listener: {e}"))?;
    tokio::spawn(async move {
        if poll_fn(|cx| int_sig.poll_recv(cx)).await.is_some() {
            info!("got offline signal");
            do_quit.run().await;
        }
    });

    let mut term_sig = signal(SignalKind::terminate())
        .map_err(|e| anyhow!("failed to create SIGTERM listener: {e}"))?;
    tokio::spawn(async move {
        if poll_fn(|cx| term_sig.poll_recv(cx)).await.is_some() {
            info!("got offline signal");
            go_offline.run().await;
        }
    });

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
