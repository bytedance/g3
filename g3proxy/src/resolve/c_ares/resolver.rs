/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use slog::Logger;

use g3_types::metrics::NodeName;

use crate::config::resolver::c_ares::CAresResolverConfig;
use crate::config::resolver::{AnyResolverConfig, ResolverConfig};
use crate::resolve::{
    ArcIntegratedResolverHandle, BoxResolverInternal, Resolver, ResolverInternal, ResolverStats,
};

pub(crate) struct CAresResolver {
    config: Arc<CAresResolverConfig>,
    inner: g3_resolver::Resolver,
    stats: Arc<ResolverStats>,
    logger: Option<Logger>,
}

impl CAresResolver {
    pub(crate) fn new_obj(config: CAresResolverConfig) -> anyhow::Result<BoxResolverInternal> {
        let mut builder = g3_resolver::ResolverBuilder::new((&config).into());
        builder.thread_name(format!("res-{}", config.name()));
        let resolver = builder.build()?;

        let logger = crate::log::resolve::get_logger(config.r#type(), config.name());
        let stats = ResolverStats::new(config.name(), resolver.get_stats());

        Ok(Box::new(CAresResolver {
            config: Arc::new(config),
            inner: resolver,
            stats: Arc::new(stats),
            logger,
        }))
    }
}

#[async_trait]
impl ResolverInternal for CAresResolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<NodeName>> {
        None
    }

    fn _clone_config(&self) -> AnyResolverConfig {
        AnyResolverConfig::CAres(self.config.as_ref().clone())
    }

    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        _dep_table: BTreeMap<NodeName, ArcIntegratedResolverHandle>,
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
        _target: &NodeName,
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
            self.logger.clone(),
        ))
    }

    fn get_stats(&self) -> Arc<ResolverStats> {
        Arc::clone(&self.stats)
    }
}
