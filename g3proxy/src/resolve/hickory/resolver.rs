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

use crate::config::resolver::hickory::HickoryResolverConfig;
use crate::config::resolver::{AnyResolverConfig, ResolverConfig};
use crate::resolve::{
    ArcIntegratedResolverHandle, BoxResolver, Resolver, ResolverInternal, ResolverStats,
};

pub(crate) struct HickoryResolver {
    config: Arc<HickoryResolverConfig>,
    inner: g3_resolver::Resolver,
    stats: Arc<ResolverStats>,
    logger: Arc<Logger>,
}

impl HickoryResolver {
    pub(crate) fn new_obj(config: HickoryResolverConfig) -> anyhow::Result<BoxResolver> {
        let mut builder = g3_resolver::ResolverBuilder::new((&config).into());
        builder.thread_name(format!("res-{}", config.name()));
        let resolver = builder.build()?;

        let logger = crate::log::resolve::get_logger(config.resolver_type(), config.name());
        let stats = ResolverStats::new(config.name(), resolver.get_stats());

        Ok(Box::new(HickoryResolver {
            config: Arc::new(config),
            inner: resolver,
            stats: Arc::new(stats),
            logger: Arc::new(logger),
        }))
    }
}

#[async_trait]
impl ResolverInternal for HickoryResolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }

    fn _clone_config(&self) -> AnyResolverConfig {
        AnyResolverConfig::Hickory(Box::new(self.config.as_ref().clone()))
    }

    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        _dep_table: BTreeMap<MetricsName, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()> {
        if let AnyResolverConfig::Hickory(config) = config {
            self.inner
                .update_config(config.as_ref().into())
                .context("failed to update inner hickory resolver config")?;
            self.config = Arc::new(*config);
            Ok(())
        } else {
            Err(anyhow!("invalid config type for HickoryResolver"))
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

impl Resolver for HickoryResolver {
    fn get_handle(&self) -> ArcIntegratedResolverHandle {
        let inner_context = self.inner.get_handle();
        Arc::new(super::HickoryResolverHandle::new(
            &self.config,
            inner_context,
            &self.logger,
        ))
    }

    fn get_stats(&self) -> Arc<ResolverStats> {
        Arc::clone(&self.stats)
    }
}
