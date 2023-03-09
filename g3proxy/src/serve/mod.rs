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

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use log::warn;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ServerQuitPolicy;

use crate::audit::AuditHandle;
use crate::auth::UserGroup;
use crate::config::server::AnyServerConfig;
use crate::escape::ArcEscaper;

mod registry;
pub(crate) use registry::{foreach_online as foreach_server, get_names, get_with_notifier};

mod idle_check;
pub(crate) use idle_check::ServerIdleChecker;

mod runtime;
use runtime::{AuxiliaryServerConfig, AuxiliaryTcpPortRuntime, OrdinaryTcpServerRuntime};

mod dummy_close;
mod intelli_proxy;
mod plain_tcp_port;
mod plain_tls_port;

mod http_proxy;
mod http_rproxy;
mod sni_proxy;
mod socks_proxy;
mod tcp_stream;
mod tls_stream;

mod error;
mod task;

pub(crate) use error::{ServerTaskError, ServerTaskForbiddenError, ServerTaskResult};
pub(crate) use task::{ServerTaskNotes, ServerTaskStage};

mod ops;
pub(crate) use ops::{
    force_quit_offline_server, force_quit_offline_servers, get_server, reload, stop_all,
    update_dependency_to_auditor, update_dependency_to_escaper, update_dependency_to_user_group,
    wait_all_tasks,
};
pub use ops::{spawn_all, spawn_offline_clean};

mod stats;
pub(crate) use stats::{
    ArcServerStats, ServerForbiddenSnapshot, ServerForbiddenStats, ServerPerTaskStats, ServerStats,
};

#[derive(Clone)]
pub(crate) enum ServerReloadCommand {
    QuitRuntime,
    ReloadVersion(usize),
    ReloadEscaper,
    ReloadUserGroup,
    ReloadAuditor,
}

#[derive(Clone)]
pub(crate) struct ServerRunContext {
    pub(crate) escaper: ArcEscaper,
    pub(crate) user_group: Option<Arc<UserGroup>>,
    pub(crate) audit_handle: Option<Arc<AuditHandle>>,
    pub(crate) worker_id: Option<usize>,
}

impl ServerRunContext {
    pub(crate) fn new(escaper: &str, user_group: &str, auditor: &str) -> Self {
        let mut ctx = ServerRunContext {
            escaper: crate::escape::get_or_insert_default(escaper),
            user_group: None,
            audit_handle: None,
            worker_id: None,
        };
        ctx.update_user_group(user_group);
        ctx.update_audit_handle(auditor);
        ctx
    }

    pub(crate) fn current_escaper(&self) -> &str {
        self.escaper.name()
    }

    pub(crate) fn update_escaper(&mut self, escaper: &str) {
        self.escaper = crate::escape::get_or_insert_default(escaper);
    }

    pub(crate) fn current_user_group(&self) -> &str {
        self.user_group
            .as_ref()
            .map(|ug| ug.name())
            .unwrap_or_default()
    }

    pub(crate) fn update_user_group(&mut self, user_group: &str) {
        if user_group.is_empty() {
            self.user_group = None;
        } else {
            let user_group = crate::auth::get_or_insert_default(user_group);
            self.user_group = Some(user_group);
        }
    }

    pub(crate) fn current_auditor(&self) -> &str {
        self.audit_handle
            .as_ref()
            .map(|h| h.name())
            .unwrap_or_default()
    }

    pub(crate) fn update_audit_handle(&mut self, auditor_name: &str) {
        if auditor_name.is_empty() {
            self.audit_handle = None;
        } else {
            let auditor = crate::audit::get_or_insert_default(auditor_name);
            match auditor.build_handle() {
                Ok(handle) => self.audit_handle = Some(handle),
                Err(e) => {
                    warn!("error when build audit handle for auditor {auditor_name}: {e:?}",);
                    self.audit_handle = None;
                }
            }
        }
    }
}

pub(crate) trait ServerInternal {
    fn _clone_config(&self) -> AnyServerConfig;
    fn _update_config_in_place(&self, flags: u64, config: AnyServerConfig) -> anyhow::Result<()>;

    fn _get_reload_notifier(&self) -> broadcast::Receiver<ServerReloadCommand>;
    fn _reload_config_notify_runtime(&self);
    fn _reload_escaper_notify_runtime(&self);
    fn _reload_user_group_notify_runtime(&self);
    fn _reload_auditor_notify_runtime(&self);

    fn _reload_with_old_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer>;
    fn _reload_with_new_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer>;

    fn _start_runtime(&self, server: &ArcServer) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

#[async_trait]
pub(crate) trait Server: ServerInternal {
    fn name(&self) -> &str;
    fn version(&self) -> usize;
    fn escaper(&self) -> String;
    fn user_group(&self) -> String;
    fn auditor(&self) -> String;

    fn get_server_stats(&self) -> Option<ArcServerStats> {
        None
    }
    fn get_listen_stats(&self) -> Arc<ListenStats>;

    fn alive_count(&self) -> i32;
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy>;

    async fn run_tcp_task(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
        ctx: ServerRunContext,
    );

    async fn run_tls_task(
        &self,
        _stream: TlsStream<TcpStream>,
        _peer_addr: SocketAddr,
        _local_addr: SocketAddr,
        _ctx: ServerRunContext,
    );
}

pub(crate) type ArcServer = Arc<dyn Server + Send + Sync>;

fn new_reload_notify_channel() -> (
    broadcast::Sender<ServerReloadCommand>,
    broadcast::Receiver<ServerReloadCommand>,
) {
    broadcast::channel::<ServerReloadCommand>(16)
}
