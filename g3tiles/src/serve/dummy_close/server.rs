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

use anyhow::anyhow;
use async_trait::async_trait;
#[cfg(feature = "quic")]
use quinn::Connection;
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use g3_daemon::listen::{
    AcceptQuicServer, AcceptTcpServer, ArcAcceptQuicServer, ArcAcceptTcpServer, ListenStats,
};
use g3_daemon::server::{BaseServer, ClientConnectionInfo, ServerReloadCommand};
use g3_types::metrics::MetricsName;

use crate::config::server::dummy_close::DummyCloseServerConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{ArcServer, Server, ServerInternal, ServerQuitPolicy};

pub(crate) struct DummyCloseServer {
    config: DummyCloseServerConfig,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    quit_policy: Arc<ServerQuitPolicy>,
}

impl DummyCloseServer {
    fn new(config: DummyCloseServerConfig, listen_stats: Arc<ListenStats>) -> Self {
        let reload_sender = crate::serve::new_reload_notify_channel();

        DummyCloseServer {
            config,
            listen_stats,
            reload_sender,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
        }
    }

    pub(crate) fn prepare_initial(config: DummyCloseServerConfig) -> anyhow::Result<ArcServer> {
        let listen_stats = Arc::new(ListenStats::new(config.name()));

        let server = DummyCloseServer::new(config, listen_stats);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &MetricsName) -> ArcServer {
        let config = DummyCloseServerConfig::new(name, None);
        let listen_stats = Arc::new(ListenStats::new(name));
        Arc::new(DummyCloseServer::new(config, listen_stats))
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<DummyCloseServer> {
        if let AnyServerConfig::DummyClose(config) = config {
            let listen_stats = Arc::clone(&self.listen_stats);

            let server = DummyCloseServer::new(config, listen_stats);
            Ok(server)
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.server_type(),
                config.server_type()
            ))
        }
    }
}

impl ServerInternal for DummyCloseServer {
    fn _clone_config(&self) -> AnyServerConfig {
        AnyServerConfig::DummyClose(self.config.clone())
    }

    fn _update_config_in_place(&self, _flags: u64, _config: AnyServerConfig) -> anyhow::Result<()> {
        Ok(())
    }

    fn _depend_on_server(&self, _name: &MetricsName) -> bool {
        false
    }

    // DummyClose server do not support reload with old runtime/notifier
    fn _reload_config_notify_runtime(&self) {}

    fn _update_next_servers_in_place(&self) {}

    fn _reload_with_old_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        Err(anyhow!(
            "this {} server doesn't support reload with old notifier",
            config.server_type()
        ))
    }

    fn _reload_with_new_notifier(&self, config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, _server: &ArcServer) -> anyhow::Result<()> {
        Ok(())
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
    }
}

impl BaseServer for DummyCloseServer {
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn server_type(&self) -> &'static str {
        self.config.server_type()
    }

    fn version(&self) -> usize {
        0
    }
}

#[async_trait]
impl AcceptTcpServer for DummyCloseServer {
    async fn run_tcp_task(&self, _stream: TcpStream, _cc_info: ClientConnectionInfo) {}

    fn get_reloaded(&self) -> ArcAcceptTcpServer {
        crate::serve::get_or_insert_default(self.config.name())
    }
}

#[async_trait]
impl AcceptQuicServer for DummyCloseServer {
    #[cfg(feature = "quic")]
    async fn run_quic_task(&self, _connection: Connection, _cc_info: ClientConnectionInfo) {}

    fn get_reloaded(&self) -> ArcAcceptQuicServer {
        crate::serve::get_or_insert_default(self.config.name())
    }
}

#[async_trait]
impl Server for DummyCloseServer {
    fn get_listen_stats(&self) -> Arc<ListenStats> {
        Arc::clone(&self.listen_stats)
    }

    fn alive_count(&self) -> i32 {
        0
    }

    #[inline]
    fn quit_policy(&self) -> &Arc<ServerQuitPolicy> {
        &self.quit_policy
    }
}
