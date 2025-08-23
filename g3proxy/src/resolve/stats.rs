/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_types::metrics::NodeName;
use g3_types::stats::StatId;

pub(crate) struct ResolverStats {
    id: StatId,
    name: NodeName,
    inner: Arc<g3_resolver::ResolverStats>,
}

impl ResolverStats {
    pub(crate) fn new(name: &NodeName, inner: Arc<g3_resolver::ResolverStats>) -> Self {
        ResolverStats {
            id: StatId::new_unique(),
            name: name.clone(),
            inner,
        }
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    pub(crate) fn inner(&self) -> &Arc<g3_resolver::ResolverStats> {
        &self.inner
    }
}
