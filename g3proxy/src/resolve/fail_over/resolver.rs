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

use g3_resolver::driver::fail_over::FailOverDriverConfig;
use g3_types::metrics::MetricsName;

use crate::config::resolver::fail_over::FailOverResolverConfig;
use crate::config::resolver::{AnyResolverConfig, ResolverConfig};
use crate::resolve::{
    ArcIntegratedResolverHandle, BoxResolver, Resolver, ResolverInternal, ResolverStats,
};

pub(crate) struct FailOverResolver {
    config: Arc<FailOverResolverConfig>,
    driver_config: FailOverDriverConfig,
    inner: g3_resolver::Resolver,
    stats: Arc<ResolverStats>,
    logger: Arc<Logger>,
}

impl FailOverResolver {
    pub(crate) fn new_obj(config: AnyResolverConfig) -> anyhow::Result<BoxResolver> {
        if let AnyResolverConfig::FailOver(config) = config {
            let mut driver_config = FailOverDriverConfig::default();

            let primary_handle = crate::resolve::get_handle(&config.primary)
                .context("failed to get primary resolver handle")?;
            let standby_handle = crate::resolve::get_handle(&config.standby)
                .context("failed to get standby resolver handle")?;
            driver_config.set_primary_handle(primary_handle.clone_inner());
            driver_config.set_standby_handle(standby_handle.clone_inner());
            driver_config.set_static_config(config.static_conf);

            let inner_config = g3_resolver::ResolverConfig {
                name: config.name().to_string(),
                runtime: config.runtime.clone(),
                driver: g3_resolver::AnyResolveDriverConfig::FailOver(driver_config.clone()),
            };
            let mut builder = g3_resolver::ResolverBuilder::new(inner_config);
            builder.thread_name(format!("res-{}", config.name()));
            let resolver = builder.build()?;

            let logger = crate::log::resolve::get_logger(config.resolver_type(), config.name());
            let stats = ResolverStats::new(config.name(), resolver.get_stats());

            Ok(Box::new(FailOverResolver {
                config: Arc::new(config),
                driver_config,
                inner: resolver,
                stats: Arc::new(stats),
                logger: Arc::new(logger),
            }))
        } else {
            Err(anyhow!("invalid config type for FailOverResolver"))
        }
    }
}

#[async_trait]
impl ResolverInternal for FailOverResolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<MetricsName>> {
        self.config.dependent_resolver()
    }

    fn _clone_config(&self) -> AnyResolverConfig {
        AnyResolverConfig::FailOver(self.config.as_ref().clone())
    }

    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        dep_table: BTreeMap<MetricsName, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()> {
        if let AnyResolverConfig::FailOver(config) = config {
            let mut driver_config = FailOverDriverConfig::default();

            let primary_handle = dep_table.get(&config.primary).unwrap();
            let standby_handle = dep_table.get(&config.standby).unwrap();
            driver_config.set_primary_handle(primary_handle.clone_inner());
            driver_config.set_standby_handle(standby_handle.clone_inner());
            driver_config.set_static_config(config.static_conf);

            let inner_config = g3_resolver::ResolverConfig {
                name: config.name().to_string(),
                runtime: config.runtime.clone(),
                driver: g3_resolver::AnyResolveDriverConfig::FailOver(driver_config.clone()),
            };

            self.inner
                .update_config(inner_config)
                .context("failed to update inner fail_over resolver config")?;
            self.driver_config = driver_config;
            self.config = Arc::new(config);
            Ok(())
        } else {
            Err(anyhow!("invalid config type for FailOverResolver"))
        }
    }

    fn _update_dependent_handle(
        &mut self,
        target: &MetricsName,
        handle: ArcIntegratedResolverHandle,
    ) -> anyhow::Result<()> {
        let mut driver_config = self.driver_config.clone();
        if self.config.primary.eq(target) {
            driver_config.set_primary_handle(handle.clone_inner());
        } else if self.config.standby.eq(target) {
            driver_config.set_standby_handle(handle.clone_inner());
        } else {
            return Err(anyhow!(
                "resolver {} doesn't depend on resolver {}",
                self.config.name(),
                target
            ));
        }

        let inner_config = g3_resolver::ResolverConfig {
            name: self.config.name().to_string(),
            runtime: self.config.runtime.clone(),
            driver: g3_resolver::AnyResolveDriverConfig::FailOver(driver_config.clone()),
        };

        self.inner
            .update_config(inner_config)
            .context("failed to update inner fail_over resolver config")?;
        self.driver_config = driver_config;
        Ok(())
    }

    async fn _shutdown(&mut self) {
        self.inner.shutdown().await;
    }
}

impl Resolver for FailOverResolver {
    fn get_handle(&self) -> ArcIntegratedResolverHandle {
        let inner_context = self.inner.get_handle();
        Arc::new(super::FailOverResolverHandle::new(
            &self.config,
            inner_context,
            &self.logger,
        ))
    }

    fn get_stats(&self) -> Arc<ResolverStats> {
        Arc::clone(&self.stats)
    }
}
