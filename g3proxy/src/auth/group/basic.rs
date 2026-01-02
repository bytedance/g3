/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::sync::Arc;

use g3_types::metrics::NodeName;

use super::BaseUserGroup;
use crate::config::auth::BasicUserGroupConfig;

pub(crate) struct BasicUserGroup {
    base: BaseUserGroup<BasicUserGroupConfig>,
}

impl BasicUserGroup {
    pub(super) fn base(&self) -> &BaseUserGroup<BasicUserGroupConfig> {
        &self.base
    }

    pub(super) fn clone_config(&self) -> BasicUserGroupConfig {
        (*self.base.config).clone()
    }

    pub(super) fn new_no_config(name: &NodeName) -> Arc<Self> {
        let config = BasicUserGroupConfig::empty(name);
        let base = BaseUserGroup::new_without_users(config);
        Arc::new(BasicUserGroup { base })
    }

    pub(super) async fn new_with_config(config: BasicUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = BaseUserGroup::new_with_config(config).await?;
        Ok(Arc::new(BasicUserGroup { base }))
    }

    pub(super) fn reload(&self, config: BasicUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = self.base.reload(config)?;
        Ok(Arc::new(BasicUserGroup { base }))
    }
}
