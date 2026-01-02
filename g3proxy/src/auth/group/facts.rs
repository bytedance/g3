/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::sync::Arc;

use super::BaseUserGroup;
use crate::config::auth::FactsUserGroupConfig;

pub(crate) struct FactsUserGroup {
    base: BaseUserGroup<FactsUserGroupConfig>,
}

impl FactsUserGroup {
    pub(super) fn base(&self) -> &BaseUserGroup<FactsUserGroupConfig> {
        &self.base
    }

    pub(super) fn clone_config(&self) -> FactsUserGroupConfig {
        (*self.base.config).clone()
    }

    pub(super) async fn new_with_config(config: FactsUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = BaseUserGroup::new_with_config(config).await?;
        Ok(Arc::new(FactsUserGroup { base }))
    }

    pub(super) fn reload(&self, config: FactsUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = self.base.reload(config)?;
        Ok(Arc::new(FactsUserGroup { base }))
    }
}
