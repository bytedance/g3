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
use std::pin::Pin;
use std::task::{Context, Poll};

use log::{info, warn};
use tokio::sync::broadcast;

use g3_types::metrics::MetricsName;

use crate::serve::{ArcServer, ServerReloadCommand, ServerRunContext};

struct ContextValue {
    server: ArcServer,
    server_notifier: broadcast::Receiver<ServerReloadCommand>,
    server_context: ServerRunContext,
}

impl ContextValue {
    fn new(name: &MetricsName) -> Self {
        let (server, receiver) = crate::serve::get_with_notifier(name);
        let context =
            ServerRunContext::new(server.escaper(), server.user_group(), server.auditor());
        ContextValue {
            server,
            server_notifier: receiver,
            server_context: context,
        }
    }

    fn reload(&mut self, log_prefix: &str, name: &MetricsName) {
        let (server, receiver) = crate::serve::get_with_notifier(name);
        self.server = server;
        self.server_notifier = receiver;

        // if escaper changed, reload it
        let old_escaper = self.server_context.current_escaper();
        let new_escaper = self.server.escaper();
        if old_escaper.ne(new_escaper) {
            info!(
                "{log_prefix}/{name} will use escaper '{new_escaper}' instead of '{old_escaper}'"
            );
            self.server_context.update_escaper(new_escaper);
        }

        // if user group changed, reload it
        let old_user_group = self.server_context.current_user_group();
        let new_user_group = self.server.user_group();
        if old_user_group.ne(new_user_group) {
            info!(
                "{log_prefix}/{name} will use user group '{new_user_group}' instead of '{old_user_group}'"
            );
            self.server_context.update_user_group(new_user_group);
        }

        // if auditor changed, reload it
        let old_auditor = self.server_context.current_auditor();
        let new_auditor = self.server.auditor();
        if old_auditor.ne(new_auditor) {
            info!(
                "{log_prefix}/{name} will use auditor '{new_auditor}' instead of '{old_auditor}'"
            );
            self.server_context.update_audit_handle(new_auditor);
        }
    }
}

pub(crate) struct AuxiliaryRunContext {
    log_prefix: String,
    lists: Vec<ContextValue>,
}

impl AuxiliaryRunContext {
    pub(crate) fn new(log_prefix: String) -> Self {
        AuxiliaryRunContext {
            log_prefix,
            lists: Vec::new(),
        }
    }

    pub(crate) fn add_server(&mut self, name: &MetricsName) -> usize {
        let v = ContextValue::new(name);
        let id = self.lists.len();
        self.lists.push(v);
        id
    }

    /// Get task run context
    ///
    /// # Safety
    ///
    /// The index should be returned by method `add_server`.
    pub(crate) unsafe fn get_unchecked(&self, index: usize) -> (ArcServer, ServerRunContext) {
        let v = self.lists.get_unchecked(index);
        (v.server.clone(), v.server_context.clone())
    }

    pub(crate) fn reload(&mut self, index: usize, name: &MetricsName) {
        if let Some(v) = self.lists.get_mut(index) {
            v.reload(&self.log_prefix, name);
        }
    }

    pub(crate) async fn check_reload(&mut self) {
        let mut futures = Vec::new();
        for (n, v) in &mut self.lists.iter_mut().enumerate() {
            let f = v.server_notifier.recv();
            futures.push((n, Box::pin(f)));
        }

        let batch_recv = BatchRecv { values: futures };
        let (index, cmd) = batch_recv.await;
        let v = self.lists.get_mut(index).unwrap();
        let name = v.server.name().clone();
        match cmd {
            Ok(ServerReloadCommand::ReloadVersion(version)) => {
                info!("{}/{name} reload to v{version}", self.log_prefix);
                v.reload(&self.log_prefix, &name);
            }
            Ok(ServerReloadCommand::ReloadEscaper) => {
                let escaper_name = v.server.escaper();
                info!(
                    "{}/{name} will reload escaper {escaper_name}",
                    self.log_prefix
                );
                v.server_context.update_escaper(escaper_name);
            }
            Ok(ServerReloadCommand::ReloadUserGroup) => {
                let user_group_name = v.server.user_group();
                info!(
                    "{}/{name} will reload user group {user_group_name}",
                    self.log_prefix
                );
                v.server_context.update_user_group(user_group_name);
            }
            Ok(ServerReloadCommand::ReloadAuditor) => {
                let auditor_name = v.server.auditor();
                info!(
                    "{}/{name} will reload auditor {auditor_name}",
                    self.log_prefix
                );
                v.server_context.update_audit_handle(auditor_name);
            }
            Ok(ServerReloadCommand::QuitRuntime) | Err(broadcast::error::RecvError::Closed) => {
                info!("{}/{name} server quit, reload it", self.log_prefix);
                v.reload(&self.log_prefix, &name);
            }
            Err(broadcast::error::RecvError::Lagged(dropped)) => {
                warn!(
                    "{}/{name} reload notify channel overflowed, {dropped} msg dropped",
                    self.log_prefix
                );
            }
        }
    }
}

struct BatchRecv<F>
where
    F: Future<Output = Result<ServerReloadCommand, broadcast::error::RecvError>>,
{
    values: Vec<(usize, F)>,
}

impl<F> Future for BatchRecv<F>
where
    F: Future<Output = Result<ServerReloadCommand, broadcast::error::RecvError>> + Unpin,
{
    type Output = (
        usize,
        Result<ServerReloadCommand, broadcast::error::RecvError>,
    );

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        for (n, f) in &mut self.values {
            match Pin::new(f).poll(cx) {
                Poll::Ready(v) => return Poll::Ready((*n, v)),
                Poll::Pending => {}
            }
        }
        Poll::Pending
    }
}
