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

use std::sync::Arc;

use async_trait::async_trait;

use g3_types::metrics::MetricsName;

use crate::config::backend::AnyBackendConfig;

mod dummy_close;
mod stream_tcp;

mod ops;
pub use ops::load_all;
pub(crate) use ops::{reload, update_dependency_to_discover};

mod registry;
pub(crate) use registry::{get_names, get_or_insert_default};

#[async_trait]
pub(crate) trait Backend {
    fn _clone_config(&self) -> AnyBackendConfig;
    fn _update_config_in_place(&self, flags: u64, config: AnyBackendConfig) -> anyhow::Result<()>;

    /// registry lock is allowed in this method
    async fn _lock_safe_reload(&self, config: AnyBackendConfig) -> anyhow::Result<ArcBackend>;

    fn name(&self) -> &MetricsName;

    fn discover(&self) -> &MetricsName;
    fn _update_discover(&self) -> anyhow::Result<()>;
}

pub(crate) type ArcBackend = Arc<dyn Backend + Send + Sync>;
