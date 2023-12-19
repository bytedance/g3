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

use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_daemon::server::{ClientConnectionInfo, ServerQuitPolicy, ServerReloadCommand};
use g3_openssl::SslStream;
use g3_types::metrics::MetricsName;

use crate::config::server::AnyServerConfig;

mod registry;
pub(crate) use registry::{foreach_online as foreach_server, get_names, get_or_insert_default};

mod idle_check;
pub(crate) use idle_check::ServerIdleChecker;

mod runtime;
#[cfg(feature = "quic")]
use runtime::ListenQuicRuntime;
use runtime::ListenTcpRuntime;

mod dummy_close;
mod intelli_proxy;
mod native_tls_port;
#[cfg(feature = "quic")]
mod plain_quic_port;
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

pub(crate) trait ServerInternal {
    fn _clone_config(&self) -> AnyServerConfig;
    fn _update_config_in_place(&self, flags: u64, config: AnyServerConfig) -> anyhow::Result<()>;

    fn _depend_on_server(&self, name: &MetricsName) -> bool;
    fn _reload_config_notify_runtime(&self);
    fn _update_next_servers_in_place(&self);
    fn _update_escaper_in_place(&self);
    fn _update_user_group_in_place(&self);
    fn _update_audit_handle_in_place(&self) -> anyhow::Result<()>;

    fn _reload_with_old_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer>;
    fn _reload_with_new_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer>;

    fn _start_runtime(&self, server: &ArcServer) -> anyhow::Result<()>;
    fn _abort_runtime(&self);
}

#[async_trait]
pub(crate) trait Server: ServerInternal {
    fn name(&self) -> &MetricsName;
    fn version(&self) -> usize;
    fn escaper(&self) -> &MetricsName;
    fn user_group(&self) -> &MetricsName;
    fn auditor(&self) -> &MetricsName;

    fn get_server_stats(&self) -> Option<ArcServerStats> {
        None
    }
    fn get_listen_stats(&self) -> Arc<ListenStats>;

    fn alive_count(&self) -> i32;
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy>;

    async fn run_tcp_task(&self, stream: TcpStream, cc_info: ClientConnectionInfo);

    async fn run_rustls_task(&self, stream: TlsStream<TcpStream>, cc_info: ClientConnectionInfo);

    async fn run_openssl_task(&self, stream: SslStream<TcpStream>, cc_info: ClientConnectionInfo);

    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, connection: Connection, cc_info: ClientConnectionInfo);
}

pub(crate) type ArcServer = Arc<dyn Server + Send + Sync>;

fn new_reload_notify_channel() -> broadcast::Sender<ServerReloadCommand> {
    broadcast::Sender::new(16)
}
