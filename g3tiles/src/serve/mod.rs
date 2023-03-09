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
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ServerQuitPolicy;

use crate::config::server::AnyServerConfig;

mod registry;
pub(crate) use registry::{foreach_online as foreach_server, get_names, get_with_notifier};

mod error;
pub(crate) use error::{ServerTaskError, ServerTaskResult};

mod runtime;
use runtime::{AuxiliaryServerConfig, AuxiliaryTcpPortRuntime, OrdinaryTcpServerRuntime};

mod dummy_close;
mod plain_tcp_port;

mod openssl_proxy;
mod rustls_proxy;

mod ops;
pub(crate) use ops::{
    force_quit_offline_server, force_quit_offline_servers, get_server, reload, stop_all,
    wait_all_tasks,
};
pub use ops::{spawn_all, spawn_offline_clean};

mod task;
pub(crate) use task::{ServerTaskNotes, ServerTaskStage};

mod stats;
pub(crate) use stats::{ArcServerStats, ServerStats};

#[derive(Clone)]
pub(crate) enum ServerReloadCommand {
    QuitRuntime,
    ReloadVersion(usize),
}

#[derive(Clone)]
pub(crate) struct ServerRunContext {
    pub(crate) worker_id: Option<usize>,
}

impl ServerRunContext {
    pub(crate) fn new() -> Self {
        ServerRunContext { worker_id: None }
    }
}

pub(crate) trait ServerInternal {
    fn _clone_config(&self) -> AnyServerConfig;
    fn _update_config_in_place(&self, flags: u64, config: AnyServerConfig) -> anyhow::Result<()>;

    fn _get_reload_notifier(&self) -> broadcast::Receiver<ServerReloadCommand>;
    fn _reload_config_notify_runtime(&self);

    fn _reload_with_old_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer>;
    fn _reload_with_new_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer>;

    fn _start_runtime(&self, server: &ArcServer) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

#[async_trait]
pub(crate) trait Server: ServerInternal {
    fn name(&self) -> &str;
    fn version(&self) -> usize;

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
}

pub(crate) type ArcServer = Arc<dyn Server + Send + Sync>;

fn new_reload_notify_channel() -> (
    broadcast::Sender<ServerReloadCommand>,
    broadcast::Receiver<ServerReloadCommand>,
) {
    broadcast::channel::<ServerReloadCommand>(16)
}
