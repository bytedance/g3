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

use anyhow::anyhow;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, watch};
use tokio_rustls::server::TlsStream;

use g3_daemon::listen::ListenStats;
use g3_types::metrics::MetricsName;

use super::runtime::IntelliProxyRuntime;
use crate::config::server::intelli_proxy::IntelliProxyConfig;
use crate::config::server::{AnyServerConfig, ServerConfig};
use crate::serve::{
    ArcServer, Server, ServerInternal, ServerQuitPolicy, ServerReloadCommand, ServerRunContext,
};

pub(crate) struct IntelliProxy {
    name: MetricsName,
    config: ArcSwap<IntelliProxyConfig>,
    listen_stats: Arc<ListenStats>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,

    cfg_sender: watch::Sender<Option<IntelliProxyConfig>>,
    cfg_receiver: watch::Receiver<Option<IntelliProxyConfig>>,

    quit_policy: Arc<ServerQuitPolicy>,
    reload_version: usize,
}

impl IntelliProxy {
    fn new(
        config: Arc<IntelliProxyConfig>,
        listen_stats: Arc<ListenStats>,
        reload_version: usize,
    ) -> Self {
        let (reload_sender, _reload_receiver) = crate::serve::new_reload_notify_channel();

        let (cfg_sender, cfg_receiver) = watch::channel(Some(config.as_ref().clone()));

        IntelliProxy {
            name: config.name().clone(),
            config: ArcSwap::new(config),
            listen_stats,
            reload_sender,
            cfg_sender,
            cfg_receiver,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(config: AnyServerConfig) -> anyhow::Result<ArcServer> {
        if let AnyServerConfig::IntelliProxy(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::new(ListenStats::new(config.name()));

            let server = IntelliProxy::new(config, listen_stats, 1);
            Ok(Arc::new(server))
        } else {
            Err(anyhow!("invalid config type for IntelliProxy server"))
        }
    }

    fn prepare_reload(&self, config: AnyServerConfig) -> anyhow::Result<IntelliProxy> {
        if let AnyServerConfig::IntelliProxy(config) = config {
            let config = Arc::new(config);
            let listen_stats = Arc::clone(&self.listen_stats);

            let server = IntelliProxy::new(config, listen_stats, self.reload_version + 1);
            Ok(server)
        } else {
            let cur_config = self.config.load();
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                cur_config.server_type(),
                config.server_type()
            ))
        }
    }
}

impl ServerInternal for IntelliProxy {
    fn _clone_config(&self) -> AnyServerConfig {
        let cur_config = self.config.load();
        AnyServerConfig::IntelliProxy(cur_config.as_ref().clone())
    }

    fn _update_config_in_place(&self, _flags: u64, config: AnyServerConfig) -> anyhow::Result<()> {
        if let AnyServerConfig::IntelliProxy(config) = config {
            self.cfg_sender
                .send(Some(config.clone()))
                .map_err(|e| anyhow!("failed to send new cfg to runtime: {}", e))?;
            self.config.store(Arc::new(config));
            Ok(())
        } else {
            Err(anyhow!("invalid config type for IntelliProxy server"))
        }
    }

    fn _get_reload_notifier(&self) -> broadcast::Receiver<ServerReloadCommand> {
        self.reload_sender.subscribe()
    }

    // IntelliProxy do not support reload with old runtime/notifier
    fn _reload_config_notify_runtime(&self) {}
    fn _reload_escaper_notify_runtime(&self) {}
    fn _reload_user_group_notify_runtime(&self) {}
    fn _reload_auditor_notify_runtime(&self) {}

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

    fn _start_runtime(&self, server: &ArcServer) -> anyhow::Result<()> {
        let cur_config = self.config.load();
        let runtime = IntelliProxyRuntime::new(
            cur_config.as_ref().clone(),
            self.cfg_receiver.clone(),
            server,
        );
        runtime.run_all_instances()
    }

    fn _abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
        let _ = self.cfg_sender.send(None);
    }
}

#[async_trait]
impl Server for IntelliProxy {
    #[inline]
    fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }

    fn escaper(&self) -> &MetricsName {
        Default::default()
    }

    fn user_group(&self) -> &MetricsName {
        Default::default()
    }

    fn auditor(&self) -> &MetricsName {
        Default::default()
    }

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

    async fn run_tcp_task(
        &self,
        _stream: TcpStream,
        _peer_addr: SocketAddr,
        _local_addr: SocketAddr,
        _ctx: ServerRunContext,
    ) {
    }

    async fn run_tls_task(
        &self,
        _stream: TlsStream<TcpStream>,
        _peer_addr: SocketAddr,
        _local_addr: SocketAddr,
        _ctx: ServerRunContext,
    ) {
    }
}
