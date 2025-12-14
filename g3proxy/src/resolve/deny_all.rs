/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::anyhow;
use arcstr::ArcStr;
use async_trait::async_trait;

use g3_resolver::{ResolveError, ResolveLocalError};
use g3_types::metrics::NodeName;

use super::{
    ArcIntegratedResolverHandle, BoxLoggedResolveJob, ErrorResolveJob, IntegratedResolverHandle,
    Resolver, ResolverInternal,
};
use crate::config::resolver::deny_all::DenyAllResolverConfig;
use crate::config::resolver::{AnyResolverConfig, ResolverConfig};
use crate::resolve::{BoxResolverInternal, ResolverStats};

pub(super) struct DenyAllResolver {
    config: Arc<DenyAllResolverConfig>,
    stats: Arc<ResolverStats>,
}

impl DenyAllResolver {
    pub(super) fn new_obj(config: DenyAllResolverConfig) -> anyhow::Result<BoxResolverInternal> {
        let stats = g3_resolver::ResolverStats::default();
        let stats = ResolverStats::new(config.name(), Arc::new(stats));
        Ok(Box::new(DenyAllResolver {
            config: Arc::new(config),
            stats: Arc::new(stats),
        }))
    }
}

#[async_trait]
impl ResolverInternal for DenyAllResolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<NodeName>> {
        None
    }

    fn _clone_config(&self) -> AnyResolverConfig {
        AnyResolverConfig::DenyAll((*self.config).clone())
    }

    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        _dep_table: BTreeMap<NodeName, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()> {
        match config {
            AnyResolverConfig::DenyAll(config) => {
                self.config = Arc::new(config);
                Ok(())
            }
            _ => Err(anyhow!("invalid config type for DenyAllResolver")),
        }
    }

    fn _update_dependent_handle(
        &mut self,
        _target: &NodeName,
        _handle: ArcIntegratedResolverHandle,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn _shutdown(&mut self) {}
}

impl Resolver for DenyAllResolver {
    fn get_handle(&self) -> ArcIntegratedResolverHandle {
        Arc::new(DenyAllResolverHandle::new(&self.config))
    }

    fn get_stats(&self) -> Arc<ResolverStats> {
        Arc::clone(&self.stats)
    }
}

struct DenyAllResolverHandle {
    config: Arc<DenyAllResolverConfig>,
}

impl DenyAllResolverHandle {
    fn new(config: &Arc<DenyAllResolverConfig>) -> Self {
        DenyAllResolverHandle {
            config: Arc::clone(config),
        }
    }
}

impl IntegratedResolverHandle for DenyAllResolverHandle {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn is_closed(&self) -> bool {
        // the task that hold this handle will always fail
        false
    }

    fn query_v4(&self, _domain: ArcStr) -> Result<BoxLoggedResolveJob, ResolveError> {
        Ok(Box::new(ErrorResolveJob::with_error(
            ResolveLocalError::NoResolverRunning.into(),
        )))
    }

    fn query_v6(&self, _domain: ArcStr) -> Result<BoxLoggedResolveJob, ResolveError> {
        Ok(Box::new(ErrorResolveJob::with_error(
            ResolveLocalError::NoResolverRunning.into(),
        )))
    }

    fn clone_inner(&self) -> Option<g3_resolver::ResolverHandle> {
        None
    }
}
