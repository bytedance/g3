/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::sync::Arc;

use super::BaseUserGroup;
use crate::config::auth::LdapUserGroupConfig;

mod protocol;
use protocol::{LdapMessageReceiver, SimpleBindRequestEncoder};

mod pool;
use pool::LdapConnector;

pub(crate) struct LdapUserGroup {
    base: BaseUserGroup<LdapUserGroupConfig>,
}

impl LdapUserGroup {
    pub(super) fn base(&self) -> &BaseUserGroup<LdapUserGroupConfig> {
        &self.base
    }

    pub(super) fn clone_config(&self) -> LdapUserGroupConfig {
        (*self.base.config).clone()
    }

    fn new(base: BaseUserGroup<LdapUserGroupConfig>) -> Self {
        LdapUserGroup { base }
    }

    pub(super) async fn new_with_config(config: LdapUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = BaseUserGroup::new_with_config(config).await?;
        Ok(Arc::new(Self::new(base)))
    }

    pub(super) fn reload(&self, config: LdapUserGroupConfig) -> anyhow::Result<Arc<Self>> {
        let base = self.base.reload(config)?;
        Ok(Arc::new(Self::new(base)))
    }
}
