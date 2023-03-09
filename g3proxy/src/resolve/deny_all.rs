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

use anyhow::anyhow;
use async_trait::async_trait;

use g3_resolver::{ResolveError, ResolveLocalError};

use super::{
    ArcIntegratedResolverHandle, BoxLoggedResolveJob, ErrorResolveJob, IntegratedResolverHandle,
    Resolver, ResolverInternal,
};
use crate::config::resolver::deny_all::DenyAllResolverConfig;
use crate::config::resolver::{AnyResolverConfig, ResolverConfig};
use crate::resolve::{BoxResolver, ResolverStats};

pub(super) struct DenyAllResolver {
    config: Arc<DenyAllResolverConfig>,
    stats: Arc<ResolverStats>,
}

impl DenyAllResolver {
    pub(super) fn new_obj(config: AnyResolverConfig) -> anyhow::Result<BoxResolver> {
        if let AnyResolverConfig::DenyAll(config) = config {
            let stats = g3_resolver::ResolverStats::default();
            let stats = ResolverStats::new(config.name(), Arc::new(stats));
            Ok(Box::new(DenyAllResolver {
                config: Arc::new(config),
                stats: Arc::new(stats),
            }))
        } else {
            Err(anyhow!("invalid config type for DenyAllResolver"))
        }
    }
}

#[async_trait]
impl ResolverInternal for DenyAllResolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<String>> {
        None
    }

    fn _clone_config(&self) -> AnyResolverConfig {
        AnyResolverConfig::DenyAll((*self.config).clone())
    }

    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        _dep_table: BTreeMap<String, ArcIntegratedResolverHandle>,
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
        _target: &str,
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
    fn name(&self) -> &str {
        self.config.name()
    }

    fn is_closed(&self) -> bool {
        // the task that hold this handle will always fail
        false
    }

    fn query_v4(&self, _domain: String) -> Result<BoxLoggedResolveJob, ResolveError> {
        Ok(Box::new(ErrorResolveJob::with_error(
            ResolveLocalError::NoResolverRunning.into(),
        )))
    }

    fn query_v6(&self, _domain: String) -> Result<BoxLoggedResolveJob, ResolveError> {
        Ok(Box::new(ErrorResolveJob::with_error(
            ResolveLocalError::NoResolverRunning.into(),
        )))
    }

    fn clone_inner(&self) -> Option<g3_resolver::ResolverHandle> {
        None
    }
}
