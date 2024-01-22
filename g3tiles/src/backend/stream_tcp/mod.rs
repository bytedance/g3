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

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use arc_swap::ArcSwapOption;
use async_trait::async_trait;

use g3_types::collection::{SelectiveVec, SelectiveVecBuilder, WeightedValue};
use g3_types::metrics::MetricsName;

use super::{ArcBackend, Backend};
use crate::config::backend::stream_tcp::StreamTcpBackendConfig;
use crate::config::backend::{AnyBackendConfig, BackendConfig};
use crate::module::stream::{StreamBackendDurationRecorder, StreamBackendDurationStats};

mod stats;
pub(crate) use stats::StreamTcpBackendStats;

pub(crate) struct StreamTcpBackend {
    config: Arc<StreamTcpBackendConfig>,
    stats: Arc<StreamTcpBackendStats>,
    duration_recorder: Arc<StreamBackendDurationRecorder>,
    duration_stats: Arc<StreamBackendDurationStats>,
    peer_addrs: Arc<ArcSwapOption<SelectiveVec<WeightedValue<SocketAddr>>>>,
}

impl StreamTcpBackend {
    fn new_obj(
        config: Arc<StreamTcpBackendConfig>,
        site_stats: Arc<StreamTcpBackendStats>,
        duration_recorder: Arc<StreamBackendDurationRecorder>,
        duration_stats: Arc<StreamBackendDurationStats>,
    ) -> anyhow::Result<ArcBackend> {
        let peer_addrs_container = Arc::new(ArcSwapOption::new(None));

        let discover = crate::discover::get_discover(&config.discover)?;
        let mut discover_receiver = discover
            .register_data(&config.discover_data)
            .context("failed to register to discover")?;

        // always update extra metrics tags
        site_stats.set_extra_tags(config.extra_metrics_tags.clone());
        duration_stats.set_extra_tags(config.extra_metrics_tags.clone());

        let peer_addrs = peer_addrs_container.clone();
        tokio::spawn(async move {
            while discover_receiver.changed().await.is_ok() {
                if let Ok(data) = discover_receiver.borrow().as_ref() {
                    let mut builder = SelectiveVecBuilder::new();
                    for v in data {
                        builder.insert(*v);
                    }
                    peer_addrs_container.store(builder.build().map(Arc::new));
                }
            }
        });

        Ok(Arc::new(StreamTcpBackend {
            config,
            stats: site_stats,
            duration_recorder,
            duration_stats,
            peer_addrs,
        }))
    }

    pub(super) fn prepare_initial(config: StreamTcpBackendConfig) -> anyhow::Result<ArcBackend> {
        let site_stats = Arc::new(StreamTcpBackendStats::new(config.name()));
        let (duration_recorder, duration_stats) =
            StreamBackendDurationRecorder::new(config.name(), &config.duration_stats);
        let duration_stats = Arc::new(duration_stats);

        // crate::stat::metrics::connector::keyless::push_connector_stats(site_stats.clone());
        // crate::stat::metrics::connector::keyless::push_duration_stats(duration_stats.clone());

        StreamTcpBackend::new_obj(
            Arc::new(config),
            site_stats,
            Arc::new(duration_recorder),
            duration_stats,
        )
    }

    fn prepare_reload(&self, config: StreamTcpBackendConfig) -> anyhow::Result<ArcBackend> {
        let stats = self.stats.clone();
        // TODO reuse old connection pool?
        StreamTcpBackend::new_obj(
            Arc::new(config),
            stats,
            self.duration_recorder.clone(),
            self.duration_stats.clone(),
        )
    }
}

#[async_trait]
impl Backend for StreamTcpBackend {
    fn _clone_config(&self) -> AnyBackendConfig {
        AnyBackendConfig::StreamTcp(self.config.as_ref().clone())
    }

    fn _update_config_in_place(
        &self,
        _flags: u64,
        _config: AnyBackendConfig,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn _lock_safe_reload(&self, config: AnyBackendConfig) -> anyhow::Result<ArcBackend> {
        if let AnyBackendConfig::StreamTcp(c) = config {
            self.prepare_reload(c)
        } else {
            Err(anyhow!("invalid backend config type"))
        }
    }

    #[inline]
    fn name(&self) -> &MetricsName {
        self.config.name()
    }
}
