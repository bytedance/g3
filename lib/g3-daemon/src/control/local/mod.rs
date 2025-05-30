/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Mutex;

use anyhow::anyhow;
use capnp_rpc::{RpcSystem, rpc_twoparty_capnp, twoparty};
use log::{debug, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
use tokio::sync::oneshot;

use g3_io_ext::LimitedWriteExt;

use super::{CtlProtoCtx, CtlProtoType, LocalControllerConfig};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::LocalControllerImpl;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::LocalControllerImpl;

static UNIQUE_CONTROLLER_ABORT_CHANNEL: Mutex<
    Option<oneshot::Sender<oneshot::Sender<LocalControllerImpl>>>,
> = Mutex::new(None);
static DAEMON_CONTROLLER_ABORT_CHANNEL: Mutex<
    Option<oneshot::Sender<oneshot::Sender<LocalControllerImpl>>>,
> = Mutex::new(None);

fn ctl_handle<R, W>(r: R, w: W)
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let ctx = CtlProtoCtx::new(
        BufReader::new(r),
        w,
        LocalControllerConfig::get_general(),
        CtlProtoType::Text,
    );
    tokio::spawn(async move {
        if let Err(e) = ctx.run().await {
            warn!("error handle client: {e}");
        }
    });
}

pub struct LocalController {
    inner: LocalControllerImpl,
}

impl LocalController {
    fn start(
        self,
        mutex: &Mutex<Option<oneshot::Sender<oneshot::Sender<LocalControllerImpl>>>>,
    ) -> anyhow::Result<impl Future + use<>> {
        let mut abort_channel = mutex.lock().unwrap();
        if abort_channel.is_some() {
            return Err(anyhow!("controller already existed"));
        }

        let (sender, receiver) = oneshot::channel();
        *abort_channel = Some(sender);
        let fut = async move { self.inner.into_running(receiver).await };
        Ok(fut)
    }

    async fn abort(mutex: &Mutex<Option<oneshot::Sender<oneshot::Sender<LocalControllerImpl>>>>) {
        let (sender, receiver) = oneshot::channel();

        let abort_channel = mutex.lock().unwrap().take();
        if let Some(quit_sender) = abort_channel {
            if quit_sender.send(sender).is_ok() {
                let _ = receiver.await;
            }
        }
    }

    pub fn listen_path(&self) -> String {
        self.inner.listen_path()
    }

    pub fn create_unique(daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let inner = LocalControllerImpl::create_unique(daemon_name, daemon_group)?;
        Ok(LocalController { inner })
    }

    pub fn start_as_unique(self) -> anyhow::Result<impl Future> {
        let fut = self.start(&UNIQUE_CONTROLLER_ABORT_CHANNEL)?;
        debug!("unique controller started");
        Ok(fut)
    }

    pub fn start_unique(
        daemon_name: &str,
        daemon_group: &str,
    ) -> anyhow::Result<impl Future + use<>> {
        LocalController::create_unique(daemon_name, daemon_group)?.start_as_unique()
    }

    pub async fn abort_unique() {
        LocalController::abort(&UNIQUE_CONTROLLER_ABORT_CHANNEL).await;
    }

    pub fn create_daemon(daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let inner = LocalControllerImpl::create_daemon(daemon_name, daemon_group)?;
        Ok(LocalController { inner })
    }

    pub fn start_as_daemon(self) -> anyhow::Result<impl Future> {
        let fut = self.start(&DAEMON_CONTROLLER_ABORT_CHANNEL)?;
        debug!("daemon controller started");
        Ok(fut)
    }

    pub fn start_daemon(
        daemon_name: &str,
        daemon_group: &str,
    ) -> anyhow::Result<impl Future + use<>> {
        LocalController::create_daemon(daemon_name, daemon_group)?.start_as_daemon()
    }

    pub async fn abort_daemon() {
        LocalController::abort(&DAEMON_CONTROLLER_ABORT_CHANNEL).await;
    }

    pub async fn send_release_controller_command(
        daemon_name: &str,
        daemon_group: &str,
    ) -> anyhow::Result<()> {
        Self::send_once_command(daemon_name, daemon_group, "release-controller\n").await
    }

    pub async fn send_cancel_shutdown_command(
        daemon_name: &str,
        daemon_group: &str,
    ) -> anyhow::Result<()> {
        Self::send_once_command(daemon_name, daemon_group, "cancel-shutdown\n").await
    }

    async fn send_once_command(
        daemon_name: &str,
        daemon_group: &str,
        command: &str,
    ) -> anyhow::Result<()> {
        let mut stream = LocalControllerImpl::connect_to_daemon(daemon_name, daemon_group).await?;
        stream
            .write_all_flush(command.as_bytes())
            .await
            .map_err(|e| anyhow!("failed to send {} command: {e}", command.trim_end()))?;
        let mut result = String::with_capacity(1);
        stream.read_to_string(&mut result).await?;
        Ok(())
    }

    pub async fn connect_rpc<T>(
        daemon_name: &str,
        daemon_group: &str,
    ) -> anyhow::Result<(RpcSystem<rpc_twoparty_capnp::Side>, T)>
    where
        T: capnp::capability::FromClientHook,
    {
        let mut stream = LocalControllerImpl::connect_to_daemon(daemon_name, daemon_group).await?;
        stream
            .write_all_flush(b"capnp\n")
            .await
            .map_err(|e| anyhow!("failed to send request: {e}"))?;

        let (reader, writer) = tokio::io::split(stream);
        let reader = tokio_util::compat::TokioAsyncReadCompatExt::compat(reader);
        let writer = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(writer);
        let rpc_network = Box::new(twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Client,
            Default::default(),
        ));
        let mut rpc_system = RpcSystem::new(rpc_network, None);
        let client: T = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
        Ok((rpc_system, client))
    }
}
