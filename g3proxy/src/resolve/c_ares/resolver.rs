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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use slog::Logger;

use g3_types::metrics::MetricsName;

use crate::config::resolver::c_ares::CAresResolverConfig;
use crate::config::resolver::{AnyResolverConfig, ResolverConfig};
use crate::resolve::{
    ArcIntegratedResolverHandle, BoxResolver, Resolver, ResolverInternal, ResolverStats,
};

pub(crate) struct CAresResolver {
    config: Arc<CAresResolverConfig>,
    inner: g3_resolver::Resolver,
    stats: Arc<ResolverStats>,
    logger: Arc<Logger>,
}

impl CAresResolver {
    pub(crate) fn new_obj(config: AnyResolverConfig) -> anyhow::Result<BoxResolver> {
        if let AnyResolverConfig::CAres(config) = config {
            let mut builder = g3_resolver::ResolverBuilder::new((&config).into());
            builder.thread_name(format!("res-{}", config.name()));
            let resolver = builder.build()?;

            let logger = crate::log::resolve::get_logger(config.resolver_type(), config.name());
            let stats = ResolverStats::new(config.name(), resolver.get_stats());

            Ok(Box::new(CAresResolver {
                config: Arc::new(config),
                inner: resolver,
                stats: Arc::new(stats),
                logger: Arc::new(logger),
            }))
        } else {
            Err(anyhow!("invalid config type for CAresResolver"))
        }
    }
}

#[async_trait]
impl ResolverInternal for CAresResolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }

    fn _clone_config(&self) -> AnyResolverConfig {
        AnyResolverConfig::CAres(self.config.as_ref().clone())
    }

    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        _dep_table: BTreeMap<MetricsName, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()> {
        if let AnyResolverConfig::CAres(config) = config {
            self.inner
                .update_config((&config).into())
                .context("failed to update inner c_ares resolver config")?;
            self.config = Arc::new(config);
            Ok(())
        } else {
            Err(anyhow!("invalid config type for CAresResolver"))
        }
    }

    fn _update_dependent_handle(
        &mut self,
        _target: &MetricsName,
        _handle: ArcIntegratedResolverHandle,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn _shutdown(&mut self) {
        self.inner.shutdown().await;
    }
}

impl Resolver for CAresResolver {
    fn get_handle(&self) -> ArcIntegratedResolverHandle {
        let inner_context = self.inner.get_handle();
        Arc::new(super::CAresResolverHandle::new(
            &self.config,
            inner_context,
            &self.logger,
        ))
    }

    fn get_stats(&self) -> Arc<ResolverStats> {
        Arc::clone(&self.stats)
    }
}
