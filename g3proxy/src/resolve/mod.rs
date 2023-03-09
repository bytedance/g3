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

use async_trait::async_trait;

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
pub(crate) use registry::{foreach as foreach_resolver, get_handle, get_names};

#[cfg(feature = "c-ares")]
mod c_ares;
mod trust_dns;

mod deny_all;
mod fail_over;

mod ops;
pub(crate) use ops::reload;
pub use ops::spawn_all;

#[async_trait]
pub(crate) trait ResolverInternal {
    fn _dependent_resolver(&self) -> Option<BTreeSet<String>>;

    fn _clone_config(&self) -> AnyResolverConfig;
    fn _update_config(
        &mut self,
        config: AnyResolverConfig,
        dep_table: BTreeMap<String, ArcIntegratedResolverHandle>,
    ) -> anyhow::Result<()>;
    fn _update_dependent_handle(
        &mut self,
        target: &str,
        handle: ArcIntegratedResolverHandle,
    ) -> anyhow::Result<()>;

    async fn _shutdown(&mut self);
}

pub(crate) trait Resolver: ResolverInternal {
    fn get_handle(&self) -> ArcIntegratedResolverHandle;
    fn get_stats(&self) -> Arc<ResolverStats>;
}

pub(crate) type BoxResolver = Box<dyn Resolver + Send>;
