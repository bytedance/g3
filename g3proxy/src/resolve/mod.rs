/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;

use g3_types::metrics::NodeName;

use crate::config::resolver::AnyResolverConfig;

#[macro_use]
mod handle;
pub(crate) use handle::{
    ArcIntegratedResolverHandle, ArriveFirstResolveJob, HappyEyeballsResolveJob,
    IntegratedResolverHandle,
};
use handle::{BoxLoggedResolveJob, ErrorResolveJob, LoggedResolveJob};

mod stats;
pub(crate) use stats::ResolverStats;

mod registry;
pub(crate) use registry::{get_handle, get_names};

#[cfg(feature = "c-ares")]
mod c_ares;
mod hickory;

mod deny_all;
mod fail_over;

mod ops;
pub use ops::spawn_all;
pub(crate) use ops::{foreach_resolver, reload};

pub(crate) trait Resolver {
    fn get_handle(&self) -> ArcIntegratedResolverHandle;
    fn get_stats(&self) -> Arc<ResolverStats>;
}

#[async_trait]
trait ResolverInternal: Resolver {
    fn _dependent_resolver(&self) -> Option<BTreeSet<NodeName>>;

    fn _clone_config(&self) -> AnyResolverConfig;
    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        dep_table: BTreeMap<NodeName, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()>;
    fn _update_dependent_handle(
        &mut self,
        target: &NodeName,
        handle: ArcIntegratedResolverHandle,
    ) -> anyhow::Result<()>;

    async fn _shutdown(&mut self);
}

type BoxResolverInternal = Box<dyn ResolverInternal + Send>;
