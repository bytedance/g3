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

use std::future::Future;
use std::sync::Mutex;

use anyhow::anyhow;
use futures_util::future::{AbortHandle, Abortable};
use log::{debug, warn};
use tokio::io::{AsyncRead, AsyncWrite, BufReader};

use super::{CtlProtoCtx, CtlProtoType, LocalControllerConfig};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::LocalControllerImpl;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::LocalControllerImpl;

static UNIQUE_CONTROLLER_ABORT_HANDLER: Mutex<Option<AbortHandle>> = Mutex::new(None);
static DAEMON_CONTROLLER_ABORT_HANDLER: Mutex<Option<AbortHandle>> = Mutex::new(None);

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
    fn start(self, mutex: &Mutex<Option<AbortHandle>>) -> anyhow::Result<impl Future> {
        let mut abort_handler_container = mutex.lock().unwrap();
        if abort_handler_container.is_some() {
            return Err(anyhow!("controller already existed"));
        }

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let future = Abortable::new(self.inner.into_running(), abort_registration);
        *abort_handler_container = Some(abort_handle);

        Ok(future)
    }

    fn abort(mutex: &Mutex<Option<AbortHandle>>) {
        let mut abort_handler_container = mutex.lock().unwrap();
        if let Some(abort_handle) = abort_handler_container.take() {
            abort_handle.abort();
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
        let fut = self.start(&UNIQUE_CONTROLLER_ABORT_HANDLER)?;
        debug!("unique controller started");
        Ok(fut)
    }

    pub fn start_unique(daemon_name: &str, daemon_group: &str) -> anyhow::Result<impl Future> {
        LocalController::create_unique(daemon_name, daemon_group)?.start_as_unique()
    }

    pub fn abort_unique() {
        LocalController::abort(&UNIQUE_CONTROLLER_ABORT_HANDLER);
    }

    pub fn create_daemon(daemon_name: &str, daemon_group: &str) -> anyhow::Result<Self> {
        let inner = LocalControllerImpl::create_daemon(daemon_name, daemon_group)?;
        Ok(LocalController { inner })
    }

    pub fn start_as_daemon(self) -> anyhow::Result<impl Future> {
        let fut = self.start(&DAEMON_CONTROLLER_ABORT_HANDLER)?;
        debug!("daemon controller started");
        Ok(fut)
    }

    pub fn start_daemon(daemon_name: &str, daemon_group: &str) -> anyhow::Result<impl Future> {
        LocalController::create_daemon(daemon_name, daemon_group)?.start_as_daemon()
    }

    pub fn abort_daemon() {
        LocalController::abort(&DAEMON_CONTROLLER_ABORT_HANDLER);
    }
}
