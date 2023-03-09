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

use std::sync::Arc;

use g3_types::stats::StatId;

pub(crate) struct ResolverStats {
    id: StatId,
    name: String,
    inner: Arc<g3_resolver::ResolverStats>,
}

impl ResolverStats {
    pub(crate) fn new(name: &str, inner: Arc<g3_resolver::ResolverStats>) -> Self {
        ResolverStats {
            id: StatId::new(),
            name: name.to_string(),
            inner,
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub(crate) fn inner(&self) -> &Arc<g3_resolver::ResolverStats> {
        &self.inner
    }
}
